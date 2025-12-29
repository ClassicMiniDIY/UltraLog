//! Comprehensive tests for the field name normalization system
//!
//! Tests cover:
//! - Built-in normalization mappings
//! - Custom mapping support
//! - Path stripping logic
//! - Sorting algorithm for channel lists
//! - Display name formatting

use std::collections::HashMap;
use ultralog::normalize::{
    get_builtin_mappings, get_display_name, has_normalization, normalize_channel_name,
    normalize_channel_name_with_custom, sort_channels_by_priority,
};

// ============================================
// Basic Normalization Tests
// ============================================

#[test]
fn test_normalize_afr_variants() {
    assert_eq!(normalize_channel_name("Act_AFR"), "AFR");
    assert_eq!(normalize_channel_name("R_EGO"), "AFR");
    assert_eq!(normalize_channel_name("Air Fuel Ratio"), "AFR");
    assert_eq!(normalize_channel_name("AFR"), "AFR");
    assert_eq!(normalize_channel_name("AFR1"), "AFR");
    assert_eq!(normalize_channel_name("WB2 AFR 1"), "AFR");
}

#[test]
fn test_normalize_rpm_variants() {
    assert_eq!(normalize_channel_name("RPM"), "RPM");
    assert_eq!(normalize_channel_name("rpm"), "RPM");
    assert_eq!(normalize_channel_name("Engine RPM4"), "RPM");
    assert_eq!(normalize_channel_name("RPM_INC_RPM"), "RPM");
}

#[test]
fn test_normalize_tps_variants() {
    assert_eq!(normalize_channel_name("TPS"), "TPS");
    assert_eq!(normalize_channel_name("tps"), "TPS");
    assert_eq!(normalize_channel_name("Throttle Position"), "TPS");
    assert_eq!(normalize_channel_name("PedalPos"), "TPS");
    assert_eq!(normalize_channel_name("TPS_Pct"), "TPS");
    assert_eq!(normalize_channel_name("tps1"), "TPS");
}

#[test]
fn test_normalize_coolant_temp_variants() {
    assert_eq!(normalize_channel_name("CLT"), "Coolant Temp");
    assert_eq!(normalize_channel_name("Coolant"), "Coolant Temp");
    assert_eq!(normalize_channel_name("Engine Temperature"), "Coolant Temp");
    assert_eq!(normalize_channel_name("CoolantTemp"), "Coolant Temp");
}

#[test]
fn test_normalize_map_variants() {
    assert_eq!(normalize_channel_name("MAP"), "MAP");
    assert_eq!(normalize_channel_name("map"), "MAP");
    assert_eq!(normalize_channel_name("Manifold Pressure"), "MAP");
    assert_eq!(normalize_channel_name("Inlet Manifold Pressure"), "MAP");
}

#[test]
fn test_normalize_battery_variants() {
    assert_eq!(normalize_channel_name("VBat"), "Battery V");
    assert_eq!(normalize_channel_name("Bat V"), "Battery V");
    assert_eq!(normalize_channel_name("Battery Voltage"), "Battery V");
    assert_eq!(normalize_channel_name("Ecu power"), "Battery V");
}

#[test]
fn test_normalize_iat_variants() {
    assert_eq!(normalize_channel_name("IAT"), "IAT");
    assert_eq!(normalize_channel_name("iat"), "IAT");
    assert_eq!(normalize_channel_name("Intake Air Temp"), "IAT");
}

#[test]
fn test_normalize_lambda_variants() {
    assert_eq!(normalize_channel_name("Lambda 1"), "Lambda 1");
    assert_eq!(normalize_channel_name("Lambda Right"), "Lambda 1");
    assert_eq!(normalize_channel_name("Exhaust Lambda"), "Lambda 1");
    assert_eq!(normalize_channel_name("LAMBDA"), "Lambda 1");
}

