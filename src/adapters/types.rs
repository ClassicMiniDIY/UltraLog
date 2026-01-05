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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_category_display_name() {
        assert_eq!(ChannelCategory::Engine.display_name(), "Engine");
        assert_eq!(ChannelCategory::DriverInput.display_name(), "Driver Input");
    }
}
