//! Adapter registry for loading and managing OpenECU Alliance adapter specifications.
//!
//! This module provides functionality to:
//! - Parse the adapter and protocol YAML embedded from `spec/openecu`
//! - Build normalization maps from channel source_names
//! - Look up channel metadata by source name
//!
//! The specs are immutable for the life of the process. Until the OpenECU Alliance was
//! retired this was a three-tier chain — a background fetch from `openecualliance.org`, a
//! 24-hour disk cache, then the embedded corpus — and the first two could replace the specs
//! at runtime, which is why everything here was behind `RwLock`s with a generation counter.
//! UltraLog owns the corpus now (see CLAUDE.md, "OpenECU spec corpus"), so the embedded tier
//! is the only tier and no lock, atomic or cache-invalidation hook is needed.

use std::collections::HashMap;
use std::sync::LazyLock;

use super::types::{AdapterSpec, ChannelCategory, ChannelSpec, ProtocolSpec};

// Embed adapter YAML files at compile time from the vendored corpus in spec/openecu.
// UltraLog owns that corpus; see CLAUDE.md "OpenECU spec corpus".
const HALTECH_NSP_YAML: &str =
    include_str!("../../spec/openecu/adapters/haltech/haltech-nsp.adapter.yaml");
const ECUMASTER_EMU_YAML: &str =
    include_str!("../../spec/openecu/adapters/ecumaster/ecumaster-emu-csv.adapter.yaml");
const LINK_LLG_YAML: &str = include_str!("../../spec/openecu/adapters/link/link-llg.adapter.yaml");
const AIM_XRK_YAML: &str = include_str!("../../spec/openecu/adapters/aim/aim-xrk.adapter.yaml");
const ROMRAIDER_CSV_YAML: &str =
    include_str!("../../spec/openecu/adapters/romraider/romraider-csv.adapter.yaml");
const SPEEDUINO_MLG_YAML: &str =
    include_str!("../../spec/openecu/adapters/speeduino/speeduino-mlg.adapter.yaml");
const RUSEFI_MLG_YAML: &str =
    include_str!("../../spec/openecu/adapters/rusefi/rusefi-mlg.adapter.yaml");
const EMERALD_LG_YAML: &str =
    include_str!("../../spec/openecu/adapters/emerald/emerald-lg.adapter.yaml");
const MEGASQUIRT_ADAPTER_YAML: &str =
    include_str!("../../spec/openecu/adapters/megasquirt/megasquirt-tunerstudio.adapter.yaml");
const BMW_MS4X_ADAPTER_YAML: &str =
    include_str!("../../spec/openecu/adapters/bmw/bmw-ms4x-romraider-csv.adapter.yaml");

// Embed protocol YAML files at compile time
// These define CAN bus message structures for real-time data streaming
const HALTECH_ELITE_PROTOCOL_YAML: &str =
    include_str!("../../spec/openecu/protocols/haltech/haltech-elite-broadcast.protocol.yaml");
const ECUMASTER_EMU_PROTOCOL_YAML: &str =
    include_str!("../../spec/openecu/protocols/ecumaster/ecumaster-emu-broadcast.protocol.yaml");
const SPEEDUINO_PROTOCOL_YAML: &str =
    include_str!("../../spec/openecu/protocols/speeduino/speeduino-broadcast.protocol.yaml");
const RUSEFI_PROTOCOL_YAML: &str =
    include_str!("../../spec/openecu/protocols/rusefi/rusefi-broadcast.protocol.yaml");
const AEM_INFINITY_PROTOCOL_YAML: &str =
    include_str!("../../spec/openecu/protocols/aem/aem-infinity-broadcast.protocol.yaml");
const MEGASQUIRT_PROTOCOL_YAML: &str =
    include_str!("../../spec/openecu/protocols/megasquirt/megasquirt-broadcast.protocol.yaml");
const MAXXECU_PROTOCOL_YAML: &str =
    include_str!("../../spec/openecu/protocols/maxxecu/maxxecu-default.protocol.yaml");
const SYVECS_S7_PROTOCOL_YAML: &str =
    include_str!("../../spec/openecu/protocols/syvecs/syvecs-s7-broadcast.protocol.yaml");
const EMTRON_PROTOCOL_YAML: &str =
    include_str!("../../spec/openecu/protocols/emtron/emtron-broadcast.protocol.yaml");
const BMW_MS42_PROTOCOL_YAML: &str =
    include_str!("../../spec/openecu/protocols/bmw/bmw-ms42-dme-broadcast.protocol.yaml");
