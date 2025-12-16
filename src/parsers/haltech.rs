use regex::Regex;
use serde::Serialize;
use std::error::Error;
use std::str::FromStr;
use strum::{AsRefStr, EnumString};

use super::types::{Channel, Log, Meta, Parseable, Value};

/// Haltech channel types
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

/// Haltech log file parser
pub struct Haltech;

impl Parseable for Haltech {
    fn parse(&self, file_contents: &str) -> Result<Log, Box<dyn Error>> {
        let mut meta = HaltechMeta::default();
        let mut channels: Vec<Channel> = vec![];
        let mut times: Vec<String> = vec![];
        let mut data: Vec<Vec<Value>> = vec![];

        let regex =
            Regex::new(r"(?<name>.+) : (?<value>.+)").expect("Failed to compile regex");

        let mut current_channel = HaltechChannel::default();

        for line in file_contents.lines() {
            let line = line.trim();

            // Try to parse as key-value pair (metadata or channel definition)
            if let Some(captures) = regex.captures(line) {
                let name = captures["name"].trim();
                let value = captures["value"].trim().to_string();

                match name {
                    "DataLogVersion" => meta.data_log_version = value,
                    "Software" => meta.software = value,
                    "SoftwareVersion" => meta.software_version = value,
                    "DownloadDate/Time" => meta.download_date_time = value,
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
                            tracing::warn!("Failed to parse channel type: {}", value);
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
            } else {
                // Not a key-value pair - must be CSV data
                // First, push any pending channel
                if !current_channel.name.is_empty() {
                    channels.push(Channel::Haltech(current_channel));
                    current_channel = HaltechChannel::default();
                }

                // Parse CSV data row
                if !line.is_empty() && !channels.is_empty() {
                    let values: Vec<Value> = line
                        .split(',')
                        .enumerate()
                        .filter_map(|(i, v)| {
                            let v = v.trim();
                            // First column is timestamp
                            if i == 0 {
                                times.push(v.to_string());
                                return None;
                            }

                            // Parse as integer (Haltech uses integer values)
                            v.parse::<i64>().ok().map(Value::Int)
                        })
                        .collect();

                    if values.len() >= channels.len() {
                        data.push(values);
                    }
                }
            }
        }

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
    fn test_parse_haltech_metadata() {
        let sample = r#"
DataLogVersion : 3
Software : Haltech ESP
SoftwareVersion : 2.0.0
DownloadDate/Time : 2024-01-15 10:30:00
Log Source : NSP
Log Number : 42
Log : 2024-01-15 10:00:00
Channel : RPM
ID : 1
Type : EngineSpeed
DisplayMaxMin : 10000,0
Channel : AFR
ID : 2
Type : AFR
DisplayMaxMin : 20,10
0.000,5000,14
0.001,5100,15
0.002,5200,14
"#;

        let parser = Haltech;
        let log = parser.parse(sample).unwrap();

        assert_eq!(log.channels.len(), 2);
        assert_eq!(log.channels[0].name(), "RPM");
        assert_eq!(log.channels[1].name(), "AFR");
        assert_eq!(log.times.len(), 3);
        assert_eq!(log.data.len(), 3);
    }
}
