use serde::Serialize;
use std::error::Error;

use super::ecumaster::{EcuMasterChannel, EcuMasterMeta};
use super::haltech::{HaltechChannel, HaltechMeta};

/// Metadata enum supporting different ECU formats
#[derive(Clone, Debug, Serialize, Default)]
pub enum Meta {
    Haltech(HaltechMeta),
    EcuMaster(EcuMasterMeta),
    #[default]
    Empty,
}

/// Channel enum supporting different ECU formats
#[derive(Clone, Debug)]
pub enum Channel {
    Haltech(HaltechChannel),
    EcuMaster(EcuMasterChannel),
}

impl Serialize for Channel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Channel::Haltech(h) => h.serialize(serializer),
            Channel::EcuMaster(e) => e.serialize(serializer),
        }
    }
}

impl Channel {
    pub fn name(&self) -> String {
        match self {
            Channel::Haltech(h) => h.name.clone(),
            Channel::EcuMaster(e) => e.name.clone(),
        }
    }

    #[allow(dead_code)]
    pub fn id(&self) -> String {
        match self {
            Channel::Haltech(h) => h.id.clone(),
            Channel::EcuMaster(e) => e.path.clone(),
        }
    }

    pub fn type_name(&self) -> String {
        match self {
            Channel::Haltech(h) => h.r#type.as_ref().to_string(),
            Channel::EcuMaster(e) => e.path.clone(),
        }
    }

    pub fn display_min(&self) -> Option<f64> {
        match self {
            Channel::Haltech(h) => h.display_min,
            Channel::EcuMaster(_) => None,
        }
    }

    pub fn display_max(&self) -> Option<f64> {
        match self {
            Channel::Haltech(h) => h.display_max,
            Channel::EcuMaster(_) => None,
        }
    }

    pub fn unit(&self) -> &str {
        match self {
            Channel::Haltech(h) => h.unit(),
            Channel::EcuMaster(e) => e.unit(),
        }
    }
}

/// Value types for log data
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Value {
    Bool(bool),
    Float(f64),
    Int(i64),
    String(String),
}

impl Value {
    /// Convert value to f64 for charting
    pub fn as_f64(&self) -> f64 {
        match self {
            Value::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Value::Float(f) => *f,
            Value::Int(i) => *i as f64,
            Value::String(_) => 0.0,
        }
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Float(f) => serializer.serialize_f64(*f),
            Value::Int(i) => serializer.serialize_i64(*i),
            Value::String(s) => serializer.serialize_str(s),
        }
    }
}

/// Parsed log file structure
#[derive(Clone, Debug, Default)]
pub struct Log {
    #[allow(dead_code)]
    pub meta: Meta,
    pub channels: Vec<Channel>,
    pub times: Vec<String>,
    pub data: Vec<Vec<Value>>,
}

impl Log {
    /// Get data for a specific channel by index
    pub fn get_channel_data(&self, channel_index: usize) -> Vec<f64> {
        self.data
            .iter()
            .filter_map(|row| row.get(channel_index).map(|v| v.as_f64()))
            .collect()
    }

    /// Get time values as f64 (seconds)
    pub fn get_times_as_f64(&self) -> Vec<f64> {
        self.times
            .iter()
            .filter_map(|t| t.parse::<f64>().ok())
            .collect()
    }

    /// Find channel index by name
    #[allow(dead_code)]
    pub fn find_channel_index(&self, name: &str) -> Option<usize> {
        self.channels.iter().position(|c| c.name() == name)
    }
}

/// Trait for log file parsers
pub trait Parseable {
    fn parse(&self, data: &str) -> Result<Log, Box<dyn Error>>;
}

/// Supported ECU types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum EcuType {
    #[default]
    Haltech,
    EcuMaster,
    MegaSquirt,
    Aem,
    MaxxEcu,
    MotEc,
    Link,
    Speeduino,
    Unknown,
}

impl EcuType {
    pub fn name(&self) -> &'static str {
        match self {
            EcuType::Haltech => "Haltech",
            EcuType::EcuMaster => "ECUMaster",
            EcuType::MegaSquirt => "MegaSquirt",
            EcuType::Aem => "AEM",
            EcuType::MaxxEcu => "MaxxECU",
            EcuType::MotEc => "MoTeC",
            EcuType::Link => "Link",
            EcuType::Speeduino => "Speeduino",
            EcuType::Unknown => "Unknown",
        }
    }
}
