use std::str::FromStr;

use regex::Regex;
use serde::Serialize;
use strum::{AsRefStr, EnumString};

use crate::parsers::types::{Log, Parser};

#[derive(AsRefStr, Clone, Debug, EnumString, Serialize)]
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
  TimeSeconds,
  Ratio,
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
}

impl Default for ChannelType {
  fn default() -> Self { ChannelType::Raw }
}

#[derive(Clone, Debug, Serialize)]
pub enum ChannelValue {
  _Bool(bool),
  _Float(f64),
  _Int(i64),
  String(String),
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Meta {
  pub data_log_version: String,
  pub software: String,
  pub software_version: String,
  pub download_date_time: String,
  pub log_source: String,
  pub log_number: String,
  pub log_date_time: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Channel {
  pub name: String,
  pub id: String,
  pub r#type: ChannelType,
  pub display_min: Option<f64>,
  pub display_max: Option<f64>,
}

pub struct Haltech {}

impl Parser<Meta, Channel, Vec<ChannelValue>> for Haltech {
  fn parse(&self, file_contents: &str) -> Result<Log<Meta, Channel, Vec<ChannelValue>>, Box<dyn std::error::Error>> {
    let mut meta = Meta::default();
    let mut channels = vec![];
    let mut data = vec![];

    let regex = Regex::new(r"(?<name>.+) : (?<value>.+)")
      .expect("Failed to compile regex");

    let mut current_channel = Channel::default();
    for line in file_contents.lines() {
      let line = line.trim();

      // Start by attempting to parse the line as a meta/channel key-value pair
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
          "Channel" => {
            if !current_channel.name.is_empty() {
              channels.push(current_channel);
            }

            current_channel = Channel::default();
            current_channel.name = value;
          }
          "Id" => current_channel.id = value,
          "Type" => current_channel.r#type = ChannelType::from_str(&value).unwrap(),
          "DisplayMaxMin" => {
            let values: Vec<&str> = value.split(",").collect();
            current_channel.display_max = values[0].parse().ok();
            current_channel.display_min = values[1].parse().ok();
          }
          _ => {}
        }
      } else {
        // This is not a key-value pair, so it must be channel data (CSV)
        if !line.is_empty() {
          let values = line
            .split(",")
            .enumerate()
            .map(|(i, v)| {
              // The first value is always a timestamp
              if i == 0 {
                return ChannelValue::String(v.to_string());
              }

              let channel_type = &channels[i - 1].r#type;
              match channel_type {
                _ => ChannelValue::String(v.to_string())
              }
            })
            .collect::<Vec<_>>();

          if channels.len() > 0 && values.len() >= channels.len() {
            data.push(values);
          }
        }
      }
    }

    Ok(Log {
      meta,
      channels,
      data,
    })
  }
}