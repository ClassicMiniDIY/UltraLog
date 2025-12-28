use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;
use std::error::Error;
use std::str::FromStr;
use std::sync::LazyLock;
use strum::{AsRefStr, EnumString};

/// Pre-compiled regex for detecting data rows (timestamp pattern)
static TIMESTAMP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{1,2}:\d{2}:\d{2}").expect("Invalid timestamp regex"));

use super::types::{Channel, Log, Meta, Parseable, Value};

/// Haltech channel types - comprehensive list from actual log files
#[derive(AsRefStr, Clone, Debug, EnumString, Serialize, Default)]
pub enum ChannelType {
    AFR,
    AbsPressure,
    Acceleration,
    Angle,
    AngularVelocity,
    BatteryVoltage,
    BoostToFuelFlowRate,
    ByteCount,
    Current,
    #[strum(serialize = "Current_uA_as_mA")]
    CurrentMicroampsAsMilliamps,
    #[strum(serialize = "Current_mA_as_A")]
    CurrentMilliampsAsAmps,
    Decibel,
    Density,
    DrivenDistance,
    EngineSpeed,
    EngineVolume,
    Flow,
    Frequency,
    #[strum(serialize = "FuelEcomony")]
    FuelEconomy,
    FuelVolume,
    Gear,
    GearRatio,
    InjFuelVolume,
    MassOverTime,
    #[strum(serialize = "MassPerCyl")]
    MassPerCylinder,
    Mileage,
    PercentPerEngineCycle,
    PercentPerLambda,
    #[strum(serialize = "PercentPerRpm")]
    PercentPerRPM,
    Percentage,
    Pressure,
    PulsesPerLongDistance,
    Ratio,
    #[default]
    Raw,
    Resistance,
    Speed,
    Stoichiometry,
    Temperature,
    #[strum(serialize = "Time_us")]
    TimeMicroseconds,
    #[strum(serialize = "TimeUsAsUs")]
    TimeMicrosecondsAsMicroseconds,
    #[strum(serialize = "Time_ms_as_s")]
    TimeMillisecondsAsSeconds,
    #[strum(serialize = "Time_ms")]
    TimeMilliseconds,
    #[strum(serialize = "Time_s")]
    TimeSeconds,
}

impl ChannelType {
    /// Convert raw value to human-readable unit based on Haltech CAN protocol spec
    /// Reference: <https://support.haltech.com/portal/en/kb/articles/haltech-can-ecu-broadcast-protocol>
    pub fn convert_value(&self, raw: f64) -> f64 {
        match self {
            // RPM: y = x (no conversion)
            ChannelType::EngineSpeed => raw,

            // Absolute Pressure: y = x/10 (kPa absolute)
            ChannelType::AbsPressure => raw / 10.0,

            // Gauge Pressure: y = x/10 - 101.3 (kPa gauge, subtract atmospheric)
            // Used for: Coolant Pressure, Fuel Pressure, Oil Pressure, Wastegate Pressure
            ChannelType::Pressure => raw / 10.0 - 101.3,

            // Percentage: y = x/10
            ChannelType::Percentage
            | ChannelType::PercentPerEngineCycle
            | ChannelType::PercentPerLambda
            | ChannelType::PercentPerRPM => raw / 10.0,

            // Angle: y = x/10 (degrees)
            ChannelType::Angle => raw / 10.0,

            // Battery Voltage: y = x/1000 (Volts)
            // Note: CSV export uses millivolts, not the CAN protocol's decavolts
            ChannelType::BatteryVoltage => raw / 1000.0,

            // Temperature: y = x/10 (Kelvin)
            // To convert to Celsius: subtract 273.15 after
            ChannelType::Temperature => raw / 10.0,

            // Speed: y = x/10 (km/h)
            ChannelType::Speed => raw / 10.0,

            // Lambda/AFR: y = x/1000 (λ)
            ChannelType::AFR => raw / 1000.0,

            // Knock Level (Decibel): y = x/100 (dB)
            ChannelType::Decibel => raw / 100.0,

            // Injection Time (microseconds): y = x/1000 (convert to ms)
            ChannelType::TimeMicroseconds => raw / 1000.0,

            // Time in microseconds displayed as microseconds (no conversion)
            ChannelType::TimeMicrosecondsAsMicroseconds => raw,

            // Time milliseconds: y = x (ms)
            ChannelType::TimeMilliseconds => raw,

            // Time milliseconds as seconds: y = x/1000
            ChannelType::TimeMillisecondsAsSeconds => raw / 1000.0,

            // Time seconds: y = x
            ChannelType::TimeSeconds => raw,

            // Acceleration: y = x/10 (m/s²)
            ChannelType::Acceleration => raw / 10.0,

            // Angular velocity: y = x/10 (deg/s)
            ChannelType::AngularVelocity => raw / 10.0,

            // Current types
            ChannelType::Current => raw / 1000.0, // mA to A
            ChannelType::CurrentMicroampsAsMilliamps => raw / 1000.0, // μA to mA
            ChannelType::CurrentMilliampsAsAmps => raw / 1000.0, // mA to A

            // Density: y = x/10 (g/m³)
            ChannelType::Density => raw / 10.0,

            // Flow: y = x (cc/min)
            ChannelType::Flow => raw,

            // Frequency: y = x (Hz)
            ChannelType::Frequency => raw,

            // Fuel Economy: y = x/10
            ChannelType::FuelEconomy => raw / 10.0,

            // Fuel Volume: y = x/10 (L)
            ChannelType::FuelVolume => raw / 10.0,

            // Gear: y = x (no conversion)
            ChannelType::Gear => raw,

            // Gear Ratio: y = x/100
            ChannelType::GearRatio => raw / 100.0,

            // Ratio: y = x/100
            ChannelType::Ratio => raw / 100.0,

            // Resistance: y = x (Ohms)
            ChannelType::Resistance => raw,

            // Stoichiometry: y = x/100
            ChannelType::Stoichiometry => raw / 100.0,

            // Distance types
            ChannelType::DrivenDistance | ChannelType::Mileage => raw,

            // Volume types
            ChannelType::EngineVolume | ChannelType::InjFuelVolume => raw,

            // Mass types
            ChannelType::MassOverTime | ChannelType::MassPerCylinder => raw,

            // Other specialized types
            ChannelType::BoostToFuelFlowRate => raw,
            ChannelType::ByteCount => raw,
            ChannelType::PulsesPerLongDistance => raw,

            // Raw/Unknown: no conversion
            ChannelType::Raw => raw,
        }
    }

