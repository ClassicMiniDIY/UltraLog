use chrono::NaiveDateTime;
use rayon::prelude::*;
use serde::Serialize;
use std::error::Error;

use super::types::{Channel, Log, Meta, Parseable, Value};

/// Locomotive log file metadata
#[derive(Clone, Debug, Default, Serialize)]
pub struct LocomotiveMeta {
    pub timestamp: String,
    pub customer: String,
    pub unit_number: String,
    pub software_part_number: String,
    pub software_version: String,
}

/// Locomotive channel definition - simple name and unit storage
#[derive(Clone, Debug, Default, Serialize)]
pub struct LocomotiveChannel {
    pub name: String,
}

impl LocomotiveChannel {
    /// Get the display unit for this channel (no units in locomotive logs)
    pub fn unit(&self) -> &'static str {
        ""
    }
}

/// Locomotive log file parser
pub struct Locomotive;

impl Locomotive {
    /// Detect if content is a locomotive CSV log
    pub fn detect(contents: &str) -> bool {
        // Locomotive logs start with "TimeStamp: " followed by a date,
        // then "Customer:", "UnitNumber:", etc.
        let mut lines = contents.lines();

        // Check first line starts with "TimeStamp: "
        if let Some(first_line) = lines.next() {
            if first_line.trim().starts_with("TimeStamp:") {
                // Check second line has "Customer:"
                if let Some(second_line) = lines.next() {
                    return second_line.trim().starts_with("Customer:");
                }
            }
        }
        false
    }

    /// Parse datetime timestamp to seconds since epoch
    /// Format: "Sat Nov 15 19:00:03 2025"
    fn parse_timestamp(timestamp: &str) -> Option<f64> {
        // Parse format: "Sat Nov 15 19:00:03 2025"
        // We'll use chrono to handle the datetime parsing
        let datetime = NaiveDateTime::parse_from_str(timestamp, "%a %b %d %H:%M:%S %Y").ok()?;
        Some(datetime.and_utc().timestamp() as f64)
    }

    /// Check if a line is a data row (starts with day of week)
    fn is_data_row(line: &str) -> bool {
        line.starts_with("Mon ")
            || line.starts_with("Tue ")
            || line.starts_with("Wed ")
            || line.starts_with("Thu ")
            || line.starts_with("Fri ")
            || line.starts_with("Sat ")
            || line.starts_with("Sun ")
    }
}

impl Parseable for Locomotive {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        let mut meta = LocomotiveMeta::default();
        let mut channels: Vec<Channel> = Vec::new();
        let mut data_lines: Vec<&str> = Vec::new();
        let mut in_header = true;
        let mut header_line_count = 0;

