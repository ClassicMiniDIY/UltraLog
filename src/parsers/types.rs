use serde::Serialize;
use std::error::Error;

use super::aim::{AimChannel, AimMeta};
use super::ecumaster::{EcuMasterChannel, EcuMasterMeta};
use super::haltech::{HaltechChannel, HaltechMeta};
use super::link::{LinkChannel, LinkMeta};
use super::romraider::{RomRaiderChannel, RomRaiderMeta};
use super::speeduino::{SpeeduinoChannel, SpeeduinoMeta};

/// Metadata enum supporting different ECU formats
#[derive(Clone, Debug, Serialize, Default)]
pub enum Meta {
    Aim(AimMeta),
    Haltech(HaltechMeta),
    EcuMaster(EcuMasterMeta),
    Link(LinkMeta),
    RomRaider(RomRaiderMeta),
    Speeduino(SpeeduinoMeta),
    #[default]
    Empty,
}

/// Information for a computed channel
#[derive(Clone, Debug, serde::Serialize)]
pub struct ComputedChannelInfo {
    /// Display name for the channel
    pub name: String,
    /// The formula expression
    pub formula: String,
    /// Unit to display
    pub unit: String,
}

/// Channel enum supporting different ECU formats
#[derive(Clone, Debug)]
pub enum Channel {
    Aim(AimChannel),
    Haltech(HaltechChannel),
    EcuMaster(EcuMasterChannel),
    Link(LinkChannel),
    RomRaider(RomRaiderChannel),
    Speeduino(SpeeduinoChannel),
    /// A computed/virtual channel derived from a formula
    Computed(ComputedChannelInfo),
}

impl Serialize for Channel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Channel::Aim(a) => a.serialize(serializer),
            Channel::Haltech(h) => h.serialize(serializer),
            Channel::EcuMaster(e) => e.serialize(serializer),
            Channel::Link(l) => l.serialize(serializer),
            Channel::RomRaider(r) => r.serialize(serializer),
            Channel::Speeduino(s) => s.serialize(serializer),
            Channel::Computed(c) => c.serialize(serializer),
        }
    }
}

impl Channel {
    pub fn name(&self) -> String {
        match self {
            Channel::Aim(a) => a.name.clone(),
            Channel::Haltech(h) => h.name.clone(),
            Channel::EcuMaster(e) => e.name.clone(),
            Channel::Link(l) => l.name.clone(),
            Channel::RomRaider(r) => r.name.clone(),
            Channel::Speeduino(s) => s.name.clone(),
            Channel::Computed(c) => c.name.clone(),
        }
    }

    #[allow(dead_code)]
    pub fn id(&self) -> String {
        match self {
            Channel::Aim(a) => a.name.clone(),
            Channel::Haltech(h) => h.id.clone(),
            Channel::EcuMaster(e) => e.path.clone(),
            Channel::Link(l) => l.channel_id.to_string(),
            Channel::RomRaider(r) => r.name.clone(),
            Channel::Speeduino(s) => s.name.clone(),
            Channel::Computed(c) => format!("computed_{}", c.name),
        }
    }

    pub fn type_name(&self) -> String {
        match self {
            Channel::Aim(_) => "AIM".to_string(),
            Channel::Haltech(h) => h.r#type.as_ref().to_string(),
            Channel::EcuMaster(e) => e.path.clone(),
            Channel::Link(_) => "Link".to_string(),
            Channel::RomRaider(_) => "RomRaider".to_string(),
            Channel::Speeduino(_) => "Speeduino/rusEFI".to_string(),
            Channel::Computed(_) => "Computed".to_string(),
        }
    }

    pub fn display_min(&self) -> Option<f64> {
        match self {
            Channel::Aim(_) => None,
            Channel::Haltech(h) => h.display_min,
            Channel::EcuMaster(_) => None,
            Channel::Link(_) => None,
            Channel::RomRaider(_) => None,
            Channel::Speeduino(_) => None,
            Channel::Computed(_) => None,
        }
    }

