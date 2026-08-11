//! MHD Tuning log file parser.
//!
//! Parses CSV datalogs exported by the MHD Tuning flashing/logging app
//! (BMW N54/N55/S55/B58 and similar platforms).
//!
//! Format characteristics:
//! - Optional UTF-8 BOM at the start of the file
//! - A leading block of `#`-prefixed metadata lines, e.g.
//!   `#Encoding: UTF-8`, `#Ecu CALID:`, `#Ecu PRGID: 7616431`, `#VIN: ...`
//! - A comma-delimited header row whose first column is `Time`
//! - Channel names carry their unit in trailing parentheses, e.g.
//!   `Boost (Bar)`, `Coolant (*C)`, `RPM (RPM)`, `Gear (-)`
//! - The final header column is usually the active tune file name
//!   (e.g. `MHD 5.26 IJE0S ST2V7.bin`) and has a matching data column
//! - Data rows are plain decimal numbers; `Time` is seconds
//!
//! MHD writes `*` where other tools write `°`, so `(*C)` means °C,
//! `(*)` means degrees and `(*CRK)` means degrees crank angle.
//!
//! Reference: <https://www.mhdtuning.com/>

use serde::Serialize;
use std::error::Error;

use super::types::{Channel, Log, Meta, Parseable, Value};

/// Number of leading lines inspected when fingerprinting a file.
/// MHD writes a handful of metadata lines; scanning more is wasted work.
const DETECT_SCAN_LINES: usize = 16;

/// MHD log file metadata, taken from the leading `#` comment block.
#[derive(Clone, Debug, Default, Serialize)]
pub struct MhdMeta {
    /// `#Encoding` value (e.g. "UTF-8")
    pub encoding: String,
    /// `#Ecu CALID` value — often blank in exports
    pub ecu_calid: String,
    /// `#Ecu PRGID` value
    pub ecu_prgid: String,
    /// `#VIN` value
    pub vin: String,
    /// Active tune file name taken from the last header column
    /// (e.g. "MHD 5.26 IJE0S ST2V7.bin")
    pub tune: String,
    /// Number of channels in the log
    pub channel_count: usize,
    /// Number of data points
    pub data_points: usize,
}

/// MHD channel definition
#[derive(Clone, Debug, Default, Serialize)]
pub struct MhdChannel {
    /// Channel name with the trailing unit removed (e.g. "Boost")
    pub name: String,
    /// Display unit normalized from MHD notation (e.g. "Bar", "°C")
    pub unit: String,
}

impl MhdChannel {
    /// Build a channel from a raw header column such as `Coolant (*C)`.
    pub fn from_header(header: &str) -> Self {
        let header = header.trim();

        // Units live in the final parenthesized group. `rfind` keeps names
        // that themselves contain parentheses intact, e.g.
        // "MAF Req (wgdc) (g/s)" -> name "MAF Req (wgdc)", unit "g/s".
        let (name, unit) = match (header.rfind('('), header.rfind(')')) {
            (Some(open), Some(close)) if close > open && close == header.len() - 1 => (
                header[..open].trim().to_string(),
                Self::normalize_unit(header[open + 1..close].trim()),
            ),
            _ => (header.to_string(), String::new()),
        };

        // A header of just "(unit)" would leave an empty name — keep the raw
        // column text in that case so the channel is still addressable.
        if name.is_empty() {
            return Self {
                name: header.to_string(),
                unit: String::new(),
            };
        }

        Self { name, unit }
    }

    /// Translate MHD unit notation into the symbols the rest of the app uses.
    /// MHD substitutes `*` for the degree sign and marks unitless channels
    /// with `-`.
    fn normalize_unit(unit: &str) -> String {
        match unit {
            "-" | "" => String::new(),
            "*" => "°".to_string(),
            _ if unit.starts_with('*') => format!("°{}", &unit[1..]),
            _ => unit.to_string(),
        }
    }
}