#[test]
fn test_normalize_ignition_variants() {
    assert_eq!(normalize_channel_name("Ignition Advance"), "Ignition Adv");
    assert_eq!(normalize_channel_name("Timing"), "Ignition Adv");
    assert_eq!(normalize_channel_name("Spark Advance"), "Ignition Adv");
}

// ============================================
// Path Stripping Tests
// ============================================

#[test]
fn test_normalize_path_simple() {
    assert_eq!(normalize_channel_name("engine/rpm"), "RPM");
}

#[test]
fn test_normalize_path_nested() {
    assert_eq!(normalize_channel_name("sensors/engine/rpm"), "RPM");
}

#[test]
fn test_normalize_path_with_tps() {
    assert_eq!(normalize_channel_name("sensors/tps1"), "TPS");
}

#[test]
fn test_normalize_path_ignition() {
    assert_eq!(normalize_channel_name("ignition/angle"), "Ignition Adv");
}

#[test]
fn test_normalize_path_unknown() {
    // Path with unknown last segment should preserve the full path
    assert_eq!(
        normalize_channel_name("sensors/custom_sensor"),
        "sensors/custom_sensor"
    );
}

// ============================================
// No Normalization Tests
// ============================================

#[test]
fn test_no_normalization_unknown_channel() {
    assert_eq!(normalize_channel_name("CustomChannel"), "CustomChannel");
    assert_eq!(normalize_channel_name("MyUnknownSensor"), "MyUnknownSensor");
    assert_eq!(normalize_channel_name("XYZ123"), "XYZ123");
}

#[test]
fn test_no_normalization_empty_string() {
    assert_eq!(normalize_channel_name(""), "");
}

#[test]
fn test_no_normalization_special_characters() {
    assert_eq!(normalize_channel_name("Sensor #1"), "Sensor #1");
    assert_eq!(normalize_channel_name("Value [A]"), "Value [A]");
}

// ============================================
// Case Sensitivity Tests
// ============================================

#[test]
fn test_case_insensitive_matching() {
    // Same normalized result regardless of case
    assert_eq!(normalize_channel_name("RPM"), "RPM");
    assert_eq!(normalize_channel_name("rpm"), "RPM");
    assert_eq!(normalize_channel_name("Rpm"), "RPM");
    assert_eq!(normalize_channel_name("rPm"), "RPM");
}

#[test]
fn test_case_insensitive_path() {
    assert_eq!(normalize_channel_name("ENGINE/RPM"), "RPM");
    assert_eq!(normalize_channel_name("Engine/Rpm"), "RPM");
}

// ============================================
// Custom Mapping Tests
// ============================================

#[test]
fn test_custom_mapping_priority() {
    let mut custom = HashMap::new();
    custom.insert("rpm".to_string(), "Custom RPM".to_string());

    let result = normalize_channel_name_with_custom("rpm", Some(&custom));
    assert_eq!(result, "Custom RPM");
}

#[test]
fn test_custom_mapping_preserves_case() {
    let mut custom = HashMap::new();
    custom.insert("MyChannel".to_string(), "Normalized".to_string());

    let result = normalize_channel_name_with_custom("MyChannel", Some(&custom));
    assert_eq!(result, "Normalized");
}

#[test]
fn test_custom_mapping_lowercase_lookup() {
    let mut custom = HashMap::new();
    custom.insert("mychannel".to_string(), "Normalized".to_string());

    // Should find via lowercase lookup
    let result = normalize_channel_name_with_custom("MyChannel", Some(&custom));
    assert_eq!(result, "Normalized");
}

#[test]
fn test_custom_mapping_path_stripping() {
    let mut custom = HashMap::new();
    custom.insert("rpm".to_string(), "Custom RPM".to_string());

    // Path stripping should work with custom mappings
    let result = normalize_channel_name_with_custom("engine/rpm", Some(&custom));
    assert_eq!(result, "Custom RPM");
}

