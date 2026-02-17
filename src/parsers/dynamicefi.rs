//! DynamicEFI EBL WhatsUp log file parser.
//!
//! Parses CSV log files exported from DynamicEFI's EBL WhatsUp software,
//! used with modified GM TBI (Throttle Body Injection) systems.
//!
//! The software logs to `.dat` files (raw serial stream) and supports
//! exporting to CSV. The CSV format uses:
//! - `RUNTIME` column in HH:MM:SS format (1-second resolution)
//! - Comma-delimited values with trailing comma
//! - Y/N flag columns and P/N (Park/Neutral) gear indicator
//! - GM TBI-specific channel abbreviations (BLM, BPC, INT, etc.)

use serde::Serialize;
use std::error::Error;

use super::types::{Channel, Log, Meta, Parseable, Value};

/// DynamicEFI log file metadata
#[derive(Clone, Debug, Default, Serialize)]
pub struct DynamicEfiMeta {
    pub channel_count: usize,
    pub data_points: usize,
}

/// DynamicEFI channel definition
#[derive(Clone, Debug, Default, Serialize)]
pub struct DynamicEfiChannel {
    pub name: String,
    pub unit: String,
}

impl DynamicEfiChannel {
    /// Create a channel from a header column name
    pub fn from_header(header: &str) -> Self {
        let name = header.trim().to_string();
        let unit = Self::infer_unit(&name);
        Self { name, unit }
    }

    /// Infer unit from known GM TBI channel abbreviations
    fn infer_unit(name: &str) -> String {
        match name {
            "RPM" => "rpm",
            "MPH" => "mph",
            "MAP" | "BRO" => "kPa",
            "PSI" => "psi",
            "VAC" => "inHg",
            "VE%" | "DC%" | "TPS" => "%",
            "CTS" | "IAT" | "I/C" => "°F",
            "O2" => "mV",
            "G/S" => "g/s",
            "SA" => "°",
            "sPW" | "aPW" | "aePW" => "ms",
            "AFR" | "WB" | "WB_0" => "AFR",
            "IAC" => "steps",
            "dTPS" | "tpsAE" | "dMAP" | "mapAE" => "/s",
            _ => "",
        }
        .to_string()
    }

    /// Get the display unit for this channel
    pub fn unit(&self) -> &str {
        &self.unit
    }
}

/// DynamicEFI log file parser
pub struct DynamicEfi;

impl DynamicEfi {
    /// Detect if file contents look like a DynamicEFI EBL WhatsUp log
    pub fn detect(contents: &str) -> bool {
        if let Some(first_line) = contents.lines().next() {
            let first_line_upper = first_line.to_uppercase();
            // Must start with RUNTIME as first column
            if !first_line_upper.starts_with("RUNTIME,") {
                return false;
            }
            // Confirm GM TBI-specific headers to avoid false positives
            first_line_upper.contains(",BLM,") || first_line_upper.contains(",BPC,")
        } else {
            false
        }
    }

    /// Parse HH:MM:SS time string to seconds
    fn parse_runtime(time_str: &str) -> Option<f64> {
        let time_str = time_str.trim();
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() != 3 {
            return None;
        }
        let hours: f64 = parts[0].parse().ok()?;
        let minutes: f64 = parts[1].parse().ok()?;
        let seconds: f64 = parts[2].parse().ok()?;
        Some(hours * 3600.0 + minutes * 60.0 + seconds)
    }

    /// Parse a single cell value, handling Y/N flags and P/N gear indicator
    fn parse_value(value: &str) -> f64 {
        let value = value.trim();
        match value {
            "Y" => 1.0,
            "N" | "P/N" => 0.0,
            _ => value.parse::<f64>().unwrap_or(0.0),
        }
    }
}

impl Parseable for DynamicEfi {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        let line_count = file_contents.lines().count();
        let estimated_data_rows = line_count.saturating_sub(1);

        let mut channels: Vec<Channel> = Vec::with_capacity(50);
        let mut times: Vec<f64> = Vec::with_capacity(estimated_data_rows);
        let mut data: Vec<Vec<Value>> = Vec::with_capacity(estimated_data_rows);

        let mut lines = file_contents.lines();

        // Parse header line
        let header = lines.next().ok_or("Empty file: no header found")?;
        let column_names: Vec<&str> = header.split(',').collect();

        if column_names.len() < 2 {
            return Err("Invalid DynamicEFI log: insufficient columns".into());
        }

        // Create channels from header (skip RUNTIME, skip empty trailing fields)
        for name in column_names.iter().skip(1) {
            let name = name.trim();
            if !name.is_empty() {
                channels.push(Channel::DynamicEfi(DynamicEfiChannel::from_header(name)));
            }
        }