    /// Get the display unit string for this channel type
    pub fn unit(&self) -> &'static str {
        match self {
            ChannelType::EngineSpeed => "RPM",
            ChannelType::AbsPressure => "kPa",
            ChannelType::Pressure => "kPa",
            ChannelType::Percentage
            | ChannelType::PercentPerEngineCycle
            | ChannelType::PercentPerLambda
            | ChannelType::PercentPerRPM => "%",
            ChannelType::Angle => "°",
            ChannelType::BatteryVoltage => "V",
            ChannelType::Temperature => "K",
            ChannelType::Speed => "km/h",
            ChannelType::AFR => "λ",
            ChannelType::Decibel => "dB",
            ChannelType::TimeMicroseconds => "ms",
            ChannelType::TimeMicrosecondsAsMicroseconds => "μs",
            ChannelType::TimeMilliseconds => "ms",
            ChannelType::TimeMillisecondsAsSeconds => "s",
            ChannelType::TimeSeconds => "s",
            ChannelType::Acceleration => "m/s²",
            ChannelType::AngularVelocity => "°/s",
            ChannelType::Current | ChannelType::CurrentMilliampsAsAmps => "A",
            ChannelType::CurrentMicroampsAsMilliamps => "mA",
            ChannelType::Density => "g/m³",
            ChannelType::Flow => "cc/min",
            ChannelType::Frequency => "Hz",
            ChannelType::FuelEconomy => "L/100km",
            ChannelType::FuelVolume => "L",
            ChannelType::Gear => "",
            ChannelType::GearRatio | ChannelType::Ratio => "",
            ChannelType::Resistance => "Ω",
            ChannelType::Stoichiometry => "",
            ChannelType::DrivenDistance | ChannelType::Mileage => "km",
            ChannelType::EngineVolume | ChannelType::InjFuelVolume => "cc",
            ChannelType::MassOverTime => "g/s",
            ChannelType::MassPerCylinder => "mg",
            _ => "",
        }
    }
}

/// Haltech log file metadata
#[derive(Clone, Debug, Default, Serialize)]
pub struct HaltechMeta {
    pub data_log_version: String,
    pub software: String,
    pub software_version: String,
    pub download_date_time: String,
    pub log_source: String,
    pub log_number: String,
    pub log_date_time: String,
}

/// Haltech channel definition
#[derive(Clone, Debug, Default, Serialize)]
pub struct HaltechChannel {
    pub name: String,
    pub id: String,
    pub r#type: ChannelType,
    pub display_min: Option<f64>,
    pub display_max: Option<f64>,
}

impl HaltechChannel {
    /// Get the display unit for this channel
    pub fn unit(&self) -> &'static str {
        self.r#type.unit()
    }
}