    pub fn display_max(&self) -> Option<f64> {
        match self {
            Channel::Aim(_) => None,
            Channel::Haltech(h) => h.display_max,
            Channel::EcuMaster(_) => None,
            Channel::Link(_) => None,
            Channel::RomRaider(_) => None,
            Channel::Speeduino(_) => None,
            Channel::Computed(_) => None,
        }
    }

    pub fn unit(&self) -> &str {
        match self {
            Channel::Aim(a) => a.unit(),
            Channel::Haltech(h) => h.unit(),
            Channel::EcuMaster(e) => e.unit(),
            Channel::Link(l) => l.unit(),
            Channel::RomRaider(r) => r.unit(),
            Channel::Speeduino(s) => s.unit(),
            Channel::Computed(c) => &c.unit,
        }
    }

    /// Check if this is a computed channel
    pub fn is_computed(&self) -> bool {
        matches!(self, Channel::Computed(_))
    }
}

/// Optimized value storage - all ECU log data is stored as f64
/// This uses 8 bytes per value instead of 16 bytes with the previous enum
#[derive(Clone, Copy, Debug, Default)]
pub struct Value(f64);

impl Value {
    /// Create a new Value from an f64
    /// Note: Named 'Float' to maintain API compatibility with previous enum variant
    #[inline]
    #[allow(non_snake_case)]
    pub fn Float(value: f64) -> Self {
        Self(value)
    }

    /// Convert value to f64 for charting
    #[inline]
    pub fn as_f64(&self) -> f64 {
        self.0
    }
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_f64(self.0)
    }
}

/// Parsed log file structure
#[derive(Clone, Debug, Default)]
pub struct Log {
    #[allow(dead_code)]
    pub meta: Meta,
    pub channels: Vec<Channel>,
    /// Time values stored directly as f64 (seconds) for efficiency
    pub times: Vec<f64>,
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

