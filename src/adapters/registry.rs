//! Adapter registry for loading and managing OpenECU Alliance adapter specifications.
//!
//! This module provides functionality to:
//! - Load adapter YAML files with fallback chain: API -> cache -> embedded
//! - Build normalization maps from channel source_names
//! - Look up channel metadata by source name
//! - Support background refresh of specs from the API

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, RwLock};

use super::api;
use super::cache;
use super::types::{AdapterSpec, ChannelCategory, ChannelSpec, ProtocolSpec};

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

// Embed protocol YAML files at compile time
// These define CAN bus message structures for real-time data streaming
const HALTECH_ELITE_PROTOCOL_YAML: &str =
    include_str!("../../spec/OECUASpecs/protocols/haltech/haltech-elite-broadcast.protocol.yaml");
const ECUMASTER_EMU_PROTOCOL_YAML: &str =
    include_str!("../../spec/OECUASpecs/protocols/ecumaster/ecumaster-emu-broadcast.protocol.yaml");
const SPEEDUINO_PROTOCOL_YAML: &str =
    include_str!("../../spec/OECUASpecs/protocols/speeduino/speeduino-broadcast.protocol.yaml");
const RUSEFI_PROTOCOL_YAML: &str =
    include_str!("../../spec/OECUASpecs/protocols/rusefi/rusefi-broadcast.protocol.yaml");
const AEM_INFINITY_PROTOCOL_YAML: &str =
    include_str!("../../spec/OECUASpecs/protocols/aem/aem-infinity-broadcast.protocol.yaml");
const MEGASQUIRT_PROTOCOL_YAML: &str =
    include_str!("../../spec/OECUASpecs/protocols/megasquirt/megasquirt-broadcast.protocol.yaml");
const MAXXECU_PROTOCOL_YAML: &str =
    include_str!("../../spec/OECUASpecs/protocols/maxxecu/maxxecu-default.protocol.yaml");
const SYVECS_S7_PROTOCOL_YAML: &str =
    include_str!("../../spec/OECUASpecs/protocols/syvecs/syvecs-s7-broadcast.protocol.yaml");
const EMTRON_PROTOCOL_YAML: &str =
    include_str!("../../spec/OECUASpecs/protocols/emtron/emtron-broadcast.protocol.yaml");

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

/// All embedded protocol YAML strings
static EMBEDDED_PROTOCOLS: &[&str] = &[
    HALTECH_ELITE_PROTOCOL_YAML,
    ECUMASTER_EMU_PROTOCOL_YAML,
    SPEEDUINO_PROTOCOL_YAML,
    RUSEFI_PROTOCOL_YAML,
    AEM_INFINITY_PROTOCOL_YAML,
    MEGASQUIRT_PROTOCOL_YAML,
    MAXXECU_PROTOCOL_YAML,
    SYVECS_S7_PROTOCOL_YAML,
    EMTRON_PROTOCOL_YAML,
];

/// Parse embedded adapter YAML strings
fn parse_embedded_adapters() -> Vec<AdapterSpec> {
    EMBEDDED_ADAPTERS
        .iter()
        .filter_map(|yaml| match serde_yaml::from_str(yaml) {
            Ok(spec) => Some(spec),
            Err(e) => {
                tracing::warn!("Failed to parse embedded adapter YAML: {}", e);
                None
            }
        })
        .collect()
}

/// Parse embedded protocol YAML strings
fn parse_embedded_protocols() -> Vec<ProtocolSpec> {
    EMBEDDED_PROTOCOLS
        .iter()
        .filter_map(|yaml| match serde_yaml::from_str(yaml) {
            Ok(spec) => Some(spec),
            Err(e) => {
                tracing::warn!("Failed to parse embedded protocol YAML: {}", e);
                None
            }
        })
        .collect()
}

/// Load adapters with fallback chain: cache -> embedded
/// API fetch is done in background to avoid blocking startup
fn load_adapters_with_fallback() -> Vec<AdapterSpec> {
    // 1. Try loading from cache first (fast, non-blocking)
    if !cache::is_cache_stale() {
        if let Some(cached) = cache::load_cached_adapters() {
            tracing::info!("Loaded {} adapters from cache", cached.len());
            return cached;
        }
    }

    // 2. Fall back to embedded specs (always available)
    tracing::info!("Using embedded adapter specs");
    parse_embedded_adapters()
}

