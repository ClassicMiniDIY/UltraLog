//! ECUMaster EMU Pro log file parser.
//!
//! Parses CSV log files exported from ECUMaster EMU Pro ECUs.
//! Format: Semicolon-delimited CSV with hierarchical channel names.

use serde::Serialize;
use std::error::Error;

use super::types::{Channel, Log, Meta, Parseable, Value};

/// ECUMaster log file metadata
#[derive(Clone, Debug, Default, Serialize)]
pub struct EcuMasterMeta {
    /// Number of channels in the log
    pub channel_count: usize,
    /// Number of data points
    pub data_points: usize,
}

/// ECUMaster channel definition
#[derive(Clone, Debug, Default, Serialize)]
pub struct EcuMasterChannel {
    /// Full channel path (e.g., "engine/rpm")
    pub path: String,
    /// Display name (last segment of path)
    pub name: String,
    /// Inferred unit based on channel name
    pub unit: String,
}

impl EcuMasterChannel {
    /// Create a new channel from a path string
    pub fn from_path(path: &str) -> Self {
        let path = path.trim().to_string();

        // Extract display name (last segment of path)
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();

        // Infer unit from channel name
        let unit = Self::infer_unit(&path, &name);

        Self { path, name, unit }
    }

    /// Infer the unit based on channel path and name
    fn infer_unit(path: &str, name: &str) -> String {
        let path_lower = path.to_lowercase();
        let name_lower = name.to_lowercase();

        // Temperature channels
        if path_lower.contains("temp") || path_lower.contains("temperature") {
            return "°C".to_string();
        }

        // Pressure channels
        if path_lower.contains("pressure")
            || name_lower == "map"
            || path_lower.contains("/map")
            || path_lower.contains("baro")
            || path_lower.contains("boost")
        {
            return "kPa".to_string();
        }

        // RPM channels (check before "rpm" substring matches)
        if name_lower == "rpm"
            || name_lower.ends_with("rpm")
            || name_lower.contains("rpmtarget")
            || name_lower.contains("rpmmatch")
        {
            return "RPM".to_string();
        }

        // Throttle/TPS and DBW target channels
        if name_lower.contains("tps")
            || name_lower.contains("throttle")
            || path_lower.contains("throttle")
            || (path_lower.contains("dbw") && name_lower == "target")
        {
            return "%".to_string();
        }

        // Percentage channels (including torque estimation percent)
        if name_lower.contains("percent")
            || name_lower.contains("duty")
            || path_lower.contains("percent")
            || name_lower.contains("correction")
            || (path_lower.contains("torqueestimation") && !name_lower.contains("torque"))
        {
            return "%".to_string();
        }

        // Ignition angle channels
        if name_lower == "angle"
            || (path_lower.contains("ignition") && name_lower.contains("angle"))
        {
            return "°".to_string();
        }

        // VVT/Cam angle channels
        if (path_lower.contains("vvt") || path_lower.contains("cam"))
            && (name_lower.contains("angle") || name_lower.contains("position"))
        {
            return "°".to_string();
        }

        // Voltage channels
        if name_lower.contains("voltage")
            || name_lower.contains("volt")
            || name_lower == "battery"
            || name_lower.contains("vbat")
        {
            return "V".to_string();
        }

        // Lambda/AFR channels
        if name_lower.contains("lambda") || name_lower.contains("afr") || name_lower == "o2" {
            return "λ".to_string();
        }

        // Speed channels
        if name_lower.contains("speed") && !name_lower.contains("rpm") {
            return "km/h".to_string();
        }

        // Gear channels (no unit)
        if name_lower.contains("gear") && !name_lower.contains("ratio") {
            return "".to_string();
        }

        // Torque channels
        if name_lower.contains("torque") {
            // Torque reduction/correction values are percentages
            if path_lower.contains("reduction") || path_lower.contains("percent") {
                return "%".to_string();
            }
            // Requested torque in Nm
            if name_lower.contains("requested") || name_lower == "torque" {
                return "Nm".to_string();
            }
            return "Nm".to_string();
        }

        // Timer/state timer channels
        if name_lower.contains("timer") || name_lower.contains("statetimer") {
            return "s".to_string();
        }

        // Flow channels
        if name_lower.contains("flow") {
            return "cc/min".to_string();
        }

        // Default: no unit
        "".to_string()
    }

    /// Get the display unit for this channel
    pub fn unit(&self) -> &str {
        &self.unit
    }
}

/// ECUMaster log file parser
pub struct EcuMaster;

impl EcuMaster {
    /// Detect if file contents look like an ECUMaster log
    pub fn detect(contents: &str) -> bool {
        // ECUMaster logs are semicolon-delimited and start with TIME
        if let Some(first_line) = contents.lines().next() {
            first_line.starts_with("TIME;") || first_line.starts_with("TIME\t")
        } else {
            false
        }
    }
}

impl Parseable for EcuMaster {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        // Pre-allocate based on estimated row count (first line is header)
        let line_count = file_contents.lines().count();
        let estimated_data_rows = line_count.saturating_sub(1);

        let mut channels: Vec<Channel> = Vec::with_capacity(50);
        let mut times: Vec<f64> = Vec::with_capacity(estimated_data_rows);
        let mut data: Vec<Vec<Value>> = Vec::with_capacity(estimated_data_rows);

        let mut lines = file_contents.lines();

        // Parse header line to get channel names
        let header = lines.next().ok_or("Empty file: no header found")?;