    /// Get time values as f64 slice (seconds) - no parsing needed, stored directly
    pub fn get_times_as_f64(&self) -> &[f64] {
        &self.times
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
    Aim,
    EcuMaster,
    MegaSquirt,
    Aem,
    MaxxEcu,
    MotEc,
    Link,
    RomRaider,
    Speeduino,
    Unknown,
}

impl EcuType {
    pub fn name(&self) -> &'static str {
        match self {
            EcuType::Haltech => "Haltech",
            EcuType::Aim => "AIM",
            EcuType::EcuMaster => "ECUMaster",
            EcuType::MegaSquirt => "MegaSquirt",
            EcuType::Aem => "AEM",
            EcuType::MaxxEcu => "MaxxECU",
            EcuType::MotEc => "MoTeC",
            EcuType::Link => "Link",
            EcuType::RomRaider => "RomRaider",
            EcuType::Speeduino => "Speeduino/rusEFI",
            EcuType::Unknown => "Unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // Value Tests
    // ============================================

    #[test]
    fn test_value_creation() {
        let val = Value::Float(42.5);
        assert_eq!(val.as_f64(), 42.5);
    }

    #[test]
    fn test_value_zero() {
        let val = Value::Float(0.0);
        assert_eq!(val.as_f64(), 0.0);
    }

    #[test]
    fn test_value_negative() {
        let val = Value::Float(-273.15);
        assert_eq!(val.as_f64(), -273.15);
    }

    #[test]
    fn test_value_large() {
        let val = Value::Float(1_000_000.0);
        assert_eq!(val.as_f64(), 1_000_000.0);
    }

    #[test]
    fn test_value_small() {
        let val = Value::Float(0.000001);
        assert!((val.as_f64() - 0.000001).abs() < 1e-10);
    }

    #[test]
    fn test_value_default() {
        let val = Value::default();
        assert_eq!(val.as_f64(), 0.0);
    }

    #[test]
    fn test_value_copy() {
        let val1 = Value::Float(100.0);
        let val2 = val1; // Copy
        assert_eq!(val1.as_f64(), val2.as_f64());
    }

    #[test]
    fn test_value_clone() {
        let val1 = Value::Float(100.0);
        #[allow(clippy::clone_on_copy)]
        let val2 = val1.clone(); // Testing Clone trait specifically
        assert_eq!(val1.as_f64(), val2.as_f64());
    }

    // ============================================
    // Log Tests
    // ============================================

    #[test]
    fn test_log_default() {
        let log = Log::default();
        assert!(log.channels.is_empty());
        assert!(log.times.is_empty());
        assert!(log.data.is_empty());
    }

    #[test]
    fn test_log_get_channel_data() {
        let log = Log {
            meta: Meta::Empty,
            channels: vec![],
            times: vec![0.0, 1.0, 2.0],
            data: vec![
                vec![Value::Float(100.0), Value::Float(200.0)],
                vec![Value::Float(110.0), Value::Float(210.0)],
                vec![Value::Float(120.0), Value::Float(220.0)],
            ],
        };

        let channel0 = log.get_channel_data(0);
        assert_eq!(channel0, vec![100.0, 110.0, 120.0]);

        let channel1 = log.get_channel_data(1);
        assert_eq!(channel1, vec![200.0, 210.0, 220.0]);
    }

    #[test]
    fn test_log_get_channel_data_out_of_bounds() {
        let log = Log {
            meta: Meta::Empty,
            channels: vec![],
            times: vec![0.0, 1.0],
            data: vec![vec![Value::Float(100.0)], vec![Value::Float(110.0)]],
        };

        // Out of bounds should return empty
        let channel_oob = log.get_channel_data(5);
        assert!(channel_oob.is_empty());
    }

    #[test]
    fn test_log_get_times_as_f64() {
        let log = Log {
            meta: Meta::Empty,
            channels: vec![],
            times: vec![0.0, 0.5, 1.0, 1.5, 2.0],
            data: vec![],
        };

        let times = log.get_times_as_f64();
        assert_eq!(times.len(), 5);
        assert_eq!(times[0], 0.0);
        assert_eq!(times[2], 1.0);
        assert_eq!(times[4], 2.0);
    }

    #[test]
    fn test_log_find_channel_index() {
        use super::super::haltech::{ChannelType, HaltechChannel};

        let log = Log {
            meta: Meta::Empty,
            channels: vec![
                Channel::Haltech(HaltechChannel {
                    name: "RPM".to_string(),
                    id: "1".to_string(),
                    r#type: ChannelType::EngineSpeed,
                    display_min: None,
                    display_max: None,
                }),
                Channel::Haltech(HaltechChannel {
                    name: "Manifold Pressure".to_string(),
                    id: "2".to_string(),
                    r#type: ChannelType::Pressure,
                    display_min: None,
                    display_max: None,
                }),
                Channel::Haltech(HaltechChannel {
                    name: "Coolant Temp".to_string(),
                    id: "3".to_string(),
                    r#type: ChannelType::Temperature,
                    display_min: None,
                    display_max: None,
                }),
            ],
            times: vec![],
            data: vec![],
        };

        assert_eq!(log.find_channel_index("RPM"), Some(0));
        assert_eq!(log.find_channel_index("Manifold Pressure"), Some(1));
        assert_eq!(log.find_channel_index("Coolant Temp"), Some(2));
        assert_eq!(log.find_channel_index("Not Found"), None);
    }

    // ============================================
    // Channel Tests
    // ============================================

    #[test]
    fn test_channel_name_haltech() {
        use super::super::haltech::{ChannelType, HaltechChannel};

        let channel = Channel::Haltech(HaltechChannel {
            name: "Engine RPM".to_string(),
            id: "123".to_string(),
            r#type: ChannelType::EngineSpeed,
            display_min: None,
            display_max: None,
        });

        assert_eq!(channel.name(), "Engine RPM");
    }

    #[test]
    fn test_channel_id_haltech() {
        use super::super::haltech::{ChannelType, HaltechChannel};

        let channel = Channel::Haltech(HaltechChannel {
            name: "Engine RPM".to_string(),
            id: "123".to_string(),
            r#type: ChannelType::EngineSpeed,
            display_min: None,
            display_max: None,
        });

        assert_eq!(channel.id(), "123");
    }

    #[test]
    fn test_channel_type_name_haltech() {
        use super::super::haltech::{ChannelType, HaltechChannel};

        let channel = Channel::Haltech(HaltechChannel {
            name: "Engine RPM".to_string(),
            id: "123".to_string(),
            r#type: ChannelType::EngineSpeed,
            display_min: None,
            display_max: None,
        });

        assert_eq!(channel.type_name(), "EngineSpeed");
    }

    #[test]
    fn test_channel_display_min_max_haltech() {
        use super::super::haltech::{ChannelType, HaltechChannel};

        let channel_with_bounds = Channel::Haltech(HaltechChannel {
            name: "RPM".to_string(),
            id: "1".to_string(),
            r#type: ChannelType::EngineSpeed,
            display_min: Some(0.0),
            display_max: Some(10000.0),
        });

        assert_eq!(channel_with_bounds.display_min(), Some(0.0));
        assert_eq!(channel_with_bounds.display_max(), Some(10000.0));

        let channel_without_bounds = Channel::Haltech(HaltechChannel {
            name: "RPM".to_string(),
            id: "1".to_string(),
            r#type: ChannelType::EngineSpeed,
            display_min: None,
            display_max: None,
        });

        assert_eq!(channel_without_bounds.display_min(), None);
        assert_eq!(channel_without_bounds.display_max(), None);
    }

    #[test]
    fn test_channel_unit_haltech() {
        use super::super::haltech::{ChannelType, HaltechChannel};

        let rpm_channel = Channel::Haltech(HaltechChannel {
            name: "RPM".to_string(),
            id: "1".to_string(),
            r#type: ChannelType::EngineSpeed,
            display_min: None,
            display_max: None,
        });
        assert_eq!(rpm_channel.unit(), "RPM");

        let temp_channel = Channel::Haltech(HaltechChannel {
            name: "Coolant".to_string(),
            id: "2".to_string(),
            r#type: ChannelType::Temperature,
            display_min: None,
            display_max: None,
        });
        assert_eq!(temp_channel.unit(), "K");

        let pressure_channel = Channel::Haltech(HaltechChannel {
            name: "MAP".to_string(),
            id: "3".to_string(),
            r#type: ChannelType::Pressure,
            display_min: None,
            display_max: None,
        });
        assert_eq!(pressure_channel.unit(), "kPa");
    }

    // ============================================
    // EcuType Tests
    // ============================================

    #[test]
    fn test_ecu_type_names() {
        assert_eq!(EcuType::Haltech.name(), "Haltech");
        assert_eq!(EcuType::EcuMaster.name(), "ECUMaster");
        assert_eq!(EcuType::MegaSquirt.name(), "MegaSquirt");
        assert_eq!(EcuType::Aem.name(), "AEM");
        assert_eq!(EcuType::MaxxEcu.name(), "MaxxECU");
        assert_eq!(EcuType::MotEc.name(), "MoTeC");
        assert_eq!(EcuType::Link.name(), "Link");
        assert_eq!(EcuType::RomRaider.name(), "RomRaider");
        assert_eq!(EcuType::Speeduino.name(), "Speeduino/rusEFI");
        assert_eq!(EcuType::Unknown.name(), "Unknown");
    }

    #[test]
    fn test_ecu_type_default() {
        let default = EcuType::default();
        assert_eq!(default, EcuType::Haltech);
    }

    #[test]
    fn test_ecu_type_equality() {
        assert_eq!(EcuType::Haltech, EcuType::Haltech);
        assert_ne!(EcuType::Haltech, EcuType::EcuMaster);
    }

    #[test]
    fn test_ecu_type_copy() {
        let ecu1 = EcuType::Speeduino;
        let ecu2 = ecu1; // Copy
        assert_eq!(ecu1, ecu2);
    }

    // ============================================
    // Meta Tests
    // ============================================

    #[test]
    fn test_meta_default() {
        let meta = Meta::default();
        assert!(matches!(meta, Meta::Empty));
    }

    #[test]
    fn test_meta_clone() {
        let meta = Meta::Empty;
        let meta_clone = meta.clone();
        assert!(matches!(meta_clone, Meta::Empty));
    }
}
