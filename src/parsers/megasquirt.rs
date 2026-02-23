//! MegaSquirt log file parser.
//!
//! Parses CSV datalog files exported from TunerStudio for MegaSquirt ECUs
//! (MS1, MS2, MS3, and compatible firmware like MS3Pro).
//!
//! Format:
//! - 4 metadata header lines (tune info, frame count, duration)
//! - Column header line with units in parentheses, e.g. "MAP(mBar)"
//! - Comma-delimited data rows
//! - "Frame" and "Duration" are the first two columns (index and time in seconds)
//!
//! Reference: <https://www.tunerstudio.com>

use serde::Serialize;
use std::error::Error;

use super::types::{Channel, Log, Meta, Parseable, Value};

/// MegaSquirt log file metadata
#[derive(Clone, Debug, Default, Serialize)]
pub struct MegaSquirtMeta {
    /// Tune file name from header
    pub tune_file: String,
    /// Total number of frames reported in header
    pub total_frames: usize,
    /// Number of channels in the log
    pub channel_count: usize,
    /// Number of parsed data points
    pub data_points: usize,
}

/// MegaSquirt channel definition
#[derive(Clone, Debug, Default, Serialize)]
pub struct MegaSquirtChannel {
    /// Display name (e.g., "MAP", "RPM", "ECT")
    pub name: String,
    /// Unit extracted from parentheses (e.g., "mBar", "kmh", "C")
    pub unit: String,
}

impl MegaSquirtChannel {
    /// Create a channel from a header column like "MAP(mBar)" or "RPM"
    pub fn from_header(header: &str) -> Self {
        let header = header.trim().to_string();

        // Parse "Name(unit)" format — no space before the parenthesis
        let (name, unit) = if let Some(paren_start) = header.rfind('(') {
            if let Some(paren_end) = header.rfind(')') {
                if paren_end > paren_start {
                    let unit = header[paren_start + 1..paren_end].trim().to_string();
                    let name = header[..paren_start].trim().to_string();
                    (name, unit)
                } else {
                    (header.clone(), Self::infer_unit(&header))
                }
            } else {
                (header.clone(), Self::infer_unit(&header))
            }
        } else {
            (header.clone(), Self::infer_unit(&header))
        };

        Self { name, unit }
    }

    /// Infer unit from channel name when not explicitly provided
    fn infer_unit(name: &str) -> String {
        let lower = name.to_lowercase();

        if lower == "rpm" {
            return "rpm".to_string();
        }
        if lower.contains("gear") {
            return "".to_string();
        }
        if lower.contains("duty") {
            return "%".to_string();
        }
        if lower.contains("ratio") || lower == "a/f ratio" {
            return "AFR".to_string();
        }
        if lower.contains("mil") {
            return "".to_string();
        }

        "".to_string()
    }

    /// Get the display unit
    pub fn unit(&self) -> &str {
        &self.unit
    }
}

/// MegaSquirt log file parser
pub struct MegaSquirt;

impl MegaSquirt {
    /// Detect if file contents look like a MegaSquirt TunerStudio datalog
    pub fn detect(contents: &str) -> bool {
        if let Some(first_line) = contents.lines().next() {
            // Strip UTF-8 BOM if present (common on Windows TunerStudio exports)
            let first_lower = first_line.trim_start_matches('\u{FEFF}').to_lowercase();
            // TunerStudio datalogs start with "Tune Datalog export of"
            return first_lower.starts_with("tune datalog export");
        }
        false
    }

    /// Parse a text value that may be boolean (ON/OFF) or numeric
    fn parse_value(s: &str) -> f64 {
        let trimmed = s.trim();
        // Handle boolean text values common in MegaSquirt logs (e.g., MIL column)
        match trimmed.to_uppercase().as_str() {
            "ON" | "YES" | "TRUE" | "ACTIVE" => 1.0,
            "OFF" | "NO" | "FALSE" | "INACTIVE" => 0.0,
            _ => trimmed.parse::<f64>().unwrap_or(0.0),
        }
    }
}

impl Parseable for MegaSquirt {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        // Strip UTF-8 BOM if present
        let file_contents = file_contents.trim_start_matches('\u{FEFF}');
        let mut lines = file_contents.lines();

        // --- Parse metadata header (4 lines) ---
        let _export_line = lines.next().ok_or("Empty file")?;

        let tune_file = lines
            .next()
            .ok_or("Missing tune file header")?
            .strip_prefix("Tune file: ")
            .unwrap_or("")
            .to_string();