/// Haltech log file parser
pub struct Haltech;

impl Haltech {
    /// Parse timestamp from HH:MM:SS.mmm format to seconds
    fn parse_timestamp(timestamp: &str) -> Option<f64> {
        // Format: "HH:MM:SS.mmm" e.g., "14:15:46.000"
        let parts: Vec<&str> = timestamp.split(':').collect();
        if parts.len() != 3 {
            return None;
        }

        let hours: f64 = parts[0].parse().ok()?;
        let minutes: f64 = parts[1].parse().ok()?;

        // Seconds may include milliseconds
        let seconds: f64 = parts[2].parse().ok()?;

        Some(hours * 3600.0 + minutes * 60.0 + seconds)
    }

    /// Check if a line looks like a data row (starts with timestamp)
    fn is_data_row(line: &str) -> bool {
        // Data rows start with HH:MM:SS pattern - uses pre-compiled static regex
        TIMESTAMP_REGEX.is_match(line)
    }
}

impl Parseable for Haltech {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        let mut meta = HaltechMeta::default();
        let mut channels: Vec<Channel> = Vec::with_capacity(50); // Typical log has ~20-50 channels

        // Regex for key-value pairs like "Key : Value"
        let kv_regex =
            Regex::new(r"^(?<name>[^:]+?)\s*:\s*(?<value>.+)$").expect("Failed to compile regex");

        let mut current_channel = HaltechChannel::default();
        let mut data_lines: Vec<&str> = Vec::new();