/// Load protocols with fallback chain: cache -> embedded
/// API fetch is done in background to avoid blocking startup
fn load_protocols_with_fallback() -> Vec<ProtocolSpec> {
    // 1. Try loading from cache first (fast, non-blocking)
    if !cache::is_cache_stale() {
        if let Some(cached) = cache::load_cached_protocols() {
            tracing::info!("Loaded {} protocols from cache", cached.len());
            return cached;
        }
    }

    // 2. Fall back to embedded specs (always available)
    tracing::info!("Using embedded protocol specs");
    parse_embedded_protocols()
}

/// Tracks whether specs have been refreshed from API
static SPECS_REFRESHED: AtomicBool = AtomicBool::new(false);

/// Dynamically updatable adapter specifications
/// Initial load uses cache/embedded, background refresh updates from API
static ADAPTER_SPECS: LazyLock<RwLock<Vec<AdapterSpec>>> =
    LazyLock::new(|| RwLock::new(load_adapters_with_fallback()));

/// Dynamically updatable protocol specifications
/// Initial load uses cache/embedded, background refresh updates from API
static PROTOCOL_SPECS: LazyLock<RwLock<Vec<ProtocolSpec>>> =
    LazyLock::new(|| RwLock::new(load_protocols_with_fallback()));

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
/// Uses RwLock to allow dynamic updates from API refresh
static SPEC_NORMALIZATION_MAP: LazyLock<RwLock<HashMap<String, String>>> = LazyLock::new(|| {
    RwLock::new(build_normalization_map(
        &ADAPTER_SPECS.read().expect("Failed to read adapter specs"),
    ))
});

/// Channel metadata lookup: source name (lowercase) -> full metadata
/// Uses RwLock to allow dynamic updates from API refresh
static CHANNEL_METADATA_MAP: LazyLock<RwLock<HashMap<String, ChannelMetadata>>> =
    LazyLock::new(|| {
        RwLock::new(build_metadata_map(
            &ADAPTER_SPECS.read().expect("Failed to read adapter specs"),
        ))
    });

/// Build normalization map from adapter specs
fn build_normalization_map(adapters: &[AdapterSpec]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for adapter in adapters {
        for channel in &adapter.channels {
            for source_name in &channel.source_names {
                // Use display name as the normalized name
                map.insert(source_name.to_lowercase(), channel.name.clone());
            }
        }
    }
    map
}