        let total_frames_line = lines.next().ok_or("Missing total frames header")?;
        let total_frames: usize = total_frames_line
            .strip_prefix("Total Frames: ")
            .unwrap_or("0")
            .trim()
            .parse()
            .unwrap_or(0);

        let _duration_line = lines.next().ok_or("Missing duration header")?;

        // --- Parse column header ---
        let header_line = lines.next().ok_or("Missing column header")?;
        let column_names: Vec<&str> = header_line.split(',').collect();

        if column_names.len() < 3 {
            return Err("Invalid MegaSquirt log: too few columns".into());
        }

        // Build channels, skipping Frame (index 0) and Duration (index 1)
        let mut channels: Vec<Channel> = Vec::with_capacity(column_names.len());
        for name in column_names.iter().skip(2) {
            let ch = MegaSquirtChannel::from_header(name);
            channels.push(Channel::MegaSquirt(ch));
        }

        // --- Parse data rows ---
        let estimated_rows = total_frames.max(1000);
        let mut durations: Vec<f64> = Vec::with_capacity(estimated_rows);
        let mut data: Vec<Vec<Value>> = Vec::with_capacity(estimated_rows);

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() < 2 {
                continue;
            }

            // Column 1 is Duration (seconds)
            let duration: f64 = match parts[1].trim().parse() {
                Ok(v) => v,
                Err(_) => continue,
            };
            durations.push(duration);

            // Parse remaining columns (skip Frame and Duration)
            let mut row: Vec<Value> = Vec::with_capacity(channels.len());
            for part in parts.iter().skip(2) {
                row.push(Value::Float(Self::parse_value(part)));
            }

            // Pad row to match channel count
            while row.len() < channels.len() {
                row.push(Value::Float(0.0));
            }