        // Phase 1: Parse metadata and channel headers, collect data lines
        for line in file_contents.lines() {
            let line = line.trim();

            // Skip empty lines
            if line.is_empty() {
                continue;
            }

            // Check if this is a data row
            if Self::is_data_row(line) {
                in_header = false;
                data_lines.push(line);
                continue;
            }

            // Parse metadata header (first 5 non-empty lines)
            if in_header && header_line_count < 5 {
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim();
                    let value = value.trim();
                    match key {
                        "TimeStamp" => meta.timestamp = value.to_string(),
                        "Customer" => meta.customer = value.to_string(),
                        "UnitNumber" => meta.unit_number = value.to_string(),
                        "SoftwarePartNumber" => meta.software_part_number = value.to_string(),
                        "SoftwareVersion" => meta.software_version = value.to_string(),
                        _ => {}
                    }
                    header_line_count += 1;
                }
            } else if in_header && header_line_count == 5 {
                // This should be the column header line
                // Parse channel names from comma-separated header
                let channel_names: Vec<&str> = line.split(',').map(|s| s.trim()).collect();

                // Skip first column (TimeStamp)
                for name in channel_names.iter().skip(1) {
                    if !name.is_empty() {
                        channels.push(Channel::Locomotive(LocomotiveChannel {
                            name: name.to_string(),
                        }));
                    }
                }
                header_line_count += 1;
            }
        }

        // Phase 2: Parse data rows in parallel
        let parsed_rows: Vec<(f64, Vec<Value>)> = data_lines
            .par_iter()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.is_empty() {
                    return None;
                }

                // First column is timestamp
                let timestamp_str = parts[0].trim();
                let timestamp_secs = Self::parse_timestamp(timestamp_str)?;

                // Parse remaining values as f64
                let values: Vec<Value> = parts[1..]
                    .iter()
                    .filter_map(|v| {
                        let v = v.trim();
                        if v.is_empty() {
                            None
                        } else {
                            v.parse::<f64>().ok().map(Value::Float)
                        }
                    })
                    .collect();

                if values.is_empty() {
                    None
                } else {
                    Some((timestamp_secs, values))
                }
            })
            .collect();

        // Phase 3: Post-process results (sequential for ordering)
        let data_count = parsed_rows.len();
        let mut times: Vec<f64> = Vec::with_capacity(data_count);
        let mut data: Vec<Vec<Value>> = Vec::with_capacity(data_count);

        if !parsed_rows.is_empty() {
            // First timestamp is the base for relative times
            let first_timestamp = parsed_rows[0].0;

            for (timestamp, values) in parsed_rows {
                times.push(timestamp - first_timestamp);
                data.push(values);
            }
        }

        // Verify data integrity - filter out rows that don't match channel count
        let channel_count = channels.len();
        if channel_count > 0 {
            let mut filtered_times = Vec::with_capacity(times.len());
            let mut filtered_data = Vec::with_capacity(data.len());
            for (time, row) in times.into_iter().zip(data.into_iter()) {
                if row.len() >= channel_count {
                    filtered_times.push(time);
                    filtered_data.push(row);
                }
            }
            times = filtered_times;
            data = filtered_data;
        }

        tracing::info!(
            "Parsed Locomotive log: {} channels, {} data points",
            channels.len(),
            data.len()
        );

        Ok(Log {
            meta: Meta::Locomotive(meta),
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
        let sample = r#"TimeStamp: Sat Nov 15 19:00:03 2025
Customer: VLi
UnitNumber: 6194"#;
        assert!(Locomotive::detect(sample));

        // Should not detect non-locomotive files
        assert!(!Locomotive::detect("TIME;RPM;Boost"));
        assert!(!Locomotive::detect("%DataLog%"));
        assert!(!Locomotive::detect("Time (msec),RPM"));
    }

    #[test]
    fn test_parse_timestamp() {
        // Test parsing locomotive datetime format
        let ts = Locomotive::parse_timestamp("Sat Nov 15 19:00:03 2025");
        assert!(ts.is_some());

        // Verify it's a reasonable timestamp (year 2025)
        let ts_val = ts.unwrap();
        assert!(ts_val > 1700000000.0); // After 2023
        assert!(ts_val < 2000000000.0); // Before 2033
    }

    #[test]
    fn test_is_data_row() {
        assert!(Locomotive::is_data_row("Sat Nov 15 19:00:03 2025, 1, 2, 3"));
        assert!(Locomotive::is_data_row(
            "Mon Jan 01 00:00:00 2024, 100, 200"
        ));
        assert!(!Locomotive::is_data_row(
            "TimeStamp: Sat Nov 15 19:00:03 2025"
        ));
        assert!(!Locomotive::is_data_row("Customer: VLi"));
        assert!(!Locomotive::is_data_row("TimeStamp, CPMRst, Rc_tfnd"));
    }

    #[test]
    fn test_parse_locomotive_log() {
        let sample = r#"TimeStamp: Sat Nov 15 19:00:03 2025
Customer: VLi
UnitNumber: 6194
SoftwarePartNumber: 16085
SoftwareVersion: 33.21.04

TimeStamp, CPMRst, Rc_tfnd, AB Mode
Sat Nov 15 19:00:03 2025, 1, 1, 1
Sat Nov 15 19:00:17 2025, 1, 1, 1
Sat Nov 15 19:00:22 2025, 1, 1, 1
"#;

        let parser = Locomotive;
        let log = parser.parse(sample).unwrap();

        assert_eq!(log.channels.len(), 3);
        assert_eq!(log.channels[0].name(), "CPMRst");
        assert_eq!(log.channels[1].name(), "Rc_tfnd");
        assert_eq!(log.channels[2].name(), "AB Mode");
        assert_eq!(log.times.len(), 3);
        assert_eq!(log.data.len(), 3);

        // Check relative timestamps
        assert!((log.times[0] - 0.0).abs() < 0.001);
        assert!((log.times[1] - 14.0).abs() < 1.0); // ~14 seconds later
        assert!((log.times[2] - 19.0).abs() < 1.0); // ~19 seconds later

        // Check data values
        assert_eq!(log.data[0][0].as_f64(), 1.0);
        assert_eq!(log.data[0][1].as_f64(), 1.0);
        assert_eq!(log.data[0][2].as_f64(), 1.0);
    }
}