/// MHD Tuning log file parser
pub struct Mhd;

impl Mhd {
    /// Detect an MHD Tuning CSV export.
    ///
    /// Detection runs against the raw file text because the MHD fingerprint
    /// lives in the leading `#` comment block, which the generic comment
    /// stripping in the dispatch chain removes before the other parsers run.
    pub fn detect(contents: &str) -> bool {
        let contents = contents.trim_start_matches('\u{FEFF}');
        let mut saw_metadata_marker = false;

        for line in contents.lines().take(DETECT_SCAN_LINES) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(comment) = line.strip_prefix('#') {
                let key = comment.trim_start().to_ascii_lowercase();
                // "Ecu CALID"/"Ecu PRGID" are MHD-specific; "Encoding" and
                // "VIN" alone are too generic to fingerprint on.
                if key.starts_with("ecu calid") || key.starts_with("ecu prgid") {
                    saw_metadata_marker = true;
                }
                continue;
            }

            // First non-comment line must be the header row.
            if !Self::is_header_row(line) {
                return false;
            }
            return saw_metadata_marker || Self::has_tune_column(line);
        }

        false
    }

    /// A header row is comma-delimited with `Time` as the first column.
    fn is_header_row(line: &str) -> bool {
        let mut fields = line.split(',');
        let first = fields.next().unwrap_or("").trim();
        first.eq_ignore_ascii_case("time") && fields.next().is_some()
    }

    /// MHD appends the active tune file name as the last header column,
    /// e.g. `MHD 5.26 IJE0S ST2V7.bin`.
    fn has_tune_column(header: &str) -> bool {
        header
            .split(',')
            .any(|field| Self::is_tune_column(field.trim()))
    }

    fn is_tune_column(field: &str) -> bool {
        field.to_ascii_lowercase().ends_with(".bin") && field.to_ascii_uppercase().contains("MHD")
    }

    /// Split a `#Key: Value` comment into its key and value.
    fn parse_metadata_line(comment: &str) -> Option<(String, String)> {
        let (key, value) = comment.split_once(':')?;
        Some((key.trim().to_ascii_lowercase(), value.trim().to_string()))
    }
}

impl Parseable for Mhd {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        let file_contents = file_contents.trim_start_matches('\u{FEFF}');
        let mut meta = MhdMeta::default();
        let mut lines = file_contents.lines();

        // Phase 1: consume the leading '#' comment block.
        let header = loop {
            let line = lines
                .next()
                .ok_or("Invalid MHD format: no header row found")?;
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            if let Some(comment) = trimmed.strip_prefix('#') {
                if let Some((key, value)) = Self::parse_metadata_line(comment) {
                    match key.as_str() {
                        "encoding" => meta.encoding = value,
                        "ecu calid" => meta.ecu_calid = value,
                        "ecu prgid" => meta.ecu_prgid = value,
                        "vin" => meta.vin = value,
                        _ => {}
                    }
                }
                continue;
            }

            break trimmed;
        };

        // Phase 2: build channels from the header row.
        let header_fields: Vec<&str> = header.split(',').collect();
        let time_header = header_fields.first().map(|f| f.trim()).unwrap_or("");
        if !time_header.eq_ignore_ascii_case("time") {
            return Err(format!(
                "Invalid MHD format: expected 'Time' as the first column, found '{}'",
                time_header
            )
            .into());
        }

        if let Some(tune) = header_fields
            .iter()
            .map(|f| f.trim())
            .find(|f| Self::is_tune_column(f))
        {
            meta.tune = tune.to_string();
        }

        let channels: Vec<Channel> = header_fields[1..]
            .iter()
            .map(|h| Channel::Mhd(MhdChannel::from_header(h)))
            .collect();

        let channel_count = channels.len();
        if channel_count == 0 {
            return Err("Invalid MHD format: header row has no channel columns".into());
        }

