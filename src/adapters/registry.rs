//! Adapter registry for loading and managing OpenECU Alliance adapter specifications.
//!
//! This module provides functionality to:
//! - Load embedded adapter YAML files at compile time
//! - Build normalization maps from channel source_names
//! - Look up channel metadata by source name

use std::collections::HashMap;
use std::sync::LazyLock;

use super::types::{AdapterSpec, ChannelCategory, ChannelSpec};

// Embed adapter YAML files at compile time
// These are loaded from the OECUASpecs git submodule (spec/OECUASpecs/)
// If building from source, run: git submodule update --init
const HALTECH_NSP_YAML: &str =
    include_str!("../../spec/OECUASpecs/adapters/haltech/haltech-nsp.adapter.yaml");
const ECUMASTER_EMU_YAML: &str =
    include_str!("../../spec/OECUASpecs/adapters/ecumaster/ecumaster-emu-csv.adapter.yaml");
const LINK_LLG_YAML: &str =
    include_str!("../../spec/OECUASpecs/adapters/link/link-llg.adapter.yaml");
const AIM_XRK_YAML: &str = include_str!("../../spec/OECUASpecs/adapters/aim/aim-xrk.adapter.yaml");
const ROMRAIDER_CSV_YAML: &str =
    include_str!("../../spec/OECUASpecs/adapters/romraider/romraider-csv.adapter.yaml");
const SPEEDUINO_MLG_YAML: &str =
    include_str!("../../spec/OECUASpecs/adapters/speeduino/speeduino-mlg.adapter.yaml");
const RUSEFI_MLG_YAML: &str =
    include_str!("../../spec/OECUASpecs/adapters/rusefi/rusefi-mlg.adapter.yaml");
const EMERALD_LG_YAML: &str =
    include_str!("../../spec/OECUASpecs/adapters/emerald/emerald-lg.adapter.yaml");

/// All embedded adapter YAML strings
static EMBEDDED_ADAPTERS: &[&str] = &[
    HALTECH_NSP_YAML,
    ECUMASTER_EMU_YAML,
    LINK_LLG_YAML,
    AIM_XRK_YAML,
    ROMRAIDER_CSV_YAML,
    SPEEDUINO_MLG_YAML,
    RUSEFI_MLG_YAML,
    EMERALD_LG_YAML,
];

/// Parsed adapter specifications (loaded lazily)
static ADAPTER_SPECS: LazyLock<Vec<AdapterSpec>> = LazyLock::new(|| {
    EMBEDDED_ADAPTERS
        .iter()
        .filter_map(|yaml| match serde_yaml::from_str(yaml) {
            Ok(spec) => Some(spec),
            Err(e) => {
                tracing::warn!("Failed to parse adapter YAML: {}", e);
                None
            }
        })
        .collect()
});

/// Channel metadata lookup by source name (lowercase)
#[derive(Debug, Clone)]
pub struct ChannelMetadata {
    /// Canonical channel ID (e.g., "rpm", "coolant_temp")
    pub canonical_id: String,
    /// Human-readable display name
    pub display_name: String,
    /// Channel category
    pub category: ChannelCategory,
    /// Canonical unit
    pub unit: String,
    /// Minimum valid value
    pub min: Option<f64>,
    /// Maximum valid value
    pub max: Option<f64>,
    /// Decimal places for display
    pub precision: Option<u32>,
    /// Vendor ID that defined this channel
    pub vendor: String,
}

/// Normalization map: source name (lowercase) -> canonical display name
static SPEC_NORMALIZATION_MAP: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for adapter in ADAPTER_SPECS.iter() {
        for channel in &adapter.channels {
            for source_name in &channel.source_names {
                // Use display name as the normalized name
                map.insert(source_name.to_lowercase(), channel.name.clone());
            }
        }
    }
    map
});

/// Channel metadata lookup: source name (lowercase) -> full metadata
static CHANNEL_METADATA_MAP: LazyLock<HashMap<String, ChannelMetadata>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    for adapter in ADAPTER_SPECS.iter() {
        for channel in &adapter.channels {
            let metadata = ChannelMetadata {
                canonical_id: channel.id.clone(),
                display_name: channel.name.clone(),
                category: channel.category,
                unit: channel.unit.clone(),
                min: channel.min,
                max: channel.max,
                precision: channel.precision,
                vendor: adapter.vendor.clone(),
            };
            for source_name in &channel.source_names {
                map.insert(source_name.to_lowercase(), metadata.clone());
            }
        }
    }
    map
});

