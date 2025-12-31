//! Field name normalization for standardizing channel names across different ECU types.
//!
//! This module provides mappings from various ECU-specific channel names to standardized names,
//! making it easier for users to compare data from different logging systems.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Mapping from normalized (standard) names to their possible source names
static NORMALIZATION_MAP: LazyLock<HashMap<&'static str, Vec<&'static str>>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();

        // AFR (Air Fuel Ratio) - generic/overall readings
        map.insert(
            "AFR",
            vec![
                "Act_AFR",
                "R_EGO",
                "Aft",
                "Act AFR",
                "AFR",
                "Air Fuel Ratio",
                "AFR_R_BANK",
                // Wideband O2 sensors - overall/combined readings only
                "Wideband O2 Overall",
                "Wideband O2",
                "WB O2",
                "O2 Wideband",
                "Wideband Overall",
            ],
        );

        // AFR Target
        map.insert(
            "AFR Target",
            vec![
                "AFR Targ",
                "Target AFR",
                "afrtgt",
                "R_AFR_TARGET",
                "AFR_Target",
            ],
        );

        // AFR 1 - first sensor/bank
        map.insert(
            "AFR Channel 1",
            vec![
                "AFR_1",
                "AFR 1",
                "AFR1",
                "WB2 AFR 1",
                "Wideband O2 1",
                "WB O2 1",
                "Wideband 1",
            ],
        );

        // AFR 2 - second sensor/bank
        map.insert(
            "AFR Channel 2",
            vec![
                "Aft2",
                "AFR 2",
                "AFR_2",
                "afr_2",
                "AFR2",
                "WB2 AFR 2",
                "Wideband O2 2",
                "WB O2 2",
                "Wideband 2",
            ],
        );

        // AFR 1 Error
        map.insert(
            "AFR 1 Error",
            vec!["AFR_Error", "AFR_1_Error", "AFR 1 Error", "KO2_AFR_CORR"],
        );

        // Battery Voltage
        map.insert(
            "Battery V",
            vec![
                "VBat",
                "Vbat",
                "Bat V",
                "Batt",
                "Volts",
                "Voltage",
                "Bat Volts ECU",
                "Battery Voltage",
                "ECU Battery Voltage",
                "Ecu power",
                "BATTERY_VOLTAGE",
                "Bat_V",
            ],
        );

        // Coolant Temperature
        map.insert(
            "Coolant Temp",
            vec![
                "Coolant",
                "CLT",
                "Temp_Coolant",
                "Temp Engine",
                "Engine Temperature",
                "Coolant Temperature",
                "CoolantTemp",
                "coolantTemp",
                "Coolant Temp",
            ],
        );

        // Duty Cycle
        map.insert(
            "Duty Cycle",
            vec![
                "DutyCycle1",
                "DutyCycle",
                "Duty Cycle",
                "PCS Fuel Duty Cyl 1",
                "DUTY CYCLE AVE",
                "Duty_Cycle",
            ],
        );

        // EGO Correction 1
        map.insert(
            "EGO Cor 1",
            vec!["EGO Correction", "ID2 Cor", "EGO_Cor_1", "EGO Cor 1"],
        );

        // EGO Correction 2
        map.insert(
            "EGO Cor 2",
            vec![
                "L_O2_Cor",
                "ID2_Cor",
                "O2_COR_L_BANK",
                "EGO_cor_2",
                "EGO Cor 2",
            ],
        );

        // Intake Air Temperature
        map.insert(
            "IAT",
            vec![
                "IAT",
                "iat",
                "IAT - Inlet Air Temp",
                "IAT Intake Air Temp",
                "Intake Air Temp",
                "Intake Air Temperature",
            ],
        );

        // Lambda 1
        map.insert(
            "Lambda 1",
            vec![
                "Lambda Right",
                "LambdaR",
                "lambdaR",
                "Lambda 1",
                "Exhaust Lambda",
                "LAMBDA",
                "LAM8",
                "LAMBDA_R_BANK",
                "Lambda1",
            ],
        );

        // Mass Air Flow
        map.insert(
            "MAF",
            vec!["Mass Air Flow", "Mass Air Flow Rate", "MAF", "maf"],
        );

        // Manifold Absolute Pressure
        map.insert(
            "MAP",
            vec![
                "MAP",
                "Map",
                "map",
                "Manifold Pressure",
                "Inlet Manifold Pressure",
            ],
        );

        // Manifold Air Temperature
        map.insert(
            "MAT",
            vec![
                "Air Temp Int",
                "Inlet Manifold Temperature",
                "TAIR",
                "MAT",
                "mat",
            ],
        );

        // O2 Sensor
        map.insert("O2", vec!["O2", "O2 Sensor", "o2"]);

        // Pulse Width
        map.insert(
            "Pulse Width",
            vec![
                "pulsewidth1",
                "Injector PW Rear",
                "injector",
                "Fuel Actual PW",
                "PW1",
                "NJ_GPW_AVE",
                "PW",
                "pw",
            ],
        );

        // RPM
        map.insert(
            "RPM",
            vec![
                "RPM",
                "rpm",
                "Speed",
                "PCS RPM4",
                "Engine RPM4",
                "RPM_INC_RPM",
                "engine/rpm",
            ],
        );

        // Throttle Position Sensor
        map.insert(
            "TPS",
            vec![
                "TPS_Pct",
                "Tps",
                "Throttle Pos",
                "TP",
                "Throttle Position",
                "PedalPos",
                "PCS TPS",
                "TPS",
                "tps",
                "tps1",
            ],
        );

        // Time
        map.insert(
            "Time",
            vec![
                "Time_s",
                "Device Time",
                "Timestamp",
                "TIME",
                "Time",
                "Offset",
                "time",
            ],
        );

        // Additional common channels
        map.insert(
            "Ignition Adv",
            vec![
                "Ignition Advance",
                "Timing",
                "Spark Advance",
                "IgnAdv",
                "ignition/angle",
            ],
        );

        map.insert(
            "Knock",
            vec!["Knock", "Knock Retard", "KnockRetard", "Knock Count"],
        );

        map.insert(
            "Boost",
            vec!["Boost", "Boost Pressure", "BoostPressure", "boost"],
        );

        map.insert(
            "Oil Pressure",
            vec!["Oil Pressure", "OilPressure", "Oil Press", "oilPressure"],
        );

        map.insert(
            "Oil Temp",
            vec!["Oil Temp", "OilTemp", "Oil Temperature", "oilTemp"],
        );

        map.insert(
            "Fuel Pressure",
            vec![
                "Fuel Pressure",
                "FuelPressure",
                "Fuel Press",
                "fuelPressure",
            ],
        );

        map.insert(
            "EGT",
            vec!["EGT", "Exhaust Gas Temp", "Exhaust Temperature", "egt"],
        );

        map.insert(
            "Vehicle Speed",
            vec!["Vehicle Speed", "VSS", "Speed", "vss", "vehicleSpeed"],
        );

        map.insert("Gear", vec!["Gear", "Current Gear", "GearPosition", "gear"]);

        map
    });

