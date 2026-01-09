//! OpenECU Alliance adapter specification types.
//!
//! These types mirror the OpenECU Alliance adapter schema for parsing
//! adapter YAML files that define ECU log format specifications.

use serde::Deserialize;

/// OpenECU Alliance adapter specification
#[derive(Debug, Clone, Deserialize)]
pub struct AdapterSpec {
    /// Specification version (e.g., "1.0")
    pub openecualliance: String,
    /// Unique adapter identifier (e.g., "haltech-nsp")
    pub id: String,
    /// Human-readable adapter name
    pub name: String,
    /// Adapter version (semver)
    pub version: String,
    /// ECU vendor/manufacturer
    pub vendor: String,
    /// Detailed description
    #[serde(default)]
    pub description: Option<String>,
    /// Vendor website URL
    #[serde(default)]
    pub website: Option<String>,
    /// Branding assets
    #[serde(default)]
    pub branding: Option<BrandingSpec>,
    /// File format specification
    pub file_format: FileFormatSpec,
    /// Channel definitions
    pub channels: Vec<ChannelSpec>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: Option<MetadataSpec>,
}

/// Branding assets for the vendor
#[derive(Debug, Clone, Deserialize, Default)]
pub struct BrandingSpec {
    /// Logo file path (relative to assets/logos/)
    #[serde(default)]
    pub logo: Option<String>,
    /// Icon file path (relative to assets/icons/)
    #[serde(default)]
    pub icon: Option<String>,
    /// Banner file path (relative to assets/banners/)
    #[serde(default)]
    pub banner: Option<String>,
    /// Primary brand color (hex)
    #[serde(default)]
    pub color_primary: Option<String>,
    /// Secondary brand color (hex)
    #[serde(default)]
    pub color_secondary: Option<String>,
}

/// File format specification
#[derive(Debug, Clone, Deserialize)]
pub struct FileFormatSpec {
    /// Format type: "csv" or "binary"
    #[serde(rename = "type")]
    pub format_type: String,
    /// Valid file extensions
    pub extensions: Vec<String>,
    /// File encoding (CSV only)
    #[serde(default)]
    pub encoding: Option<String>,
    /// Column delimiter (CSV only)
    #[serde(default)]
    pub delimiter: Option<String>,
    /// Header row index (CSV only)
    #[serde(default)]
    pub header_row: Option<i32>,
    /// Data start row index (CSV only)
    #[serde(default)]
    pub data_start_row: Option<i32>,
    /// Timestamp column name (CSV only)
    #[serde(default)]
    pub timestamp_column: Option<String>,
    /// Timestamp unit (CSV only)
    #[serde(default)]
    pub timestamp_unit: Option<String>,
    /// Byte order: "little" or "big" (binary only)
    #[serde(default)]
    pub endianness: Option<String>,
    /// File signature bytes (binary only)
    #[serde(default)]
    pub magic_bytes: Option<Vec<u8>>,
    /// Header size in bytes (binary only)
    #[serde(default)]
    pub header_size: Option<i32>,
    /// Record size type: "fixed" or "variable" (binary only)
    #[serde(default)]
    pub record_size: Option<String>,
    /// Link to format documentation
    #[serde(default)]
    pub specification_url: Option<String>,
}

/// Channel specification
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSpec {
    /// Canonical channel identifier (e.g., "rpm", "coolant_temp")
    pub id: String,
    /// Human-readable display name
    pub name: String,
    /// Detailed description
    #[serde(default)]
    pub description: Option<String>,
    /// Channel category
    pub category: ChannelCategory,
    /// Data type of values
    pub data_type: DataType,
    /// Canonical unit
    pub unit: String,
    /// Minimum valid value
    #[serde(default)]
    pub min: Option<f64>,
    /// Maximum valid value
    #[serde(default)]
    pub max: Option<f64>,
    /// Decimal places for display
    #[serde(default)]
    pub precision: Option<u32>,
    /// Vendor-specific names in log files (for normalization)
    pub source_names: Vec<String>,
    /// Unit of source data if different from canonical
    #[serde(default)]
    pub source_unit: Option<String>,
    /// Formula to convert from source to canonical unit
    #[serde(default)]
    pub conversion: Option<String>,
    /// Searchable tags
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

/// Channel categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelCategory {
    Engine,
    Fuel,
    Ignition,
    Temperature,
    Pressure,
    Electrical,
    Speed,
    Drivetrain,
    Correction,
    System,
    Acceleration,
    Rotation,
    Position,
    Suspension,
    Timing,
    Traction,
    DriverInput,
    Custom,
}