            data.push(row);
        }

        if durations.is_empty() {
            return Err("No data rows found in MegaSquirt log".into());
        }

        // --- Build time axis ---
        // MegaSquirt logs sometimes have an anomalous first-row Duration
        // (e.g., ECU uptime at log start) that is much larger than the second row.
        // Detect this and normalize so the timeline starts at 0.
        let times = if durations.len() > 1 && durations[0] > durations[1] {
            // Anomalous first row: set row 0 to t=0, rest relative to row 1
            let base = durations[1];
            let mut t = Vec::with_capacity(durations.len());
            t.push(0.0);
            for &d in &durations[1..] {
                t.push(d - base);
            }
            t
        } else {
            // Normal case: subtract first value
            let base = durations[0];
            durations.iter().map(|&d| d - base).collect()
        };

        tracing::info!(
            "Parsed MegaSquirt log: {} channels, {} data points",
            channels.len(),
            data.len()
        );

        Ok(Log {
            meta: Meta::MegaSquirt(MegaSquirtMeta {
                tune_file,
                total_frames,
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
    fn test_detect_megasquirt() {
        assert!(MegaSquirt::detect(
            "Tune Datalog export of DL\nTune file: test.bin\n"
        ));
        assert!(MegaSquirt::detect(
            "tune datalog export of DL\nTune file: test.bin\n"
        ));
        // Should detect with UTF-8 BOM
        assert!(MegaSquirt::detect(
            "\u{FEFF}Tune Datalog export of DL\nTune file: test.bin\n"
        ));
        assert!(!MegaSquirt::detect("Time (msec),RPM\n0,1000\n"));
        assert!(!MegaSquirt::detect("%DataLog%\nDataLogVersion : 1.1\n"));
    }

    #[test]
    fn test_channel_from_header() {
        let ch = MegaSquirtChannel::from_header("MAP(mBar)");
        assert_eq!(ch.name, "MAP");
        assert_eq!(ch.unit, "mBar");

        let ch = MegaSquirtChannel::from_header("VSS(kmh)");
        assert_eq!(ch.name, "VSS");
        assert_eq!(ch.unit, "kmh");

        let ch = MegaSquirtChannel::from_header("RPM");
        assert_eq!(ch.name, "RPM");
        assert_eq!(ch.unit, "rpm");

        let ch = MegaSquirtChannel::from_header("MIL");
        assert_eq!(ch.name, "MIL");
        assert_eq!(ch.unit, "");

        let ch = MegaSquirtChannel::from_header("TPS Voltage(v)");
        assert_eq!(ch.name, "TPS Voltage");
        assert_eq!(ch.unit, "v");

        let ch = MegaSquirtChannel::from_header("A/F Ratio");
        assert_eq!(ch.name, "A/F Ratio");
        assert_eq!(ch.unit, "AFR");
    }

    #[test]
    fn test_parse_megasquirt_log() {
        let sample = "Tune Datalog export of DL\n\
                       Tune file: test.bin\n\
                       Total Frames: 5\n\
                       Duration :00:00:02.000\n\
                       Frame,Duration,RPM,MAP(mBar),TPS(%),ECT(C)\n\
                       0,0.0,1800,311,5.0,88\n\
                       1,0.5,1850,320,5.5,88\n\
                       2,1.0,1900,330,6.0,89\n\
                       3,1.5,1950,340,6.5,89\n\
                       4,2.0,2000,350,7.0,90\n";

        let parser = MegaSquirt;
        let log = parser.parse(sample).unwrap();

        assert_eq!(log.channels.len(), 4);
        assert_eq!(log.channels[0].name(), "RPM");
        assert_eq!(log.channels[1].name(), "MAP");
        assert_eq!(log.channels[2].name(), "TPS");
        assert_eq!(log.channels[3].name(), "ECT");

        assert_eq!(log.channels[1].unit(), "mBar");
        assert_eq!(log.channels[2].unit(), "%");
        assert_eq!(log.channels[3].unit(), "C");

        assert_eq!(log.times.len(), 5);
        assert_eq!(log.data.len(), 5);

        // Times should be normalized to start at 0
        assert!((log.times[0] - 0.0).abs() < 0.001);
        assert!((log.times[1] - 0.5).abs() < 0.001);
        assert!((log.times[4] - 2.0).abs() < 0.001);

        // Check data values
        assert_eq!(log.data[0][0].as_f64(), 1800.0); // RPM
        assert_eq!(log.data[0][1].as_f64(), 311.0); // MAP
        assert_eq!(log.data[4][0].as_f64(), 2000.0); // RPM last row
    }

    #[test]
    fn test_parse_anomalous_first_row() {
        let sample = "Tune Datalog export of DL\n\
                       Tune file: test.bin\n\
                       Total Frames: 3\n\
                       Duration :00:00:01.000\n\
                       Frame,Duration,RPM,MAP(mBar)\n\
                       0,281.816,1800,311\n\
                       1,0.02,1850,320\n\
                       2,0.52,1900,330\n";

        let parser = MegaSquirt;
        let log = parser.parse(sample).unwrap();

        // First row should be t=0, rest relative to row 1
        assert!((log.times[0] - 0.0).abs() < 0.001);
        assert!((log.times[1] - 0.0).abs() < 0.001);
        assert!((log.times[2] - 0.5).abs() < 0.001);

        assert_eq!(log.data.len(), 3);
        assert_eq!(log.data[0][0].as_f64(), 1800.0);
    }

    #[test]
    fn test_parse_with_text_values() {
        let sample = "Tune Datalog export of DL\n\
                       Tune file: test.bin\n\
                       Total Frames: 2\n\
                       Duration :00:00:01.000\n\
                       Frame,Duration,RPM,MIL\n\
                       0,0.0,1800,OFF\n\
                       1,0.5,1850,ON\n";

        let parser = MegaSquirt;
        let log = parser.parse(sample).unwrap();

        // Boolean text values: OFF -> 0.0, ON -> 1.0
        assert_eq!(log.data[0][1].as_f64(), 0.0);
        assert_eq!(log.data[1][1].as_f64(), 1.0);
    }

    #[test]
    fn test_parse_real_file() {
        let contents =
            std::fs::read_to_string("exampleLogs/megasquirt/10.92.csv").expect("example file");
        let parser = MegaSquirt;
        let log = parser.parse(&contents).unwrap();

        // Should have channels (23 columns - Frame - Duration = 21 channels)
        assert_eq!(log.channels.len(), 21);

        // Should have all data rows
        assert_eq!(log.data.len(), 7977); // 7976 frames + 1 (0-indexed)

        // Times should start at 0
        assert!((log.times[0] - 0.0).abs() < 0.001);

        // Times should be monotonically increasing after the first row
        for i in 2..log.times.len() {
            assert!(
                log.times[i] >= log.times[i - 1],
                "Time should be monotonic at index {}",
                i
            );
        }

        // Check some known channel names
        assert_eq!(log.channels[0].name(), "RPM");
        assert_eq!(log.channels[3].name(), "MAP");
        assert_eq!(log.channels[3].unit(), "mBar");
    }
}