const BMW_MS43_PROTOCOL_YAML: &str =
    include_str!("../../spec/openecu/protocols/bmw/bmw-ms43-dme-broadcast.protocol.yaml");
const BMW_E8X_E9X_PROTOCOL_YAML: &str =
    include_str!("../../spec/openecu/protocols/bmw/bmw-e8x-e9x-powertrain.protocol.yaml");

/// All embedded adapter YAML strings.
///
/// ORDER IS LOAD-BEARING. `build_normalization_map` and `build_metadata_map` key a
/// single flat map on the lowercased source name, so a name two adapters both claim
/// resolves to whichever appears LAST here. 54 source names get conflicting display
/// names across this corpus and 43 resolve to different canonical ids depending on
/// order — "boost" is `boost` or `mgp`, "knock" is `knock_level`, `knock_level_db` or
/// `knock_retard`, "fuel flow" is `fuel_flow` (l/h) or `fuel_flow_mass` (g/s). See
/// CLAUDE.md "Source-name collisions"; the real fix is a per-adapter namespace.
///
/// BMW is deliberately FIRST so that adding it changed no existing mapping: it
/// contributes its unique names and loses every collision to an adapter that was
/// already here. Appending it instead silently moved `rpm`'s display name from
/// "Engine RPM" to BMW's "Engine Speed", which is what `test_normalize_rpm` caught.
/// Add a new adapter at the FRONT unless you have checked its collisions.
///
/// This order is now the ONLY order. While the API and disk-cache tiers existed they could
/// replace the specs at runtime in whatever sequence they arrived, so neither the guarantee
/// above nor `test_normalize_rpm` held on a machine with a populated cache. Both tiers are
/// gone, so what the test asserts is what ships.
static EMBEDDED_ADAPTERS: &[&str] = &[
    BMW_MS4X_ADAPTER_YAML,
    HALTECH_NSP_YAML,
    ECUMASTER_EMU_YAML,
    LINK_LLG_YAML,
    AIM_XRK_YAML,
    ROMRAIDER_CSV_YAML,
    SPEEDUINO_MLG_YAML,
    RUSEFI_MLG_YAML,
    EMERALD_LG_YAML,
    MEGASQUIRT_ADAPTER_YAML,
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
    BMW_MS42_PROTOCOL_YAML,
    BMW_MS43_PROTOCOL_YAML,
    BMW_E8X_E9X_PROTOCOL_YAML,
];

/// Parse embedded adapter YAML strings
fn parse_embedded_adapters() -> Vec<AdapterSpec> {
    EMBEDDED_ADAPTERS
        .iter()
        .filter_map(|yaml| match serde_norway::from_str(yaml) {
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
        .filter_map(|yaml| match serde_norway::from_str(yaml) {
            Ok(spec) => Some(spec),
            Err(e) => {
                tracing::warn!("Failed to parse embedded protocol YAML: {}", e);
                None
            }
        })
        .collect()
}

/// The embedded adapter specifications, parsed once.
static ADAPTER_SPECS: LazyLock<Vec<AdapterSpec>> = LazyLock::new(parse_embedded_adapters);

/// The embedded protocol specifications, parsed once.
static PROTOCOL_SPECS: LazyLock<Vec<ProtocolSpec>> = LazyLock::new(parse_embedded_protocols);

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
static SPEC_NORMALIZATION_MAP: LazyLock<HashMap<String, String>> =
    LazyLock::new(|| build_normalization_map(&ADAPTER_SPECS));

/// Channel metadata lookup: source name (lowercase) -> full metadata
static CHANNEL_METADATA_MAP: LazyLock<HashMap<String, ChannelMetadata>> =
    LazyLock::new(|| build_metadata_map(&ADAPTER_SPECS));

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

// These were lock guards while a background refresh could replace the specs. They are plain
// borrows now; the call sites are unchanged because `&Vec`/`&HashMap` deref the same way.
fn read_adapter_specs() -> &'static [AdapterSpec] {
    &ADAPTER_SPECS
}

fn read_protocol_specs() -> &'static [ProtocolSpec] {
    &PROTOCOL_SPECS
}

fn read_normalization_map() -> &'static HashMap<String, String> {
    &SPEC_NORMALIZATION_MAP
}

fn read_metadata_map() -> &'static HashMap<String, ChannelMetadata> {
    &CHANNEL_METADATA_MAP
}

/// Get all loaded adapter specifications
pub fn get_adapters() -> Vec<AdapterSpec> {
    read_adapter_specs().to_vec()
}

/// Get adapter by ID
pub fn get_adapter_by_id(id: &str) -> Option<AdapterSpec> {
    read_adapter_specs().iter().find(|a| a.id == id).cloned()
}