/// Build metadata map from adapter specs
fn build_metadata_map(adapters: &[AdapterSpec]) -> HashMap<String, ChannelMetadata> {
    let mut map = HashMap::new();
    for adapter in adapters {
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
}

/// Get all loaded adapter specifications
pub fn get_adapters() -> Vec<AdapterSpec> {
    ADAPTER_SPECS
        .read()
        .expect("Failed to read adapter specs")
        .clone()
}

/// Get adapter by ID
pub fn get_adapter_by_id(id: &str) -> Option<AdapterSpec> {
    ADAPTER_SPECS
        .read()
        .expect("Failed to read adapter specs")
        .iter()
        .find(|a| a.id == id)
        .cloned()
}

/// Get adapter by vendor name
pub fn get_adapters_by_vendor(vendor: &str) -> Vec<AdapterSpec> {
    let vendor_lower = vendor.to_lowercase();
    ADAPTER_SPECS
        .read()
        .expect("Failed to read adapter specs")
        .iter()
        .filter(|a| a.vendor.to_lowercase() == vendor_lower)
        .cloned()
        .collect()
}

/// Normalize a channel name using the spec-driven normalization map.
/// Returns the canonical display name if found, otherwise returns the original name.
pub fn normalize_from_spec(name: &str) -> Option<String> {
    SPEC_NORMALIZATION_MAP
        .read()
        .expect("Failed to read normalization map")
        .get(&name.to_lowercase())
        .cloned()
}

/// Get channel metadata by source name
pub fn get_channel_metadata(name: &str) -> Option<ChannelMetadata> {
    CHANNEL_METADATA_MAP
        .read()
        .expect("Failed to read metadata map")
        .get(&name.to_lowercase())
        .cloned()
}

/// Get all spec-based normalization mappings as (source_name, display_name) pairs.
/// This can be used to merge with or enhance the existing normalize.rs mappings.
pub fn get_spec_normalizations() -> Vec<(String, String)> {
    SPEC_NORMALIZATION_MAP
        .read()
        .expect("Failed to read normalization map")
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Check if a channel name has spec-based normalization
pub fn has_spec_normalization(name: &str) -> bool {
    SPEC_NORMALIZATION_MAP
        .read()
        .expect("Failed to read normalization map")
        .contains_key(&name.to_lowercase())
}

/// Find matching adapters for a file based on extension
pub fn find_adapters_by_extension(extension: &str) -> Vec<AdapterSpec> {
    let ext = if extension.starts_with('.') {
        extension.to_lowercase()
    } else {
        format!(".{}", extension.to_lowercase())
    };

    ADAPTER_SPECS
        .read()
        .expect("Failed to read adapter specs")
        .iter()
        .filter(|a| {
            a.file_format
                .extensions
                .iter()
                .any(|e| e.to_lowercase() == ext)
        })
        .cloned()
        .collect()
}

/// Get all unique channel categories from loaded adapters
pub fn get_all_categories() -> Vec<ChannelCategory> {
    let mut categories: Vec<ChannelCategory> = ADAPTER_SPECS
        .read()
        .expect("Failed to read adapter specs")
        .iter()
        .flat_map(|a| a.channels.iter().map(|c| c.category))
        .collect();
    categories.sort_by_key(|c| c.display_name());
    categories.dedup();
    categories
}

/// Get all channels for a specific category across all adapters
pub fn get_channels_by_category(category: ChannelCategory) -> Vec<ChannelSpec> {
    ADAPTER_SPECS
        .read()
        .expect("Failed to read adapter specs")
        .iter()
        .flat_map(|a| a.channels.iter().cloned())
        .filter(|c| c.category == category)
        .collect()
}

// ============================================================================
// Protocol Registry Functions
// ============================================================================

/// Get all loaded protocol specifications
pub fn get_protocols() -> Vec<ProtocolSpec> {
    PROTOCOL_SPECS
        .read()
        .expect("Failed to read protocol specs")
        .clone()
}

/// Get protocol by ID
pub fn get_protocol_by_id(id: &str) -> Option<ProtocolSpec> {
    PROTOCOL_SPECS
        .read()
        .expect("Failed to read protocol specs")
        .iter()
        .find(|p| p.id == id)
        .cloned()
}

/// Get protocols by vendor name
pub fn find_protocols_by_vendor(vendor: &str) -> Vec<ProtocolSpec> {
    let vendor_lower = vendor.to_lowercase();
    PROTOCOL_SPECS
        .read()
        .expect("Failed to read protocol specs")
        .iter()
        .filter(|p| p.vendor.to_lowercase() == vendor_lower)
        .cloned()
        .collect()
}

// ============================================================================
// Background Refresh Functions
// ============================================================================

/// Result of a spec refresh operation
#[derive(Debug, Clone)]
pub enum RefreshResult {
    /// Successfully refreshed from API
    Success {
        adapters_count: usize,
        protocols_count: usize,
    },
    /// Failed to refresh (using cached/embedded data)
    Failed(String),
    /// Already refreshed, skipped
    AlreadyRefreshed,
}

/// Refresh specs from the API and update the registry
/// This function is designed to be called from a background thread
pub fn refresh_specs_from_api() -> RefreshResult {
    // Check if already refreshed to avoid redundant API calls
    if SPECS_REFRESHED.load(Ordering::SeqCst) {
        return RefreshResult::AlreadyRefreshed;
    }

    tracing::info!("Refreshing specs from OpenECUAlliance API...");

    // Fetch adapters from API
    let adapters_result = api::fetch_all_adapters();
    let protocols_result = api::fetch_all_protocols();

    match (adapters_result, protocols_result) {
        (Ok(adapters), Ok(protocols)) => {
            let adapters_count = adapters.len();
            let protocols_count = protocols.len();

            // Save to cache
            if let Err(e) = cache::save_adapters_to_cache(&adapters) {
                tracing::warn!("Failed to cache adapters: {}", e);
            }
            if let Err(e) = cache::save_protocols_to_cache(&protocols) {
                tracing::warn!("Failed to cache protocols: {}", e);
            }

            // Update the registry
            if let Ok(mut adapter_lock) = ADAPTER_SPECS.write() {
                *adapter_lock = adapters;
            }
            if let Ok(mut protocol_lock) = PROTOCOL_SPECS.write() {
                *protocol_lock = protocols;
            }

            // Rebuild derived maps
            if let Ok(specs) = ADAPTER_SPECS.read() {
                if let Ok(mut norm_lock) = SPEC_NORMALIZATION_MAP.write() {
                    *norm_lock = build_normalization_map(&specs);
                }
                if let Ok(mut meta_lock) = CHANNEL_METADATA_MAP.write() {
                    *meta_lock = build_metadata_map(&specs);
                }
            }

            SPECS_REFRESHED.store(true, Ordering::SeqCst);
            tracing::info!(
                "Successfully refreshed {} adapters and {} protocols from API",
                adapters_count,
                protocols_count
            );

            RefreshResult::Success {
                adapters_count,
                protocols_count,
            }
        }
        (Err(e), _) => {
            tracing::warn!("Failed to fetch adapters from API: {}", e);
            RefreshResult::Failed(format!("Adapter fetch failed: {}", e))
        }
        (_, Err(e)) => {
            tracing::warn!("Failed to fetch protocols from API: {}", e);
            RefreshResult::Failed(format!("Protocol fetch failed: {}", e))
        }
    }
}

/// Check if specs have been refreshed from API
pub fn specs_refreshed() -> bool {
    SPECS_REFRESHED.load(Ordering::SeqCst)
}

/// Get the current spec source (for display purposes)
pub fn get_spec_source() -> &'static str {
    if SPECS_REFRESHED.load(Ordering::SeqCst) {
        "API (refreshed)"
    } else if !cache::is_cache_stale() && cache::load_cached_adapters().is_some() {
        "Cache"
    } else {
        "Embedded"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapters_load() {
        let adapters = get_adapters();
        assert!(!adapters.is_empty(), "Should load at least one adapter");

        // Check that we have the expected adapters
        let adapter_ids: Vec<String> = adapters.iter().map(|a| a.id.clone()).collect();
        assert!(
            adapter_ids.iter().any(|id| id == "haltech-nsp"),
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

    #[test]
    fn test_protocols_load() {
        let protocols = get_protocols();
        assert!(!protocols.is_empty(), "Should load at least one protocol");

        // Check that we have the expected protocols
        let protocol_ids: Vec<String> = protocols.iter().map(|p| p.id.clone()).collect();
        assert!(
            protocol_ids
                .iter()
                .any(|id| id == "haltech-elite-broadcast"),
            "Should have haltech-elite-broadcast protocol"
        );
    }

    #[test]
    fn test_get_protocol_by_id() {
        let protocol = get_protocol_by_id("haltech-elite-broadcast");
        assert!(protocol.is_some());
        let proto = protocol.unwrap();
        assert_eq!(proto.vendor, "haltech");
        assert!(!proto.messages.is_empty(), "Protocol should have messages");
    }

    #[test]
    fn test_find_protocols_by_vendor() {
        let haltech_protocols = find_protocols_by_vendor("haltech");
        assert!(
            !haltech_protocols.is_empty(),
            "Should find Haltech protocols"
        );

        let speeduino_protocols = find_protocols_by_vendor("speeduino");
        assert!(
            !speeduino_protocols.is_empty(),
            "Should find Speeduino protocols"
        );
    }

    #[test]
    fn test_get_spec_source() {
        // Initially should be "Embedded" or "Cache" depending on environment
        let source = get_spec_source();
        assert!(
            source == "Embedded" || source == "Cache" || source == "API (refreshed)",
            "Spec source should be valid"
        );
    }
}