        // Determine delimiter (semicolon or tab)
        let delimiter = if header.contains(';') { ';' } else { '\t' };

        let column_names: Vec<&str> = header.split(delimiter).collect();

        if column_names.is_empty() || column_names[0].to_uppercase() != "TIME" {
            return Err("Invalid ECUMaster log: first column must be TIME".into());
        }

        // Create channels from header (skip TIME column)
        for name in column_names.iter().skip(1) {
            let channel = EcuMasterChannel::from_path(name);
            channels.push(Channel::EcuMaster(channel));
        }

        // Track last known values for sparse data interpolation
        let mut last_values: Vec<Option<f64>> = vec![None; channels.len()];

        // Parse data rows
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let parts: Vec<&str> = line.split(delimiter).collect();
            if parts.is_empty() {
                continue;
            }

            // First column is time (already in seconds)
            let time_str = parts[0].trim();
            if let Ok(time_val) = time_str.parse::<f64>() {
                times.push(time_val);

                // Parse remaining values (may be sparse/empty)
                let mut row_values: Vec<Value> = Vec::with_capacity(channels.len());

                for (idx, part) in parts.iter().skip(1).enumerate() {
                    let part = part.trim();

                    if part.is_empty() {
                        // Empty value - use last known value or 0
                        let value = last_values.get(idx).and_then(|v| *v).unwrap_or(0.0);
                        row_values.push(Value::Float(value));
                    } else if let Ok(val) = part.parse::<f64>() {
                        // Valid numeric value
                        if idx < last_values.len() {
                            last_values[idx] = Some(val);
                        }
                        row_values.push(Value::Float(val));
                    } else {
                        // Non-numeric value - use last known or 0
                        let value = last_values.get(idx).and_then(|v| *v).unwrap_or(0.0);
                        row_values.push(Value::Float(value));
                    }
                }

                // Pad row to match channel count if needed
                while row_values.len() < channels.len() {
                    let idx = row_values.len();
                    let value = last_values.get(idx).and_then(|v| *v).unwrap_or(0.0);
                    row_values.push(Value::Float(value));
                }

                data.push(row_values);
            }
        }

        tracing::info!(
            "Parsed ECUMaster log: {} channels, {} data points",
            channels.len(),
            data.len()
        );

        Ok(Log {
            meta: Meta::EcuMaster(EcuMasterMeta {
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
    fn test_channel_from_path() {
        let ch = EcuMasterChannel::from_path("engine/rpm");
        assert_eq!(ch.name, "rpm");
        assert_eq!(ch.path, "engine/rpm");
        assert_eq!(ch.unit, "RPM");

        let ch = EcuMasterChannel::from_path("sensors/tps1");
        assert_eq!(ch.name, "tps1");
        assert_eq!(ch.unit, "%");

        let ch = EcuMasterChannel::from_path("ignition/angle");
        assert_eq!(ch.name, "angle");
        assert_eq!(ch.unit, "°");
    }

    #[test]
    fn test_detect_ecumaster() {
        assert!(EcuMaster::detect(
            "TIME;engine/rpm;sensors/tps1\n0.000;1000;50"
        ));
        assert!(EcuMaster::detect(
            "TIME\tengine/rpm\tsensors/tps1\n0.000\t1000\t50"
        ));
        assert!(!EcuMaster::detect("%DataLog%\nSomething else"));
        assert!(!EcuMaster::detect("timestamp,rpm,tps"));
    }

    #[test]
    fn test_parse_ecumaster_log() {
        let sample = "TIME;engine/rpm;sensors/tps1;ignition/angle\n\
                      0.000;1000;10.5;15.0\n\
                      0.020;1050;;15.5\n\
                      0.040;1100;12.0;\n";

        let parser = EcuMaster;
        let log = parser.parse(sample).unwrap();

        assert_eq!(log.channels.len(), 3);
        assert_eq!(log.channels[0].name(), "rpm");
        assert_eq!(log.channels[1].name(), "tps1");
        assert_eq!(log.channels[2].name(), "angle");

        assert_eq!(log.times.len(), 3);
        assert_eq!(log.data.len(), 3);

        // Check first row
        assert_eq!(log.data[0][0].as_f64(), 1000.0);
        assert_eq!(log.data[0][1].as_f64(), 10.5);
        assert_eq!(log.data[0][2].as_f64(), 15.0);

        // Check sparse data handling (empty values use previous)
        assert_eq!(log.data[1][0].as_f64(), 1050.0);
        assert_eq!(log.data[1][1].as_f64(), 10.5); // Previous value
        assert_eq!(log.data[1][2].as_f64(), 15.5);

        // Check units
        assert_eq!(log.channels[0].unit(), "RPM");
        assert_eq!(log.channels[1].unit(), "%");
        assert_eq!(log.channels[2].unit(), "°");
    }

    #[test]
    fn test_unit_inference() {
        // Temperature
        assert_eq!(
            EcuMasterChannel::infer_unit("sensors/coolantTemp", "coolantTemp"),
            "°C"
        );

        // Pressure
        assert_eq!(EcuMasterChannel::infer_unit("sensors/map", "map"), "kPa");

        // Lambda
        assert_eq!(
            EcuMasterChannel::infer_unit("sensors/lambda1", "lambda1"),
            "λ"
        );

        // Voltage
        assert_eq!(
            EcuMasterChannel::infer_unit("sensors/batteryVoltage", "batteryVoltage"),
            "V"
        );
    }
}