impl ChannelCategory {
    /// Get display name for the category
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Engine => "Engine",
            Self::Fuel => "Fuel",
            Self::Ignition => "Ignition",
            Self::Temperature => "Temperature",
            Self::Pressure => "Pressure",
            Self::Electrical => "Electrical",
            Self::Speed => "Speed",
            Self::Drivetrain => "Drivetrain",
            Self::Correction => "Correction",
            Self::System => "System",
            Self::Acceleration => "Acceleration",
            Self::Rotation => "Rotation",
            Self::Position => "Position",
            Self::Suspension => "Suspension",
            Self::Timing => "Timing",
            Self::Traction => "Traction",
            Self::DriverInput => "Driver Input",
            Self::Custom => "Custom",
        }
    }
}

/// Data types for channel values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    Float,
    Int,
    Bool,
    String,
    Enum,
}

/// Adapter metadata
#[derive(Debug, Clone, Deserialize, Default)]
pub struct MetadataSpec {
    /// Adapter author
    #[serde(default)]
    pub author: Option<String>,
    /// License
    #[serde(default)]
    pub license: Option<String>,
    /// Source repository URL
    #[serde(default)]
    pub repository: Option<String>,
    /// ECU models tested with this adapter
    #[serde(default)]
    pub tested_with: Option<Vec<String>>,
    /// Known issues or limitations
    #[serde(default)]
    pub known_issues: Option<Vec<String>>,
    /// Changelog entries
    #[serde(default)]
    pub changelog: Option<Vec<ChangelogEntry>>,
}

/// Changelog entry
#[derive(Debug, Clone, Deserialize)]
pub struct ChangelogEntry {
    /// Version
    pub version: String,
    /// Release date
    pub date: String,
    /// List of changes
    pub changes: Vec<String>,
}

// ============================================================================
// CAN Protocol Types (for real-time data streaming)
// ============================================================================

/// OpenECU Alliance CAN protocol specification
#[derive(Debug, Clone, Deserialize)]
pub struct ProtocolSpec {
    /// Specification version (e.g., "1.0")
    pub openecualliance: String,
    /// Type identifier (must be "protocol")
    #[serde(rename = "type")]
    pub spec_type: String,
    /// Unique protocol identifier (e.g., "haltech-elite-broadcast")
    pub id: String,
    /// Human-readable protocol name
    pub name: String,
    /// Protocol version (semver)
    pub version: String,
    /// ECU vendor/manufacturer
    pub vendor: String,
    /// Detailed description
    #[serde(default)]
    pub description: Option<String>,
    /// Vendor website URL
    #[serde(default)]
    pub website: Option<String>,
    /// Branding assets
    #[serde(default)]
    pub branding: Option<BrandingSpec>,
    /// Protocol configuration (CAN settings)
    pub protocol: ProtocolInfo,
    /// CAN message definitions
    pub messages: Vec<MessageSpec>,
    /// Enumeration definitions for discrete signals
    #[serde(default)]
    pub enums: Option<Vec<EnumSpec>>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: Option<MetadataSpec>,
}

/// Protocol configuration
#[derive(Debug, Clone, Deserialize)]
pub struct ProtocolInfo {
    /// Protocol type: "can", "canfd", "lin", "k-line"
    #[serde(rename = "type")]
    pub protocol_type: ProtocolType,
    /// Communication speed in bits per second
    pub baudrate: u32,
    /// Whether to use 29-bit extended IDs (true) or 11-bit standard IDs (false)
    #[serde(default)]
    pub extended_id: bool,
    /// Data phase baudrate for CAN FD (bits per second)
    #[serde(default)]
    pub data_baudrate: Option<u32>,
    /// Whether CAN FD is enabled
    #[serde(default)]
    pub fd_enabled: bool,
    /// Base message ID (if configurable)
    #[serde(default)]
    pub base_id: Option<u32>,
    /// Whether base ID can be changed in ECU settings
    #[serde(default)]
    pub base_id_configurable: bool,
}

/// Protocol types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolType {
    Can,
    Canfd,
    Lin,
    #[serde(rename = "k-line")]
    KLine,
}