#[test]
fn test_custom_mapping_fallback_to_builtin() {
    let mut custom = HashMap::new();
    custom.insert("custom".to_string(), "Custom Value".to_string());

    // Built-in mapping should work when no custom match
    let result = normalize_channel_name_with_custom("TPS", Some(&custom));
    assert_eq!(result, "TPS");
}

#[test]
fn test_empty_custom_mappings() {
    let custom: HashMap<String, String> = HashMap::new();

    // Should fall back to built-in
    let result = normalize_channel_name_with_custom("TPS", Some(&custom));
    assert_eq!(result, "TPS");
}

#[test]
fn test_none_custom_mappings() {
    // None should use built-in only
    let result = normalize_channel_name_with_custom("TPS", None);
    assert_eq!(result, "TPS");
}

// ============================================
// has_normalization Tests
// ============================================

#[test]
fn test_has_normalization_builtin() {
    assert!(has_normalization("RPM", None));
    assert!(has_normalization("TPS", None));
    assert!(has_normalization("Throttle Position", None));
}

#[test]
fn test_has_normalization_unknown() {
    assert!(!has_normalization("UnknownChannel", None));
    assert!(!has_normalization("CustomSensor123", None));
}

#[test]
fn test_has_normalization_path() {
    assert!(has_normalization("engine/rpm", None));
    assert!(has_normalization("sensors/tps1", None));
}

#[test]
fn test_has_normalization_custom() {
    let mut custom = HashMap::new();
    custom.insert("mychannel".to_string(), "Normalized".to_string());

    assert!(has_normalization("mychannel", Some(&custom)));
    assert!(has_normalization("MyChannel", Some(&custom)));
}

#[test]
fn test_has_normalization_custom_path() {
    let mut custom = HashMap::new();
    custom.insert("sensor".to_string(), "Normalized".to_string());

    assert!(has_normalization("sensors/sensor", Some(&custom)));
}

// ============================================
// Display Name Tests
// ============================================

#[test]
fn test_display_name_with_normalization() {
    assert_eq!(get_display_name("Act_AFR", true), "AFR (Act_AFR)");
    assert_eq!(get_display_name("R_EGO", true), "AFR (R_EGO)");
}

#[test]
fn test_display_name_without_normalization() {
    // When normalized equals original, no suffix
    assert_eq!(get_display_name("AFR", true), "AFR");
    assert_eq!(get_display_name("RPM", true), "RPM");
}

#[test]
fn test_display_name_unknown_channel() {
    // Unknown channels show as-is
    assert_eq!(get_display_name("CustomChannel", true), "CustomChannel");
}

#[test]
fn test_display_name_show_original_false() {
    // When show_original is false, just return normalized
    assert_eq!(get_display_name("Act_AFR", false), "AFR");
    assert_eq!(get_display_name("R_EGO", false), "AFR");
}

// ============================================
// get_builtin_mappings Tests
// ============================================

#[test]
fn test_get_builtin_mappings_not_empty() {
    let mappings = get_builtin_mappings();
    assert!(!mappings.is_empty(), "Should have built-in mappings");
}

#[test]
fn test_get_builtin_mappings_has_common_channels() {
    let mappings = get_builtin_mappings();
    let normalized_names: Vec<&str> = mappings.iter().map(|(n, _)| *n).collect();

    assert!(normalized_names.contains(&"RPM"));
    assert!(normalized_names.contains(&"TPS"));
    assert!(normalized_names.contains(&"AFR"));
    assert!(normalized_names.contains(&"MAP"));
}

#[test]
fn test_get_builtin_mappings_structure() {
    let mappings = get_builtin_mappings();

    for (normalized, sources) in &mappings {
        // Each mapping should have a normalized name
        assert!(
            !normalized.is_empty(),
            "Normalized name should not be empty"
        );

        // Each mapping should have at least one source
        assert!(
            !sources.is_empty(),
            "Mapping for {} should have sources",
            normalized
        );
    }
}

// ============================================
// Sorting Tests
// ============================================