        // Phase 1: Parse metadata and channels, collect data lines
        for line in file_contents.lines() {
            let line = line.trim();

            // Skip empty lines and header marker
            if line.is_empty() || line == "%DataLog%" {
                continue;
            }

            // Check if this is a data row
            if Self::is_data_row(line) {
                // Push any pending channel before collecting data lines
                if !current_channel.name.is_empty() {
                    channels.push(Channel::Haltech(current_channel));
                    current_channel = HaltechChannel::default();
                }

                // Collect data line for parallel processing
                data_lines.push(line);
                continue;
            }

            // Not in data section yet - parse metadata and channel definitions
            if data_lines.is_empty() {
                if let Some(captures) = kv_regex.captures(line) {
                    let name = captures["name"].trim();
                    let value = captures["value"].trim().to_string();

                    match name {
                        "DataLogVersion" => meta.data_log_version = value,
                        "Software" => meta.software = value,
                        "SoftwareVersion" => meta.software_version = value,
                        "DownloadDateTime" | "DownloadDate/Time" => meta.download_date_time = value,
                        "Log Source" => meta.log_source = value,
                        "Log Number" => meta.log_number = value,
                        "Log" => meta.log_date_time = value,
                        // "Channel" key indicates start of a new channel definition
                        "Channel" => {
                            if !current_channel.name.is_empty() {
                                channels.push(Channel::Haltech(current_channel));
                            }
                            current_channel = HaltechChannel::default();
                            current_channel.name = value;
                        }
                        "ID" => current_channel.id = value,
                        "Type" => {
                            if let Ok(channel_type) = ChannelType::from_str(&value) {
                                current_channel.r#type = channel_type;
                            } else {
                                tracing::warn!("Unknown channel type: {}", value);
                                current_channel.r#type = ChannelType::Raw;
                            }
                        }
                        "DisplayMaxMin" => {
                            let values: Vec<&str> = value.split(',').collect();
                            if values.len() >= 2 {
                                current_channel.display_max = values[0].trim().parse().ok();
                                current_channel.display_min = values[1].trim().parse().ok();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Phase 2: Parse data rows in parallel
        // Each row is parsed independently, returning (timestamp, values)
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

                // Parse remaining values and apply unit conversions
                let values: Vec<Value> = parts[1..]
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, v)| {
                        let v = v.trim();
                        let raw_value: f64 = v.parse().ok()?;

                        // Apply conversion based on channel type if available
                        let converted = if let Some(Channel::Haltech(ch)) = channels.get(idx) {
                            ch.r#type.convert_value(raw_value)
                        } else {
                            raw_value
                        };
                        Some(Value::Float(converted))
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

        // Verify data integrity
        let channel_count = channels.len();
        if channel_count > 0 {
            // Filter out data rows that don't match channel count
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
            "Parsed Haltech log: {} channels, {} data points",
            channels.len(),
            data.len()
        );

        Ok(Log {
            meta: Meta::Haltech(meta),
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
    fn test_parse_timestamp() {
        assert_eq!(Haltech::parse_timestamp("00:00:00.000"), Some(0.0));
        assert_eq!(Haltech::parse_timestamp("00:01:00.000"), Some(60.0));
        assert_eq!(Haltech::parse_timestamp("01:00:00.000"), Some(3600.0));
        assert_eq!(Haltech::parse_timestamp("14:15:46.000"), Some(51346.0));
        assert_eq!(Haltech::parse_timestamp("14:15:46.500"), Some(51346.5));
    }

    #[test]
    fn test_parse_haltech_log() {
        let sample = r#"%DataLog%
DataLogVersion : 1.1
Software : Haltech NSP
SoftwareVersion : 999.999.999.999
DownloadDateTime : 20250718 04:09:48
Channel : RPM
ID : 384
Type : EngineSpeed
DisplayMaxMin : 20000,0
Channel : Manifold Pressure
ID : 224
Type : Pressure
DisplayMaxMin : 4013,13
Log Source : 20
Log Number : 1118
Log : 20250718 02:15:46
14:15:46.000,5000,1013
14:15:46.020,5100,1020
14:15:46.040,5200,1030
"#;

        let parser = Haltech;
        let log = parser.parse(sample).unwrap();

        assert_eq!(log.channels.len(), 2);
        assert_eq!(log.channels[0].name(), "RPM");
        assert_eq!(log.channels[1].name(), "Manifold Pressure");
        assert_eq!(log.times.len(), 3);
        assert_eq!(log.data.len(), 3);

        // Check relative timestamps (now stored as f64)
        assert!((log.times[0] - 0.0).abs() < 0.001);
        assert!((log.times[1] - 0.020).abs() < 0.001);
        assert!((log.times[2] - 0.040).abs() < 0.001);

        // Check unit conversions are applied
        // RPM: y = x (no conversion) - raw 5000 -> 5000 RPM
        assert_eq!(log.data[0][0].as_f64(), 5000.0);

        // Pressure: y = x/10 - 101.3 (gauge kPa) - raw 1013 -> 0.0 kPa
        let pressure_value = log.data[0][1].as_f64();
        assert!(
            (pressure_value - 0.0).abs() < 0.01,
            "Expected ~0.0, got {}",
            pressure_value
        );

        // Check units
        assert_eq!(log.channels[0].unit(), "RPM");
        assert_eq!(log.channels[1].unit(), "kPa");
    }

    #[test]
    fn test_channel_type_conversions() {
        // RPM: no conversion
        assert_eq!(ChannelType::EngineSpeed.convert_value(5000.0), 5000.0);

        // Absolute Pressure: y = x/10
        assert_eq!(ChannelType::AbsPressure.convert_value(1013.0), 101.3);

        // Gauge Pressure: y = x/10 - 101.3
        assert!((ChannelType::Pressure.convert_value(1013.0) - 0.0).abs() < 0.01);
        assert!((ChannelType::Pressure.convert_value(2013.0) - 100.0).abs() < 0.01);

        // Percentage: y = x/10
        assert_eq!(ChannelType::Percentage.convert_value(500.0), 50.0);
        assert_eq!(ChannelType::Percentage.convert_value(1000.0), 100.0);

        // Angle: y = x/10
        assert_eq!(ChannelType::Angle.convert_value(150.0), 15.0);
        assert_eq!(ChannelType::Angle.convert_value(-300.0), -30.0);

        // Battery Voltage: y = x/1000 (CSV uses millivolts)
        assert_eq!(ChannelType::BatteryVoltage.convert_value(14000.0), 14.0);

        // Temperature: y = x/10 (Kelvin)
        assert_eq!(ChannelType::Temperature.convert_value(2931.0), 293.1);

        // Speed: y = x/10
        assert_eq!(ChannelType::Speed.convert_value(1200.0), 120.0);

        // Lambda/AFR: y = x/1000
        assert_eq!(ChannelType::AFR.convert_value(1000.0), 1.0);
        assert_eq!(ChannelType::AFR.convert_value(850.0), 0.85);

        // Knock dB: y = x/100
        assert_eq!(ChannelType::Decibel.convert_value(2500.0), 25.0);

        // Time microseconds to ms: y = x/1000
        assert_eq!(ChannelType::TimeMicroseconds.convert_value(5000.0), 5.0);
    }

    #[test]
    fn test_is_data_row() {
        assert!(Haltech::is_data_row("14:15:46.000,5000,1013"));
        assert!(Haltech::is_data_row("0:00:00.000,100,200"));
        assert!(!Haltech::is_data_row("Channel : RPM"));
        assert!(!Haltech::is_data_row("ID : 384"));
        assert!(!Haltech::is_data_row("%DataLog%"));
    }
}