/// Reverse lookup map: source name (lowercase) -> normalized name
static REVERSE_MAP: LazyLock<HashMap<String, &'static str>> = LazyLock::new(|| {
    let mut reverse = HashMap::new();
    for (normalized, sources) in NORMALIZATION_MAP.iter() {
        for source in sources {
            reverse.insert(source.to_lowercase(), *normalized);
        }
    }
    reverse
});

/// Normalize a channel name to its standard form.
/// Returns the normalized name if a mapping exists, otherwise returns the original name.
pub fn normalize_channel_name(name: &str) -> String {
    normalize_channel_name_with_custom(name, None)
}

/// Normalize a channel name with optional custom mappings.
/// Custom mappings take priority over built-in mappings.
pub fn normalize_channel_name_with_custom(
    name: &str,
    custom_mappings: Option<&std::collections::HashMap<String, String>>,
) -> String {
    let name_lower = name.to_lowercase();

    // Check custom mappings first (they take priority)
    if let Some(custom) = custom_mappings {
        // Direct lookup in custom mappings
        if let Some(normalized) = custom.get(&name_lower) {
            return normalized.clone();
        }
        // Also check with original case
        if let Some(normalized) = custom.get(name) {
            return normalized.clone();
        }
        // Try matching with path stripped
        if let Some(last_segment) = name.rsplit('/').next() {
            let segment_lower = last_segment.to_lowercase();
            if let Some(normalized) = custom.get(&segment_lower) {
                return normalized.clone();
            }
        }
    }

    // Fall back to built-in mappings
    // Direct lookup
    if let Some(normalized) = REVERSE_MAP.get(&name_lower) {
        return normalized.to_string();
    }

    // Try matching with path stripped (e.g., "engine/rpm" -> "rpm")
    if let Some(last_segment) = name.rsplit('/').next() {
        let segment_lower = last_segment.to_lowercase();
        if let Some(normalized) = REVERSE_MAP.get(&segment_lower) {
            return normalized.to_string();
        }
    }

    // No mapping found, return original
    name.to_string()
}

/// Get all built-in normalization mappings as a vector of (normalized_name, source_names)
pub fn get_builtin_mappings() -> Vec<(&'static str, Vec<&'static str>)> {
    NORMALIZATION_MAP
        .iter()
        .map(|(k, v)| (*k, v.clone()))
        .collect()
}