/// CAN message definition
#[derive(Debug, Clone, Deserialize)]
pub struct MessageSpec {
    /// CAN message ID (supports hex strings like "0x360" or decimal integers)
    #[serde(deserialize_with = "deserialize_message_id")]
    pub id: u32,
    /// Human-readable message name
    pub name: String,
    /// Detailed description
    #[serde(default)]
    pub description: Option<String>,
    /// Message length in bytes (0-8 for CAN, 0-64 for CAN FD)
    pub length: u8,
    /// Broadcast interval in milliseconds
    #[serde(default)]
    pub interval_ms: Option<f64>,
    /// Node that transmits this message
    #[serde(default)]
    pub transmitter: Option<String>,
    /// Signal definitions within this message
    pub signals: Vec<SignalSpec>,
}

/// Signal within a CAN message
#[derive(Debug, Clone, Deserialize)]
pub struct SignalSpec {
    /// Signal name
    pub name: String,
    /// Detailed description
    #[serde(default)]
    pub description: Option<String>,
    /// Starting bit position (0-indexed)
    pub start_bit: u16,
    /// Signal length in bits
    pub length: u8,
    /// Byte order (Intel = little_endian, Motorola = big_endian)
    pub byte_order: ByteOrder,
    /// Data type interpretation
    pub data_type: SignalDataType,
    /// Scale factor: physical_value = (raw_value * scale) + offset
    #[serde(default = "default_scale")]
    pub scale: f64,
    /// Offset value: physical_value = (raw_value * scale) + offset
    #[serde(default)]
    pub offset: f64,
    /// Physical unit of the signal
    #[serde(default)]
    pub unit: Option<String>,
    /// Minimum physical value
    #[serde(default)]
    pub min: Option<f64>,
    /// Maximum physical value
    #[serde(default)]
    pub max: Option<f64>,
    /// Reference to an enum definition for discrete values
    #[serde(default)]
    pub enum_ref: Option<String>,
    /// Additional notes
    #[serde(default)]
    pub comment: Option<String>,
}

fn default_scale() -> f64 {
    1.0
}

/// Byte order for multi-byte signals
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ByteOrder {
    LittleEndian,
    BigEndian,
}

/// Signal data types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalDataType {
    Unsigned,
    Signed,
    Float,
    Double,
}

/// Enumeration definition for discrete signal values
#[derive(Debug, Clone, Deserialize)]
pub struct EnumSpec {
    /// Enumeration name
    pub name: String,
    /// Description of this enumeration
    #[serde(default)]
    pub description: Option<String>,
    /// Mapping of raw values to labels
    pub values: std::collections::HashMap<String, String>,
}

/// Custom deserializer for message IDs that can be hex strings or integers
fn deserialize_message_id<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct MessageIdVisitor;

    impl<'de> Visitor<'de> for MessageIdVisitor {
        type Value = u32;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a hex string (0x...) or an integer")
        }

        fn visit_u64<E>(self, value: u64) -> Result<u32, E>
        where
            E: de::Error,
        {
            if value > u32::MAX as u64 {
                Err(E::custom(format!("message ID out of range: {}", value)))
            } else {
                Ok(value as u32)
            }
        }

        fn visit_i64<E>(self, value: i64) -> Result<u32, E>
        where
            E: de::Error,
        {
            if value < 0 || value > u32::MAX as i64 {
                Err(E::custom(format!("message ID out of range: {}", value)))
            } else {
                Ok(value as u32)
            }
        }

        fn visit_str<E>(self, value: &str) -> Result<u32, E>
        where
            E: de::Error,
        {
            // Parse hex string (e.g., "0x360" -> 864)
            if value.starts_with("0x") || value.starts_with("0X") {
                u32::from_str_radix(&value[2..], 16)
                    .map_err(|e| E::custom(format!("invalid hex string '{}': {}", value, e)))
            } else {
                value
                    .parse::<u32>()
                    .map_err(|e| E::custom(format!("invalid integer '{}': {}", value, e)))
            }
        }
    }

    deserializer.deserialize_any(MessageIdVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_category_display_name() {
        assert_eq!(ChannelCategory::Engine.display_name(), "Engine");
        assert_eq!(ChannelCategory::DriverInput.display_name(), "Driver Input");
    }

    #[test]
    fn test_message_id_deserialization() {
        // Test deserializing from hex string
        let json = r#"{"id": "0x360", "name": "Test", "length": 8, "signals": []}"#;
        let msg: Result<MessageSpec, _> = serde_json::from_str(json);
        assert!(msg.is_ok());
        assert_eq!(msg.unwrap().id, 0x360);

        // Test deserializing from decimal integer
        let json = r#"{"id": 864, "name": "Test", "length": 8, "signals": []}"#;
        let msg: Result<MessageSpec, _> = serde_json::from_str(json);
        assert!(msg.is_ok());
        assert_eq!(msg.unwrap().id, 864);
    }
}