/// Get adapter by vendor name
pub fn get_adapters_by_vendor(vendor: &str) -> Vec<AdapterSpec> {
    let vendor_lower = vendor.to_lowercase();
    read_adapter_specs()
        .iter()
        .filter(|a| a.vendor.to_lowercase() == vendor_lower)
        .cloned()
        .collect()
}

/// Normalize a channel name using the spec-driven normalization map.
/// Returns the canonical display name if found, otherwise returns the original name.
pub fn normalize_from_spec(name: &str) -> Option<String> {
    read_normalization_map().get(&name.to_lowercase()).cloned()
}

/// Get channel metadata by source name
pub fn get_channel_metadata(name: &str) -> Option<ChannelMetadata> {
    read_metadata_map().get(&name.to_lowercase()).cloned()
}

/// Get all spec-based normalization mappings as (source_name, display_name) pairs.
/// This can be used to merge with or enhance the existing normalize.rs mappings.
pub fn get_spec_normalizations() -> Vec<(String, String)> {
    read_normalization_map()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Check if a channel name has spec-based normalization
pub fn has_spec_normalization(name: &str) -> bool {
    read_normalization_map().contains_key(&name.to_lowercase())
}

/// Find matching adapters for a file based on extension
pub fn find_adapters_by_extension(extension: &str) -> Vec<AdapterSpec> {
    let ext = if extension.starts_with('.') {
        extension.to_lowercase()
    } else {
        format!(".{}", extension.to_lowercase())
    };

    read_adapter_specs()
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
    let mut categories: Vec<ChannelCategory> = read_adapter_specs()
        .iter()
        .flat_map(|a| a.channels.iter().map(|c| c.category))
        .collect();
    categories.sort_by_key(|c| c.display_name());
    categories.dedup();
    categories
}

/// Get all channels for a specific category across all adapters
pub fn get_channels_by_category(category: ChannelCategory) -> Vec<ChannelSpec> {
    read_adapter_specs()
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
    read_protocol_specs().to_vec()
}

/// Get protocol by ID
pub fn get_protocol_by_id(id: &str) -> Option<ProtocolSpec> {
    read_protocol_specs().iter().find(|p| p.id == id).cloned()
}

/// Get protocols by vendor name
pub fn find_protocols_by_vendor(vendor: &str) -> Vec<ProtocolSpec> {
    let vendor_lower = vendor.to_lowercase();
    read_protocol_specs()
        .iter()
        .filter(|p| p.vendor.to_lowercase() == vendor_lower)
        .cloned()
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

    /// Every embedded adapter spec must deserialize.
    ///
    /// `parse_embedded_adapters` deliberately stays lenient at runtime — it
    /// warns and skips a bad spec so one upstream typo cannot break startup.
    /// That means a broken spec is otherwise invisible, so it is caught here
    /// instead. This is what let `data_type: boolean` sit unnoticed in
    /// megasquirt-tunerstudio.adapter.yaml.
    ///
    /// Since the value enums gained a `#[serde(other)] Unknown` fallback, a
    /// typo such as `category: Fuel` parses instead of failing, so the test
    /// also fails on any field that landed on `Unknown`.
    #[test]
    fn all_embedded_adapters_parse() {
        let failures: Vec<String> = EMBEDDED_ADAPTERS
            .iter()
            .enumerate()
            .filter_map(|(i, yaml)| {
                spec_problem(
                    serde_norway::from_str::<AdapterSpec>(yaml).map(|s| s.unknown_fields()),
                )
                .map(|e| format!("adapter[{}] ({}): {}", i, spec_label(yaml), e))
            })
            .collect();
        assert!(
            failures.is_empty(),
            "{} of {} embedded adapter specs failed to parse:\n  {}",
            failures.len(),
            EMBEDDED_ADAPTERS.len(),
            failures.join("\n  ")
        );
    }

    /// Every embedded protocol spec must deserialize. See above.
    #[test]
    fn all_embedded_protocols_parse() {
        let failures: Vec<String> = EMBEDDED_PROTOCOLS
            .iter()
            .enumerate()
            .filter_map(|(i, yaml)| {
                spec_problem(
                    serde_norway::from_str::<ProtocolSpec>(yaml).map(|s| s.unknown_fields()),
                )
                .map(|e| format!("protocol[{}] ({}): {}", i, spec_label(yaml), e))
            })
            .collect();
        assert!(
            failures.is_empty(),
            "{} of {} embedded protocol specs failed to parse:\n  {}",
            failures.len(),
            EMBEDDED_PROTOCOLS.len(),
            failures.join("\n  ")
        );
    }

    /// A parse error, or the fields that fell back to `Unknown`; `None` if clean.
    fn spec_problem(parsed: Result<Vec<String>, serde_norway::Error>) -> Option<String> {
        match parsed {
            Err(e) => Some(e.to_string()),
            Ok(unknown) if !unknown.is_empty() => {
                Some(format!("unrecognized value in {}", unknown.join(", ")))
            }
            Ok(_) => None,
        }
    }

    /// One unrecognized value must cost one field, not the spec: the other
    /// channels, and the other fields of the same channel, still load.
    #[test]
    fn adapter_spec_survives_one_unknown_value() {
        // Trimmed from GET /api/adapters/speeduino/speeduino-mlg, with one
        // category changed to a value no build knows.
        let json = r##"{
            "openecualliance": "1.0",
            "id": "speeduino-mlg",
            "name": "Speeduino MLG",
            "version": "1.0.0",
            "vendor": "speeduino",
            "fileFormat": { "type": "binary", "extensions": [".mlg"] },
            "channels": [
                { "id": "rpm", "name": "Engine RPM", "category": "engine",
                  "dataType": "float", "unit": "rpm", "sourceNames": ["RPM"] },
                { "id": "rev_limiter_active", "name": "Rev Limiter Active",
                  "category": "status", "dataType": "bool", "unit": "",
                  "sourceNames": ["Rev Limiter"] },
                { "id": "e85", "name": "Ethanol", "category": "flex_fuel",
                  "dataType": "float", "unit": "%", "sourceNames": ["Ethanol %"] }
            ]
        }"##;

        let spec: AdapterSpec = serde_json::from_str(json).expect("one unknown value");
        assert_eq!(spec.channels.len(), 3);
        assert_eq!(
            spec.channels[0].category,
            crate::adapters::ChannelCategory::Engine
        );
        assert_eq!(
            spec.channels[1].category,
            crate::adapters::ChannelCategory::Status
        );
        assert_eq!(
            spec.channels[2].category,
            crate::adapters::ChannelCategory::Unknown
        );
        assert_eq!(spec.channels[2].source_names, vec!["Ethanol %"]);
        assert_eq!(spec.unknown_fields(), vec!["channels[2] (e85).category"]);
    }

    /// Protocol side of the same contract, on the BMW signal types the API serves.
    #[test]
    fn protocol_spec_survives_one_unknown_value() {
        let json = r##"{
            "openecualliance": "1.0",
            "type": "protocol",
            "id": "bmw-ms42-dme-broadcast",
            "name": "BMW MS42 DME Broadcast",
            "version": "1.0.0",
            "vendor": "bmw",
            "protocol": { "type": "can", "baudrate": 500000 },
            "messages": [{
                "id": "0x316",
                "name": "DME1",
                "length": 8,
                "signals": [
                    { "name": "LV_SWI_IGK", "startBit": 0, "length": 1,
                      "byteOrder": "little_endian", "dataType": "bool" },
                    { "name": "TQ_STATE", "startBit": 4, "length": 4,
                      "byteOrder": "little_endian", "dataType": "enum" },
                    { "name": "N_ENG", "startBit": 16, "length": 16,
                      "byteOrder": "little_endian", "dataType": "fixed_point" }
                ]
            }]
        }"##;

        let spec: ProtocolSpec = serde_json::from_str(json).expect("one unknown value");
        let sigs = &spec.messages[0].signals;
        assert_eq!(sigs.len(), 3);
        assert_eq!(sigs[0].data_type, crate::adapters::SignalDataType::Bool);
        assert_eq!(sigs[1].data_type, crate::adapters::SignalDataType::Enum);
        assert_eq!(sigs[2].data_type, crate::adapters::SignalDataType::Unknown);
        assert_eq!(sigs[2].start_bit, 16);
        assert_eq!(
            spec.unknown_fields(),
            vec!["messages[0].signals[2] (N_ENG).data_type"]
        );
    }

    /// The embedded-spec guard must catch a typo that `serde(other)` now accepts.
    #[test]
    fn spec_problem_flags_unknown_fields() {
        let yaml = "openecualliance: '1.0'\nid: t\nname: T\nversion: 1.0.0\nvendor: t\n\
                    file_format: {type: csv, extensions: ['.csv']}\n\
                    channels:\n  - {id: rpm, name: RPM, category: Fuel, data_type: float, unit: rpm, source_names: [RPM]}\n";
        let problem =
            spec_problem(serde_norway::from_str::<AdapterSpec>(yaml).map(|s| s.unknown_fields()));
        assert_eq!(
            problem.as_deref(),
            Some("unrecognized value in channels[0] (rpm).category")
        );
    }

    /// Pull the `id:` line out of a spec so failures name the offending file.
    fn spec_label(yaml: &str) -> &str {
        yaml.lines()
            .find_map(|l| l.strip_prefix("id:"))
            .map(str::trim)
            .unwrap_or("unknown")
    }

    /// The API serves camelCase where the YAML is snake_case. Both must
    /// deserialize into the same structs, or the API tier of the fallback
    /// chain silently resolves nothing.
    #[test]
    fn adapter_spec_accepts_camel_case_from_api() {
        // Trimmed from GET /api/adapters/haltech/haltech-nsp.
        let json = r##"{
            "openecualliance": "1.0",
            "id": "haltech-nsp",
            "name": "Haltech NSP CSV Export",
            "version": "1.0.0",
            "vendor": "haltech",
            "branding": { "colorPrimary": "#FFBE1A", "colorSecondary": "#1A1A1A" },
            "fileFormat": {
                "type": "csv",
                "extensions": [".csv"],
                "delimiter": ",",
                "headerRow": -1,
                "dataStartRow": -1
            },
            "channels": [{
                "id": "rpm",
                "name": "Engine RPM",
                "category": "engine",
                "dataType": "float",
                "unit": "rpm",
                "sourceNames": ["Engine Speed", "RPM"]
            }]
        }"##;

        let spec: AdapterSpec = serde_json::from_str(json).expect("camelCase adapter JSON");
        assert_eq!(spec.file_format.format_type, "csv");
        assert_eq!(spec.file_format.header_row, Some(-1));
        assert_eq!(spec.file_format.data_start_row, Some(-1));
        assert_eq!(spec.channels[0].data_type, crate::adapters::DataType::Float);
        assert_eq!(spec.channels[0].source_names, vec!["Engine Speed", "RPM"]);
        assert_eq!(
            spec.branding.as_ref().unwrap().color_primary.as_deref(),
            Some("#FFBE1A")
        );
    }

    /// Same contract on the protocol side (`startBit`, `byteOrder`, `dataType`).
    #[test]
    fn protocol_spec_accepts_camel_case_from_api() {
        // Trimmed from GET /api/protocols/haltech/haltech-elite-broadcast.
        let json = r##"{
            "openecualliance": "1.0",
            "type": "protocol",
            "id": "haltech-elite-broadcast",
            "name": "Haltech Elite Broadcast",
            "version": "1.0.0",
            "vendor": "haltech",
            "protocol": { "type": "can", "baudrate": 1000000, "extendedId": false },
            "messages": [{
                "id": "0x360",
                "name": "RPM / MAP",
                "length": 8,
                "intervalMs": 20,
                "signals": [{
                    "name": "RPM",
                    "startBit": 0,
                    "length": 16,
                    "byteOrder": "big_endian",
                    "dataType": "unsigned",
                    "scale": 1,
                    "offset": 0,
                    "unit": "rpm"
                }]
            }]
        }"##;

        let spec: ProtocolSpec = serde_json::from_str(json).expect("camelCase protocol JSON");
        assert!(!spec.protocol.extended_id);
        assert_eq!(spec.messages[0].id, 0x360);
        assert_eq!(spec.messages[0].interval_ms, Some(20.0));
        let sig = &spec.messages[0].signals[0];
        assert_eq!(sig.start_bit, 0);
        assert_eq!(sig.byte_order, crate::adapters::ByteOrder::BigEndian);
        assert_eq!(sig.data_type, crate::adapters::SignalDataType::Unsigned);
    }

    /// snake_case (the embedded YAML casing) must keep working unchanged.
    #[test]
    fn adapter_spec_still_accepts_snake_case_from_yaml() {
        let yaml = r##"
openecualliance: "1.0"
id: test-adapter
name: Test
version: 1.0.0
vendor: test
file_format:
  type: csv
  extensions: [".csv"]
  header_row: 0
channels:
  - id: rpm
    name: Engine RPM
    category: engine
    data_type: float
    unit: rpm
    source_names: ["RPM"]
  - id: flag
    name: Some Flag
    category: system
    data_type: boolean
    unit: ""
    source_names: ["Flag"]
"##;
        let spec: AdapterSpec = serde_norway::from_str(yaml).expect("snake_case adapter YAML");
        assert_eq!(spec.file_format.header_row, Some(0));
        assert_eq!(spec.channels[0].data_type, crate::adapters::DataType::Float);
        // `boolean` is an upstream typo for `bool`; both must land on Bool.
        assert_eq!(spec.channels[1].data_type, crate::adapters::DataType::Bool);
    }
}