        // Phase 3: parse data rows.
        // Unparseable fields carry the last known value for that column
        // (0.0 before the first valid sample) so the row stays aligned with
        // the channel list, matching the Haltech and ECUMaster parsers.
        let mut times: Vec<f64> = Vec::new();
        let mut data: Vec<Vec<Value>> = Vec::new();
        let mut last_values: Vec<Value> = vec![Value::Float(0.0); channel_count];
        let mut first_time: Option<f64> = None;
        let mut out_of_order_rows = 0usize;

        for line in lines {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() <= channel_count {
                // Short row - not enough columns to fill every channel.
                continue;
            }

            let time: f64 = match parts[0].trim().parse() {
                Ok(t) => t,
                Err(_) => continue, // Skip rows without a usable timestamp
            };

            let base = *first_time.get_or_insert(time);
            let relative_time = time - base;

            // `times` must stay sorted: computed-channel time-shift lookups
            // binary-search it. Drop any row that steps backwards.
            if times.last().is_some_and(|last| relative_time < *last) {
                out_of_order_rows += 1;
                continue;
            }

            let row: Vec<Value> = parts[1..=channel_count]
                .iter()
                .enumerate()
                .map(|(idx, raw)| match raw.trim().parse::<f64>() {
                    Ok(v) => {
                        let value = Value::Float(v);
                        last_values[idx] = value;
                        value
                    }
                    Err(_) => last_values[idx],
                })
                .collect();

            times.push(relative_time);
            data.push(row);
        }

        if out_of_order_rows > 0 {
            tracing::warn!(
                "MHD log contained {} out-of-order timestamps; those rows were dropped",
                out_of_order_rows
            );
        }

        meta.channel_count = channel_count;
        meta.data_points = times.len();

        tracing::info!(
            "MHD parse complete: {} channels, {} data points",
            channel_count,
            meta.data_points
        );