/// Get the normalized name for display, with original name as suffix if different.
/// Returns "Normalized (Original)" format when normalization occurs.
pub fn get_display_name(name: &str, show_original: bool) -> String {
    let normalized = normalize_channel_name(name);
    if normalized != name && show_original {
        format!("{} ({})", normalized, name)
    } else {
        normalized
    }
}

/// Check if a channel name has a known normalization mapping.
/// Returns true if the name exists in the normalization mappings (built-in or custom).
pub fn has_normalization(name: &str, custom_mappings: Option<&HashMap<String, String>>) -> bool {
    let name_lower = name.to_lowercase();

    // Check custom mappings first
    if let Some(custom) = custom_mappings {
        if custom.contains_key(&name_lower) || custom.contains_key(name) {
            return true;
        }
        // Check path-stripped version
        if let Some(last_segment) = name.rsplit('/').next() {
            let segment_lower = last_segment.to_lowercase();
            if custom.contains_key(&segment_lower) {
                return true;
            }
        }
    }

    // Check built-in mappings
    if REVERSE_MAP.contains_key(&name_lower) {
        return true;
    }

    // Check path-stripped version
    if let Some(last_segment) = name.rsplit('/').next() {
        let segment_lower = last_segment.to_lowercase();
        if REVERSE_MAP.contains_key(&segment_lower) {
            return true;
        }
    }

    false
}

/// Sort channel indices by: normalized fields first, then alphabetically by display name.
/// Returns a sorted vector of (original_index, display_name, is_normalized).
pub fn sort_channels_by_priority<F>(
    channel_count: usize,
    get_original_name: F,
    field_normalization: bool,
    custom_mappings: Option<&HashMap<String, String>>,
) -> Vec<(usize, String, bool)>
where
    F: Fn(usize) -> String,
{
    let mut channels: Vec<(usize, String, bool)> = (0..channel_count)
        .map(|idx| {
            let original_name = get_original_name(idx);
            let display_name = if field_normalization {
                normalize_channel_name_with_custom(&original_name, custom_mappings)
            } else {
                original_name.clone()
            };
            let is_normalized = has_normalization(&original_name, custom_mappings);
            (idx, display_name, is_normalized)
        })
        .collect();

    // Sort: normalized channels first, then alphabetically by display name
    channels.sort_by(|a, b| {
        // First sort by is_normalized (true comes before false)
        match (a.2, b.2) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            // If both have same normalization status, sort alphabetically (case-insensitive)
            _ => a.1.to_lowercase().cmp(&b.1.to_lowercase()),
        }
    });

    channels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_afr() {
        // Generic AFR channels
        assert_eq!(normalize_channel_name("Act_AFR"), "AFR");
        assert_eq!(normalize_channel_name("R_EGO"), "AFR");
        assert_eq!(normalize_channel_name("Air Fuel Ratio"), "AFR");
        // Wideband O2 overall/generic should normalize to AFR
        assert_eq!(normalize_channel_name("Wideband O2 Overall"), "AFR");
        assert_eq!(normalize_channel_name("Wideband O2"), "AFR");
        assert_eq!(normalize_channel_name("WB O2"), "AFR");
        // Numbered sensors should normalize to AFR Channel 1, AFR Channel 2 (not generic AFR)
        assert_eq!(normalize_channel_name("Wideband O2 1"), "AFR Channel 1");
        assert_eq!(normalize_channel_name("Wideband O2 2"), "AFR Channel 2");
        assert_eq!(normalize_channel_name("WB O2 1"), "AFR Channel 1");
        assert_eq!(normalize_channel_name("Wideband 1"), "AFR Channel 1");
        assert_eq!(normalize_channel_name("Wideband 2"), "AFR Channel 2");
    }

    #[test]
    fn test_normalize_rpm() {
        assert_eq!(normalize_channel_name("RPM"), "RPM");
        assert_eq!(normalize_channel_name("rpm"), "RPM");
        assert_eq!(normalize_channel_name("Engine RPM4"), "RPM");
    }

    #[test]
    fn test_normalize_tps() {
        assert_eq!(normalize_channel_name("TPS"), "TPS");
        assert_eq!(normalize_channel_name("Throttle Position"), "TPS");
        assert_eq!(normalize_channel_name("PedalPos"), "TPS");
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_channel_name("engine/rpm"), "RPM");
    }

    #[test]
    fn test_no_normalization() {
        assert_eq!(normalize_channel_name("CustomChannel"), "CustomChannel");
        assert_eq!(normalize_channel_name("MyUnknownSensor"), "MyUnknownSensor");
    }

    #[test]
    fn test_display_name() {
        assert_eq!(get_display_name("Act_AFR", true), "AFR (Act_AFR)");
        assert_eq!(get_display_name("AFR", true), "AFR");
        assert_eq!(get_display_name("CustomChannel", true), "CustomChannel");
    }
}