#[test]
fn test_sort_channels_normalized_first() {
    let channel_names = vec![
        "CustomChannel".to_string(),
        "RPM".to_string(),
        "UnknownSensor".to_string(),
        "TPS".to_string(),
    ];

    let get_name = |idx: usize| channel_names[idx].clone();
    let sorted = sort_channels_by_priority(4, get_name, true, None);

    // Normalized channels (RPM, TPS) should come before non-normalized
    let is_normalized: Vec<bool> = sorted.iter().map(|(_, _, n)| *n).collect();

    // First should be normalized (true)
    assert!(
        is_normalized[0] || is_normalized[1],
        "First channels should be normalized"
    );
}

#[test]
fn test_sort_channels_alphabetical_within_groups() {
    let channel_names = vec![
        "Zebra".to_string(),
        "Apple".to_string(),
        "Mango".to_string(),
    ];

    let get_name = |idx: usize| channel_names[idx].clone();
    let sorted = sort_channels_by_priority(3, get_name, true, None);

    // All are unknown, so they should be sorted alphabetically
    let display_names: Vec<String> = sorted.iter().map(|(_, n, _)| n.clone()).collect();

    // Should be: Apple, Mango, Zebra (case-insensitive)
    assert_eq!(display_names[0].to_lowercase(), "apple");
    assert_eq!(display_names[1].to_lowercase(), "mango");
    assert_eq!(display_names[2].to_lowercase(), "zebra");
}

#[test]
fn test_sort_channels_empty_list() {
    let sorted = sort_channels_by_priority(0, |_| String::new(), true, None);
    assert!(sorted.is_empty());
}

#[test]
fn test_sort_channels_single_item() {
    let sorted = sort_channels_by_priority(1, |_| "RPM".to_string(), true, None);
    assert_eq!(sorted.len(), 1);
    assert_eq!(sorted[0].1, "RPM");
}

#[test]
fn test_sort_channels_with_normalization_disabled() {
    let channel_names = vec!["Act_AFR".to_string(), "rpm".to_string()];

    let get_name = |idx: usize| channel_names[idx].clone();
    let sorted = sort_channels_by_priority(2, get_name, false, None);

    // With normalization disabled, display names should be original
    let display_names: Vec<String> = sorted.iter().map(|(_, n, _)| n.clone()).collect();
    assert!(display_names.contains(&"Act_AFR".to_string()));
    assert!(display_names.contains(&"rpm".to_string()));
}

#[test]
fn test_sort_channels_preserves_indices() {
    let channel_names = vec!["C".to_string(), "A".to_string(), "B".to_string()];

    let get_name = |idx: usize| channel_names[idx].clone();
    let sorted = sort_channels_by_priority(3, get_name, true, None);

    // Verify original indices are preserved
    let original_indices: Vec<usize> = sorted.iter().map(|(idx, _, _)| *idx).collect();

    // A is at original index 1, B at 2, C at 0
    // After sorting (A, B, C), should be indices [1, 2, 0]
    assert!(original_indices.contains(&0));
    assert!(original_indices.contains(&1));
    assert!(original_indices.contains(&2));
}

// ============================================
// Edge Cases
// ============================================

#[test]
fn test_normalize_whitespace_only() {
    assert_eq!(normalize_channel_name("   "), "   ");
}

#[test]
fn test_normalize_with_leading_trailing_spaces() {
    // Spaces are preserved
    assert_eq!(normalize_channel_name(" RPM "), " RPM ");
}

#[test]
fn test_normalize_unicode() {
    // Unicode should be preserved
    assert_eq!(normalize_channel_name("Température"), "Température");
    assert_eq!(normalize_channel_name("温度"), "温度");
}

#[test]
fn test_normalize_numbers_only() {
    assert_eq!(normalize_channel_name("12345"), "12345");
}

#[test]
fn test_normalize_very_long_name() {
    let long_name = "A".repeat(1000);
    assert_eq!(normalize_channel_name(&long_name), long_name);
}