        // Collect raw parsed rows with their whole-second timestamps
        struct RawRow {
            time_secs: f64,
            values: Vec<Value>,
        }

        let mut raw_rows: Vec<RawRow> = Vec::with_capacity(estimated_data_rows);

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() < 2 {
                continue;
            }

            if let Some(time_secs) = Self::parse_runtime(parts[0]) {
                let mut row_values: Vec<Value> = Vec::with_capacity(channels.len());

                for part in parts.iter().skip(1) {
                    let part = part.trim();
                    if part.is_empty() {
                        continue; // Skip trailing empty field from trailing comma
                    }
                    row_values.push(Value::Float(Self::parse_value(part)));
                }

                // Pad if needed
                while row_values.len() < channels.len() {
                    row_values.push(Value::Float(0.0));
                }

                // Truncate if we got more values than channels (shouldn't happen, but be safe)
                row_values.truncate(channels.len());

                raw_rows.push(RawRow {
                    time_secs,
                    values: row_values,
                });
            }
        }

        // Distribute sub-second timestamps evenly within each second group.
        // The RUNTIME column only has 1-second resolution but data is sampled
        // much faster, so we interpolate to preserve ordering.
        // Also handles RUNTIME resets (wraps back to 00:00:00) by continuing
        // monotonically with a 1-second gap.
        if !raw_rows.is_empty() {
            let first_time = raw_rows[0].time_secs;
            let mut time_offset: f64 = 0.0;
            let mut prev_raw_time = first_time;
            let mut i = 0;

            while i < raw_rows.len() {
                let raw_time = raw_rows[i].time_secs;

                // Detect time wrap (RUNTIME reset to 00:00:00)
                if raw_time < prev_raw_time {
                    // Continue from where we left off + 1 second gap
                    time_offset = times.last().copied().unwrap_or(0.0) + 1.0 + first_time;
                }
                prev_raw_time = raw_time;

                let group_start = i;

                // Find the end of this same-second group
                while i < raw_rows.len() && raw_rows[i].time_secs == raw_time {
                    i += 1;
                }

                let group_size = i - group_start;
                for (j, row) in raw_rows[group_start..i].iter().enumerate() {
                    let fractional = if group_size > 1 {
                        j as f64 / group_size as f64
                    } else {
                        0.0
                    };
                    times.push((raw_time + fractional) - first_time + time_offset);
                    data.push(row.values.clone());
                }
            }
        }

        tracing::info!(
            "Parsed DynamicEFI log: {} channels, {} data points",
            channels.len(),
            data.len()
        );

        Ok(Log {
            meta: Meta::DynamicEfi(DynamicEfiMeta {
                channel_count: channels.len(),
                data_points: data.len(),
            }),
            channels,
            times,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect() {
        // Should detect DynamicEFI format
        assert!(DynamicEfi::detect(
            "RUNTIME,RPM,MPH,MAP,PSI,BRO,VAC,VE%,TPS,CTS,IAT,I/C,O2,G/S,SA,Rt,KnkCt,sPW,aPW,DC%,Sf,Ay,Ae,De,Id,Hw,Pe,Dc,Cl,Ln,Fn,Ac,Tc,Cp,Eg,Gr,Cl,INT,BLM,BPC,IAC,AFR,WB,dTPS,tpsAE,dMAP,mapAE,aePW,WB_0,\n00:00:00,0,0,98"
        ));

        // Should not detect other formats
        assert!(!DynamicEfi::detect("Time (msec),Engine Speed (rpm)"));
        assert!(!DynamicEfi::detect("%DataLog%"));
        assert!(!DynamicEfi::detect("TIME;engine/rpm;sensors/tps"));
        assert!(!DynamicEfi::detect("TimeStamp: Sat Nov 15 19:00:03 2025"));

        // RUNTIME without GM-specific headers should not match
        assert!(!DynamicEfi::detect("RUNTIME,RPM,MPH,MAP\n00:00:00,0,0,98"));
    }

    #[test]
    fn test_parse_runtime() {
        assert_eq!(DynamicEfi::parse_runtime("00:00:00"), Some(0.0));
        assert_eq!(DynamicEfi::parse_runtime("00:01:53"), Some(113.0));
        assert_eq!(DynamicEfi::parse_runtime("01:00:00"), Some(3600.0));
        assert_eq!(DynamicEfi::parse_runtime("00:30:45"), Some(1845.0));
        assert_eq!(DynamicEfi::parse_runtime("invalid"), None);
        assert_eq!(DynamicEfi::parse_runtime("00:00"), None);
    }

    #[test]
    fn test_parse_value() {
        assert_eq!(DynamicEfi::parse_value("Y"), 1.0);
        assert_eq!(DynamicEfi::parse_value("N"), 0.0);
        assert_eq!(DynamicEfi::parse_value("P/N"), 0.0);
        assert_eq!(DynamicEfi::parse_value(" 775 "), 775.0);
        assert_eq!(DynamicEfi::parse_value("1.587"), 1.587);
        assert_eq!(DynamicEfi::parse_value("invalid"), 0.0);
    }

    #[test]
    fn test_channel_units() {
        assert_eq!(DynamicEfiChannel::from_header("RPM").unit(), "rpm");
        assert_eq!(DynamicEfiChannel::from_header("MPH").unit(), "mph");
        assert_eq!(DynamicEfiChannel::from_header("MAP").unit(), "kPa");
        assert_eq!(DynamicEfiChannel::from_header("TPS").unit(), "%");
        assert_eq!(DynamicEfiChannel::from_header("CTS").unit(), "°F");
        assert_eq!(DynamicEfiChannel::from_header("AFR").unit(), "AFR");
        assert_eq!(DynamicEfiChannel::from_header("sPW").unit(), "ms");
        assert_eq!(DynamicEfiChannel::from_header("SA").unit(), "°");
        assert_eq!(DynamicEfiChannel::from_header("Sf").unit(), "");
    }

    #[test]
    fn test_parse_basic() {
        let sample = "RUNTIME,RPM,MPH,MAP,BLM,BPC,\n\
                       00:00:00, 750,  0, 35,128,140,\n\
                       00:00:00, 775,  0, 36,128,140,\n\
                       00:00:01, 762,  0, 35,128,140,\n";

        let parser = DynamicEfi;
        let log = parser.parse(sample).unwrap();

        assert_eq!(log.channels.len(), 5);
        assert_eq!(log.channels[0].name(), "RPM");
        assert_eq!(log.channels[1].name(), "MPH");
        assert_eq!(log.channels[2].name(), "MAP");
        assert_eq!(log.channels[3].name(), "BLM");
        assert_eq!(log.channels[4].name(), "BPC");

        assert_eq!(log.times.len(), 3);
        assert_eq!(log.data.len(), 3);

        // First two rows share 00:00:00, should get sub-second interpolation
        assert!((log.times[0] - 0.0).abs() < 0.001);
        assert!(log.times[1] > 0.0 && log.times[1] < 1.0);
        assert!((log.times[2] - 1.0).abs() < 0.001);

        // Check values
        assert_eq!(log.data[0][0].as_f64(), 750.0);
        assert_eq!(log.data[1][0].as_f64(), 775.0);
        assert_eq!(log.data[2][0].as_f64(), 762.0);
    }

    #[test]
    fn test_parse_flags() {
        let sample = "RUNTIME,RPM,Sf,Ay,Gr,BLM,BPC,\n\
                       00:00:00, 750,Y,N,P/N,128,140,\n";

        let parser = DynamicEfi;
        let log = parser.parse(sample).unwrap();

        assert_eq!(log.data[0][1].as_f64(), 1.0); // Y -> 1.0
        assert_eq!(log.data[0][2].as_f64(), 0.0); // N -> 0.0
        assert_eq!(log.data[0][3].as_f64(), 0.0); // P/N -> 0.0
    }

    #[test]
    fn test_parse_real_file() {
        let contents =
            std::fs::read_to_string("exampleLogs/dynamicEFI/20251006_0834_toWorkNewCap.csv");
        if let Ok(contents) = contents {
            assert!(DynamicEfi::detect(&contents));

            let parser = DynamicEfi;
            let log = parser.parse(&contents).unwrap();

            // Should have channels (header has ~48 non-empty columns after RUNTIME)
            assert!(log.channels.len() > 40);
            assert!(log.data.len() > 1000);
            assert_eq!(log.times.len(), log.data.len());

            // Check known channels exist
            let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
            assert!(names.contains(&"RPM".to_string()));
            assert!(names.contains(&"MAP".to_string()));
            assert!(names.contains(&"AFR".to_string()));
            assert!(names.contains(&"BLM".to_string()));

            // Times should be monotonically non-decreasing
            for i in 1..log.times.len() {
                assert!(
                    log.times[i] >= log.times[i - 1],
                    "Time at index {} ({}) < time at index {} ({})",
                    i,
                    log.times[i],
                    i - 1,
                    log.times[i - 1]
                );
            }
        }
    }
}