        Ok(Log {
            meta: Meta::Mhd(meta),
            channels,
            times,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "#Encoding: UTF-8\n\
        #Ecu CALID: \n\
        #Ecu PRGID: 7616431\n\
        #VIN: TESTVIN0000000000\n\
        Time,Boost (Bar),Coolant (*C),Gear (-),MAF Req (wgdc) (g/s),RPM (RPM),MHD 5.26 IJE0S ST2V7.bin\n\
        539.607,-0.479,89,2,15,1346,0\n\
        539.742,-0.466,89,2,15.5,1356,0\n\
        539.867,-0.455,90,2,16.1,1372,0\n";

    #[test]
    fn detects_mhd_export() {
        assert!(Mhd::detect(SAMPLE));
    }

    #[test]
    fn detects_mhd_export_with_bom() {
        assert!(Mhd::detect(&format!("\u{FEFF}{}", SAMPLE)));
    }

    #[test]
    fn detects_via_tune_column_without_ecu_markers() {
        let contents = "#Encoding: UTF-8\n\
            Time,RPM (RPM),MHD 5.26 IJE0S ST2V7.bin\n\
            0,1000,0\n";
        assert!(Mhd::detect(contents));
    }

    #[test]
    fn rejects_other_time_prefixed_csv() {
        // RomRaider/ME221-style exports have no MHD fingerprint.
        assert!(!Mhd::detect("Time,RPM,Load\n0,1000,50\n"));
        // A generic commented CSV is not MHD either.
        assert!(!Mhd::detect("#Some tool\nTime,RPM\n0,1000\n"));
        // Haltech
        assert!(!Mhd::detect("%DataLog%\nDataLogVersion : 1.1\n"));
    }

    #[test]
    fn parses_channel_names_and_units() {
        let channel = MhdChannel::from_header("Boost (Bar)");
        assert_eq!(channel.name, "Boost");
        assert_eq!(channel.unit, "Bar");

        // MHD writes '*' for the degree sign
        let coolant = MhdChannel::from_header("Coolant (*C)");
        assert_eq!(coolant.name, "Coolant");
        assert_eq!(coolant.unit, "°C");

        let timing = MhdChannel::from_header("Cyl1 Timing Cor (*)");
        assert_eq!(timing.name, "Cyl1 Timing Cor");
        assert_eq!(timing.unit, "°");

        let crank = MhdChannel::from_header("Timing Cyl. 1 (*CRK)");
        assert_eq!(crank.name, "Timing Cyl. 1");
        assert_eq!(crank.unit, "°CRK");

        // '-' marks a unitless channel
        let gear = MhdChannel::from_header("Gear (-)");
        assert_eq!(gear.name, "Gear");
        assert_eq!(gear.unit, "");

        // Only the trailing parenthesized group is the unit
        let maf = MhdChannel::from_header("MAF Req (wgdc) (g/s)");
        assert_eq!(maf.name, "MAF Req (wgdc)");
        assert_eq!(maf.unit, "g/s");

        // No parentheses at all
        let tune = MhdChannel::from_header("MHD 5.26 IJE0S ST2V7.bin");
        assert_eq!(tune.name, "MHD 5.26 IJE0S ST2V7.bin");
        assert_eq!(tune.unit, "");
    }

    #[test]
    fn parses_sample_log() {
        let log = Mhd.parse(SAMPLE).expect("parse failed");

        assert_eq!(log.channels.len(), 6);
        assert_eq!(log.channels[0].name(), "Boost");
        assert_eq!(log.channels[0].unit(), "Bar");
        assert_eq!(log.channels[4].name(), "RPM");
        assert_eq!(log.times.len(), 3);
        assert_eq!(log.data.len(), 3);

        // Times are relative to the first sample
        assert!((log.times[0]).abs() < 1e-9);
        assert!((log.times[1] - 0.135).abs() < 1e-6);
        assert!((log.times[2] - 0.26).abs() < 1e-6);

        // Values pass through unscaled
        assert!((log.data[0][0].as_f64() + 0.479).abs() < 1e-9);
        assert_eq!(log.data[2][4].as_f64(), 1372.0);

        match log.meta {
            Meta::Mhd(meta) => {
                assert_eq!(meta.encoding, "UTF-8");
                assert_eq!(meta.ecu_prgid, "7616431");
                assert_eq!(meta.vin, "TESTVIN0000000000");
                assert_eq!(meta.tune, "MHD 5.26 IJE0S ST2V7.bin");
                assert_eq!(meta.channel_count, 6);
                assert_eq!(meta.data_points, 3);
            }
            other => panic!("expected MHD metadata, got {:?}", other),
        }
    }

    #[test]
    fn carries_last_known_value_over_unparseable_fields() {
        let contents = "#Ecu PRGID: 1\n\
            Time,RPM (RPM),Boost (Bar)\n\
            0,1000,0.5\n\
            0.1,,0.6\n\
            0.2,1200,N/A\n";

        let log = Mhd.parse(contents).expect("parse failed");
        assert_eq!(log.data.len(), 3);
        // Blank RPM keeps the previous sample
        assert_eq!(log.data[1][0].as_f64(), 1000.0);
        // Unparseable boost keeps the previous sample
        assert!((log.data[2][1].as_f64() - 0.6).abs() < 1e-9);
    }

    #[test]
    fn drops_out_of_order_and_short_rows() {
        let contents = "#Ecu PRGID: 1\n\
            Time,RPM (RPM),Boost (Bar)\n\
            0,1000,0.5\n\
            0.2,1100,0.6\n\
            0.1,1200,0.7\n\
            0.3,1300\n\
            0.4,1400,0.8\n";

        let log = Mhd.parse(contents).expect("parse failed");
        assert_eq!(log.times.len(), 3);
        assert!(log.times.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(log.data[2][0].as_f64(), 1400.0);
    }

    #[test]
    fn rejects_header_without_time_column() {
        let contents = "#Ecu PRGID: 1\nRPM,Boost\n1000,0.5\n";
        assert!(Mhd.parse(contents).is_err());
    }
}