/// Get all loaded adapter specifications
pub fn get_adapters() -> &'static Vec<AdapterSpec> {
    &ADAPTER_SPECS
}

/// Get adapter by ID
pub fn get_adapter_by_id(id: &str) -> Option<&'static AdapterSpec> {
    ADAPTER_SPECS.iter().find(|a| a.id == id)
}

/// Get adapter by vendor name
pub fn get_adapters_by_vendor(vendor: &str) -> Vec<&'static AdapterSpec> {
    let vendor_lower = vendor.to_lowercase();
    ADAPTER_SPECS
        .iter()
        .filter(|a| a.vendor.to_lowercase() == vendor_lower)
        .collect()
}

/// Normalize a channel name using the spec-driven normalization map.
/// Returns the canonical display name if found, otherwise returns the original name.
pub fn normalize_from_spec(name: &str) -> Option<String> {
    SPEC_NORMALIZATION_MAP.get(&name.to_lowercase()).cloned()
}

/// Get channel metadata by source name
pub fn get_channel_metadata(name: &str) -> Option<&'static ChannelMetadata> {
    CHANNEL_METADATA_MAP.get(&name.to_lowercase())
}

/// Get all spec-based normalization mappings as (source_name, display_name) pairs.
/// This can be used to merge with or enhance the existing normalize.rs mappings.
pub fn get_spec_normalizations() -> impl Iterator<Item = (&'static String, &'static String)> {
    SPEC_NORMALIZATION_MAP.iter()
}

/// Check if a channel name has spec-based normalization
pub fn has_spec_normalization(name: &str) -> bool {
    SPEC_NORMALIZATION_MAP.contains_key(&name.to_lowercase())
}

/// Find matching adapters for a file based on extension
pub fn find_adapters_by_extension(extension: &str) -> Vec<&'static AdapterSpec> {
    let ext = if extension.starts_with('.') {
        extension.to_lowercase()
    } else {
        format!(".{}", extension.to_lowercase())
    };

    ADAPTER_SPECS
        .iter()
        .filter(|a| {
            a.file_format
                .extensions
                .iter()
                .any(|e| e.to_lowercase() == ext)
        })
        .collect()
}

/// Get all unique channel categories from loaded adapters
pub fn get_all_categories() -> Vec<ChannelCategory> {
    let mut categories: Vec<ChannelCategory> = ADAPTER_SPECS
        .iter()
        .flat_map(|a| a.channels.iter().map(|c| c.category))
        .collect();
    categories.sort_by_key(|c| c.display_name());
    categories.dedup();
    categories
}

/// Get all channels for a specific category across all adapters
pub fn get_channels_by_category(category: ChannelCategory) -> Vec<&'static ChannelSpec> {
    ADAPTER_SPECS
        .iter()
        .flat_map(|a| a.channels.iter())
        .filter(|c| c.category == category)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapters_load() {
        let adapters = get_adapters();
        assert!(!adapters.is_empty(), "Should load at least one adapter");

        // Check that we have the expected adapters
        let adapter_ids: Vec<&str> = adapters.iter().map(|a| a.id.as_str()).collect();
        assert!(
            adapter_ids.contains(&"haltech-nsp"),
            "Should have haltech-nsp adapter"
        );
    }

    #[test]
    fn test_normalize_rpm() {
        // These source names should all normalize to "Engine RPM" based on haltech-nsp spec
        let rpm_sources = ["Engine Speed", "Engine RPM", "RPM", "Eng Speed"];
        for source in rpm_sources {
            let normalized = normalize_from_spec(source);
            assert!(normalized.is_some(), "Should normalize '{}'", source);
            assert_eq!(
                normalized.unwrap(),
                "Engine RPM",
                "'{}' should normalize to 'Engine RPM'",
                source
            );
        }
    }

    #[test]
    fn test_get_channel_metadata() {
        let metadata = get_channel_metadata("Engine Speed");
        assert!(metadata.is_some());
        let meta = metadata.unwrap();
        assert_eq!(meta.canonical_id, "rpm");
        assert_eq!(meta.unit, "rpm");
        assert_eq!(meta.category, ChannelCategory::Engine);
    }

    #[test]
    fn test_find_adapters_by_extension() {
        let csv_adapters = find_adapters_by_extension(".csv");
        assert!(!csv_adapters.is_empty(), "Should find CSV adapters");

        let llg_adapters = find_adapters_by_extension("llg");
        assert!(!llg_adapters.is_empty(), "Should find LLG adapters");
    }
}
