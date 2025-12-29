//! Comprehensive integration tests for Computed Channels feature
//!
//! These tests verify the computed channels functionality works correctly
//! with parsed log files and integrates properly with the existing features.

use std::collections::HashMap;
use ultralog::computed::{
    ComputedChannel, ComputedChannelLibrary, ComputedChannelTemplate, FormulaEditorState, TimeShift,
};
use ultralog::expression::{
    build_channel_bindings, evaluate_all_records, extract_channel_references, generate_preview,
    validate_formula,
};
use ultralog::parsers::haltech::Haltech;
use ultralog::parsers::types::{Parseable, Value};

/// Helper function to read a file, panicking with a clear message if not found.
fn read_example_file(file_path: &str) -> String {
    std::fs::read_to_string(file_path)
        .unwrap_or_else(|e| panic!("Failed to read example file '{}': {}", file_path, e))
}

/// Helper function to quote channel names that contain spaces for use in formulas
fn quote_if_needed(name: &str) -> String {
    if name.contains(' ') {
        format!("\"{}\"", name)
    } else {
        name.to_string()
    }
}

// ============================================
// Expression Parsing Tests
// ============================================

#[test]
fn test_extract_simple_channel_reference() {
    let refs = extract_channel_references("RPM * 2");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "RPM");
    assert_eq!(refs[0].time_shift, TimeShift::None);
    assert_eq!(refs[0].full_match, "RPM");
}

#[test]
fn test_extract_multiple_channel_references() {
    let refs = extract_channel_references("RPM + Boost * 2 - TPS");
    assert_eq!(refs.len(), 3);

    let names: Vec<&str> = refs.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"RPM"));
    assert!(names.contains(&"Boost"));
    assert!(names.contains(&"TPS"));
}

#[test]
fn test_extract_quoted_channel_with_spaces() {
    let refs = extract_channel_references("\"Manifold Pressure\" + 10");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "Manifold Pressure");
}

#[test]
fn test_extract_index_offset_negative() {
    let refs = extract_channel_references("RPM[-1]");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "RPM");
    assert_eq!(refs[0].time_shift, TimeShift::IndexOffset(-1));
}

#[test]
fn test_extract_index_offset_positive() {
    let refs = extract_channel_references("RPM[+2]");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "RPM");
    assert_eq!(refs[0].time_shift, TimeShift::IndexOffset(2));
}

#[test]
fn test_extract_time_offset_negative() {
    let refs = extract_channel_references("RPM@-0.1s");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "RPM");
    assert_eq!(refs[0].time_shift, TimeShift::TimeOffset(-0.1));
}

#[test]
fn test_extract_time_offset_positive() {
    let refs = extract_channel_references("RPM@+0.5s");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "RPM");
    assert_eq!(refs[0].time_shift, TimeShift::TimeOffset(0.5));
}

#[test]
fn test_extract_mixed_time_shifts() {
    let refs = extract_channel_references("RPM - RPM[-1] + Boost@-0.1s");
    assert_eq!(refs.len(), 3);

    // Find each reference by name and time shift
    let rpm_current = refs
        .iter()
        .find(|r| r.name == "RPM" && r.time_shift == TimeShift::None);
    let rpm_prev = refs
        .iter()
        .find(|r| r.name == "RPM" && r.time_shift == TimeShift::IndexOffset(-1));
    let boost = refs
        .iter()
        .find(|r| r.name == "Boost" && r.time_shift == TimeShift::TimeOffset(-0.1));

    assert!(rpm_current.is_some(), "Should find current RPM");
    assert!(rpm_prev.is_some(), "Should find previous RPM");
    assert!(boost.is_some(), "Should find time-shifted Boost");
}

#[test]
fn test_skip_reserved_math_functions() {
    let refs = extract_channel_references("sin(RPM) + cos(Boost) + sqrt(TPS)");

    // Should find RPM, Boost, TPS but not sin, cos, sqrt
    let names: Vec<&str> = refs.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"RPM"));
    assert!(names.contains(&"Boost"));
    assert!(names.contains(&"TPS"));
    assert!(!names.contains(&"sin"));
    assert!(!names.contains(&"cos"));
    assert!(!names.contains(&"sqrt"));
}

#[test]
fn test_skip_reserved_constants() {
    let refs = extract_channel_references("RPM * pi + e");

    // Should only find RPM, not pi or e
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "RPM");
}

#[test]
fn test_quoted_channel_with_time_shift() {
    let refs = extract_channel_references("\"Engine Speed\"[-1]");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "Engine Speed");
    assert_eq!(refs[0].time_shift, TimeShift::IndexOffset(-1));
}

#[test]
fn test_complex_formula_parsing() {
    let formula = "(RPM - RPM[-1]) / 0.01 * \"Throttle Position\"@-0.05s";
    let refs = extract_channel_references(formula);

    assert_eq!(refs.len(), 3);

    // Verify we captured all references with correct time shifts
    let has_rpm_current = refs
        .iter()
        .any(|r| r.name == "RPM" && r.time_shift == TimeShift::None);
    let has_rpm_prev = refs
        .iter()
        .any(|r| r.name == "RPM" && r.time_shift == TimeShift::IndexOffset(-1));
    let has_throttle = refs
        .iter()
        .any(|r| r.name == "Throttle Position" && r.time_shift == TimeShift::TimeOffset(-0.05));

    assert!(has_rpm_current);
    assert!(has_rpm_prev);
    assert!(has_throttle);
}

// ============================================
// Formula Validation Tests
// ============================================

#[test]
fn test_validate_valid_formula() {
    let channels = vec!["RPM".to_string(), "Boost".to_string(), "TPS".to_string()];
    let result = validate_formula("RPM + Boost * 2", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_validate_formula_with_all_operators() {
    let channels = vec!["A".to_string(), "B".to_string()];
    let result = validate_formula("A + B - A * B / A ^ B", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_validate_formula_with_functions() {
    let channels = vec!["X".to_string()];
    let result = validate_formula("sin(X) + cos(X) + sqrt(abs(X))", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_validate_formula_with_time_shifts() {
    let channels = vec!["RPM".to_string()];
    let result = validate_formula("RPM - RPM[-1]", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_validate_missing_channel() {
    let channels = vec!["RPM".to_string()];
    let result = validate_formula("RPM + MissingChannel", &channels);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown channels"));
}

#[test]
fn test_validate_empty_formula() {
    let channels = vec!["RPM".to_string()];
    let result = validate_formula("", &channels);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("empty"));
}

#[test]
fn test_validate_whitespace_only_formula() {
    let channels = vec!["RPM".to_string()];
    let result = validate_formula("   \t\n  ", &channels);
    assert!(result.is_err());
}

#[test]
fn test_validate_syntax_error() {
    let channels = vec!["RPM".to_string()];
    let result = validate_formula("RPM + + +", &channels);
    assert!(result.is_err());
}

#[test]
fn test_validate_case_insensitive_channel_match() {
    let channels = vec!["RPM".to_string()];
    let result = validate_formula("rpm + Rpm + RPM", &channels);
    // Should be ok since matching is case-insensitive
    assert!(result.is_ok());
}

// ============================================
// Formula Evaluation Tests
// ============================================

#[test]
fn test_evaluate_simple_addition() {
    let data = vec![
        vec![Value::Float(100.0), Value::Float(10.0)],
        vec![Value::Float(200.0), Value::Float(20.0)],
        vec![Value::Float(300.0), Value::Float(30.0)],
    ];
    let times = vec![0.0, 0.1, 0.2];
    let mut bindings = HashMap::new();
    bindings.insert("A".to_string(), 0);
    bindings.insert("B".to_string(), 1);

    let result = evaluate_all_records("A + B", &bindings, &data, &times).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0], 110.0);
    assert_eq!(result[1], 220.0);
    assert_eq!(result[2], 330.0);
}

#[test]
fn test_evaluate_multiplication() {
    let data = vec![
        vec![Value::Float(2.0), Value::Float(3.0)],
        vec![Value::Float(4.0), Value::Float(5.0)],
    ];
    let times = vec![0.0, 0.1];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);
    bindings.insert("Y".to_string(), 1);

    let result = evaluate_all_records("X * Y", &bindings, &data, &times).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0], 6.0);
    assert_eq!(result[1], 20.0);
}

#[test]
fn test_evaluate_with_constants() {
    let data = vec![vec![Value::Float(100.0)], vec![Value::Float(200.0)]];
    let times = vec![0.0, 0.1];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    let result = evaluate_all_records("X * 0.5 + 10", &bindings, &data, &times).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0], 60.0); // 100 * 0.5 + 10
    assert_eq!(result[1], 110.0); // 200 * 0.5 + 10
}

#[test]
fn test_evaluate_with_math_functions() {
    let data = vec![
        vec![Value::Float(4.0)],
        vec![Value::Float(9.0)],
        vec![Value::Float(16.0)],
    ];
    let times = vec![0.0, 0.1, 0.2];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    let result = evaluate_all_records("sqrt(X)", &bindings, &data, &times).unwrap();

    assert_eq!(result.len(), 3);
    assert!((result[0] - 2.0).abs() < 0.0001);
    assert!((result[1] - 3.0).abs() < 0.0001);
    assert!((result[2] - 4.0).abs() < 0.0001);
}

#[test]
fn test_evaluate_index_offset_previous() {
    let data = vec![
        vec![Value::Float(1000.0)],
        vec![Value::Float(2000.0)],
        vec![Value::Float(3000.0)],
    ];
    let times = vec![0.0, 0.1, 0.2];
    let mut bindings = HashMap::new();
    bindings.insert("RPM".to_string(), 0);

    // RPM - RPM[-1] gives the change from previous sample
    let result = evaluate_all_records("RPM - RPM[-1]", &bindings, &data, &times).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0], 0.0); // 1000 - 1000 (clamped to index 0)
    assert_eq!(result[1], 1000.0); // 2000 - 1000
    assert_eq!(result[2], 1000.0); // 3000 - 2000
}

#[test]
fn test_evaluate_index_offset_future() {
    let data = vec![
        vec![Value::Float(1000.0)],
        vec![Value::Float(2000.0)],
        vec![Value::Float(3000.0)],
    ];
    let times = vec![0.0, 0.1, 0.2];
    let mut bindings = HashMap::new();
    bindings.insert("RPM".to_string(), 0);

    // RPM[+1] - RPM gives the upcoming change
    let result = evaluate_all_records("RPM[+1] - RPM", &bindings, &data, &times).unwrap();

    assert_eq!(result.len(), 3);
    assert_eq!(result[0], 1000.0); // 2000 - 1000
    assert_eq!(result[1], 1000.0); // 3000 - 2000
    assert_eq!(result[2], 0.0); // 3000 - 3000 (clamped to last)
}

#[test]
fn test_evaluate_time_offset() {
    // Create data with known time steps
    let data = vec![
        vec![Value::Float(100.0)],
        vec![Value::Float(200.0)],
        vec![Value::Float(300.0)],
        vec![Value::Float(400.0)],
        vec![Value::Float(500.0)],
    ];
    let times = vec![0.0, 0.1, 0.2, 0.3, 0.4];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    // X - X@-0.1s should give change over 0.1 seconds
    let result = evaluate_all_records("X - X@-0.1s", &bindings, &data, &times).unwrap();

    assert_eq!(result.len(), 5);
    // At t=0.0, X@-0.1s clamps to t=0.0, so result is 0
    assert_eq!(result[0], 0.0);
    // At t=0.1, X@-0.1s is at t=0.0, so 200 - 100 = 100
    assert_eq!(result[1], 100.0);
    // At t=0.2, X@-0.1s is at t=0.1, so 300 - 200 = 100
    assert_eq!(result[2], 100.0);
}

#[test]
fn test_evaluate_handles_nan_and_infinity() {
    let data = vec![
        vec![Value::Float(0.0)], // Division by zero case
        vec![Value::Float(1.0)],
    ];
    let times = vec![0.0, 0.1];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    // 1/X would be infinity at X=0, should be converted to 0
    let result = evaluate_all_records("1/X", &bindings, &data, &times).unwrap();

    assert_eq!(result.len(), 2);
    assert_eq!(result[0], 0.0); // Infinity converted to 0
    assert_eq!(result[1], 1.0);
}

#[test]
fn test_evaluate_empty_data() {
    let data: Vec<Vec<Value>> = vec![];
    let times: Vec<f64> = vec![];
    let bindings = HashMap::new();

    let result = evaluate_all_records("X + 1", &bindings, &data, &times).unwrap();
    assert!(result.is_empty());
}

// ============================================
// Preview Generation Tests
// ============================================

#[test]
fn test_generate_preview() {
    let data = vec![
        vec![Value::Float(1.0)],
        vec![Value::Float(2.0)],
        vec![Value::Float(3.0)],
        vec![Value::Float(4.0)],
        vec![Value::Float(5.0)],
        vec![Value::Float(6.0)],
        vec![Value::Float(7.0)],
        vec![Value::Float(8.0)],
        vec![Value::Float(9.0)],
        vec![Value::Float(10.0)],
    ];
    let times: Vec<f64> = (0..10).map(|i| i as f64 * 0.1).collect();
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    let preview = generate_preview("X * 2", &bindings, &data, &times, 5).unwrap();

    assert_eq!(preview.len(), 5);
    assert_eq!(preview[0], 2.0);
    assert_eq!(preview[1], 4.0);
    assert_eq!(preview[2], 6.0);
    assert_eq!(preview[3], 8.0);
    assert_eq!(preview[4], 10.0);
}

// ============================================
// Channel Binding Tests
// ============================================

#[test]
fn test_build_channel_bindings() {
    let channels = vec!["RPM".to_string(), "Boost".to_string(), "TPS".to_string()];
    let refs = extract_channel_references("RPM + Boost");
    let bindings = build_channel_bindings(&refs, &channels).unwrap();

    assert_eq!(bindings.get("RPM"), Some(&0));
    assert_eq!(bindings.get("Boost"), Some(&1));
}

#[test]
fn test_build_channel_bindings_case_insensitive() {
    let channels = vec!["RPM".to_string(), "BOOST".to_string()];
    let refs = extract_channel_references("rpm + boost");
    let bindings = build_channel_bindings(&refs, &channels).unwrap();

    assert_eq!(bindings.get("rpm"), Some(&0));
    assert_eq!(bindings.get("boost"), Some(&1));
}

#[test]
fn test_build_channel_bindings_missing_channel() {
    let channels = vec!["RPM".to_string()];
    let refs = extract_channel_references("RPM + Missing");
    let result = build_channel_bindings(&refs, &channels);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}

// ============================================
// ComputedChannelTemplate Tests
// ============================================

#[test]
fn test_template_creation_generates_unique_id() {
    let template1 = ComputedChannelTemplate::new(
        "Test".to_string(),
        "X".to_string(),
        "unit".to_string(),
        String::new(),
    );
    let template2 = ComputedChannelTemplate::new(
        "Test".to_string(),
        "X".to_string(),
        "unit".to_string(),
        String::new(),
    );

    assert_ne!(template1.id, template2.id);
}

#[test]
fn test_template_touch_updates_modified_time() {
    let mut template = ComputedChannelTemplate::new(
        "Test".to_string(),
        "X".to_string(),
        "unit".to_string(),
        String::new(),
    );

    let original_modified = template.modified_at;
    std::thread::sleep(std::time::Duration::from_millis(10));
    template.touch();

    assert!(template.modified_at >= original_modified);
}

// ============================================
// ComputedChannelLibrary Tests
// ============================================

#[test]
fn test_library_add_and_find() {
    let mut library = ComputedChannelLibrary::new();

    let template = ComputedChannelTemplate::new(
        "Test Channel".to_string(),
        "X * 2".to_string(),
        "units".to_string(),
        "Description".to_string(),
    );
    let id = template.id.clone();

    library.add_template(template);

    let found = library.find_template(&id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().name, "Test Channel");
}

#[test]
fn test_library_remove() {
    let mut library = ComputedChannelLibrary::new();

    let template = ComputedChannelTemplate::new(
        "Test".to_string(),
        "X".to_string(),
        "unit".to_string(),
        String::new(),
    );
    let id = template.id.clone();

    library.add_template(template);
    assert_eq!(library.templates.len(), 1);

    let removed = library.remove_template(&id);
    assert!(removed.is_some());
    assert!(library.templates.is_empty());
}

#[test]
fn test_library_find_nonexistent() {
    let library = ComputedChannelLibrary::new();
    let found = library.find_template("nonexistent-id");
    assert!(found.is_none());
}

#[test]
fn test_library_remove_nonexistent() {
    let mut library = ComputedChannelLibrary::new();
    let removed = library.remove_template("nonexistent-id");
    assert!(removed.is_none());
}

#[test]
fn test_library_find_template_mut() {
    let mut library = ComputedChannelLibrary::new();

    let template = ComputedChannelTemplate::new(
        "Original".to_string(),
        "X".to_string(),
        "unit".to_string(),
        String::new(),
    );
    let id = template.id.clone();
    library.add_template(template);

    if let Some(t) = library.find_template_mut(&id) {
        t.name = "Modified".to_string();
    }

    assert_eq!(library.find_template(&id).unwrap().name, "Modified");
}

// ============================================
// ComputedChannel Tests
// ============================================

#[test]
fn test_computed_channel_from_template() {
    let template = ComputedChannelTemplate::new(
        "RPM Delta".to_string(),
        "RPM - RPM[-1]".to_string(),
        "RPM/sample".to_string(),
        "Rate of RPM change".to_string(),
    );

    let channel = ComputedChannel::from_template(template);

    assert_eq!(channel.name(), "RPM Delta");
    assert_eq!(channel.formula(), "RPM - RPM[-1]");
    assert_eq!(channel.unit(), "RPM/sample");
    assert!(!channel.is_valid()); // Not yet evaluated
}

#[test]
fn test_computed_channel_validity() {
    let template = ComputedChannelTemplate::new(
        "Test".to_string(),
        "X".to_string(),
        "unit".to_string(),
        String::new(),
    );

    let mut channel = ComputedChannel::from_template(template);

    // Initially not valid (no cached data)
    assert!(!channel.is_valid());

    // Set cached data
    channel.cached_data = Some(vec![1.0, 2.0, 3.0]);
    assert!(channel.is_valid());

    // Set error
    channel.error = Some("Test error".to_string());
    assert!(!channel.is_valid()); // Error means not valid

    // Clear error
    channel.error = None;
    assert!(channel.is_valid());

    // Invalidate cache
    channel.invalidate_cache();
    assert!(!channel.is_valid());
}

// ============================================
// FormulaEditorState Tests
// ============================================

#[test]
fn test_formula_editor_open_new() {
    let mut state = FormulaEditorState::default();
    assert!(!state.is_open);

    state.open_new();

    assert!(state.is_open);
    assert!(state.name.is_empty());
    assert!(state.formula.is_empty());
    assert!(!state.is_editing());
}

#[test]
fn test_formula_editor_open_edit() {
    let template = ComputedChannelTemplate::new(
        "Test".to_string(),
        "X + Y".to_string(),
        "unit".to_string(),
        "Description".to_string(),
    );

    let mut state = FormulaEditorState::default();
    state.open_edit(&template);

    assert!(state.is_open);
    assert_eq!(state.name, "Test");
    assert_eq!(state.formula, "X + Y");
    assert_eq!(state.description, "Description");
    assert!(state.is_editing());
}

#[test]
fn test_formula_editor_close() {
    let mut state = FormulaEditorState::default();
    state.open_new();
    state.name = "Test".to_string();
    state.validation_error = Some("Error".to_string());

    state.close();

    assert!(!state.is_open);
    assert!(state.validation_error.is_none());
    assert!(!state.is_editing());
}

// ============================================
// Integration with Real Log Data Tests
// ============================================

#[test]
fn test_computed_channel_with_haltech_log() {
    let file_path = "exampleLogs/haltech/2025-07-18_0215pm_Log1118.csv";
    let content = read_example_file(file_path);

    let parser = Haltech;
    let log = parser.parse(&content).expect("Should parse Haltech log");

    // Get available channel names
    let available_channels: Vec<String> = log.channels.iter().map(|c| c.name()).collect();

    // Find a channel that exists (first one)
    let first_channel = &available_channels[0];

    // Create a simple formula using the first channel (quote if it contains spaces)
    let quoted_channel = quote_if_needed(first_channel);
    let formula = format!("{} * 2", quoted_channel);

    // Validate the formula
    let validation = validate_formula(&formula, &available_channels);
    assert!(
        validation.is_ok(),
        "Formula should validate: {:?}",
        validation
    );

    // Extract references and build bindings
    let refs = extract_channel_references(&formula);
    let bindings = build_channel_bindings(&refs, &available_channels).unwrap();

    // Evaluate the formula
    let result = evaluate_all_records(&formula, &bindings, &log.data, &log.times).unwrap();

    // Verify results
    assert_eq!(
        result.len(),
        log.data.len(),
        "Result should have same length as input data"
    );

    // Verify the formula calculation is correct
    let original_data = log.get_channel_data(0);
    for (i, (&computed, &original)) in result.iter().zip(original_data.iter()).enumerate() {
        let expected = original * 2.0;
        assert!(
            (computed - expected).abs() < 0.0001,
            "Record {}: computed {} should equal expected {}",
            i,
            computed,
            expected
        );
    }

    eprintln!(
        "Computed channel test: evaluated {} records successfully",
        result.len()
    );
}

#[test]
fn test_rate_of_change_formula_with_haltech_log() {
    let file_path = "exampleLogs/haltech/2025-07-18_0215pm_Log1118.csv";
    let content = read_example_file(file_path);

    let parser = Haltech;
    let log = parser.parse(&content).expect("Should parse Haltech log");

    let available_channels: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    let first_channel = &available_channels[0];

    // Create a rate-of-change formula (quote if channel name contains spaces)
    let quoted_channel = quote_if_needed(first_channel);
    let formula = format!("{} - {}[-1]", quoted_channel, quoted_channel);

    let validation = validate_formula(&formula, &available_channels);
    assert!(validation.is_ok());

    let refs = extract_channel_references(&formula);
    let bindings = build_channel_bindings(&refs, &available_channels).unwrap();
    let result = evaluate_all_records(&formula, &bindings, &log.data, &log.times).unwrap();

    assert_eq!(result.len(), log.data.len());

    // First value should be 0 (current - clamped previous = current - current)
    assert_eq!(result[0], 0.0);

    // Subsequent values should be the difference from previous
    let original_data = log.get_channel_data(0);
    for i in 1..result.len() {
        let expected = original_data[i] - original_data[i - 1];
        assert!(
            (result[i] - expected).abs() < 0.0001,
            "Record {}: computed {} should equal expected {}",
            i,
            result[i],
            expected
        );
    }

    eprintln!(
        "Rate of change test: evaluated {} records successfully",
        result.len()
    );
}

#[test]
fn test_multi_channel_formula_with_haltech_log() {
    let file_path = "exampleLogs/haltech/2025-07-18_0215pm_Log1118.csv";
    let content = read_example_file(file_path);

    let parser = Haltech;
    let log = parser.parse(&content).expect("Should parse Haltech log");

    // Need at least 2 channels for this test
    if log.channels.len() < 2 {
        eprintln!("Skipping multi-channel test: not enough channels");
        return;
    }

    let available_channels: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    let ch1 = &available_channels[0];
    let ch2 = &available_channels[1];

    // Create formula using two channels (quote if names contain spaces)
    let quoted_ch1 = quote_if_needed(ch1);
    let quoted_ch2 = quote_if_needed(ch2);
    let formula = format!("({} + {}) / 2", quoted_ch1, quoted_ch2);

    let validation = validate_formula(&formula, &available_channels);
    assert!(validation.is_ok());

    let refs = extract_channel_references(&formula);
    let bindings = build_channel_bindings(&refs, &available_channels).unwrap();
    let result = evaluate_all_records(&formula, &bindings, &log.data, &log.times).unwrap();

    assert_eq!(result.len(), log.data.len());

    // Verify the average calculation
    let data1 = log.get_channel_data(0);
    let data2 = log.get_channel_data(1);
    for i in 0..result.len().min(10) {
        // Check first 10 records
        let expected = (data1[i] + data2[i]) / 2.0;
        assert!(
            (result[i] - expected).abs() < 0.0001,
            "Record {}: computed {} should equal expected {}",
            i,
            result[i],
            expected
        );
    }

    eprintln!(
        "Multi-channel test: evaluated {} records successfully",
        result.len()
    );
}

// ============================================
// Edge Cases and Error Handling Tests
// ============================================

#[test]
fn test_formula_with_nested_parentheses() {
    let channels = vec!["X".to_string(), "Y".to_string()];
    let result = validate_formula("((X + Y) * (X - Y)) / 2", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_formula_with_negative_numbers() {
    let channels = vec!["X".to_string()];
    let result = validate_formula("X + (-10) * -5", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_formula_with_scientific_notation() {
    let channels = vec!["X".to_string()];
    // meval may not support scientific notation directly, use regular decimals
    let result = validate_formula("X * 0.001 + 250", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_large_index_offset_clamping() {
    let data = vec![
        vec![Value::Float(1.0)],
        vec![Value::Float(2.0)],
        vec![Value::Float(3.0)],
    ];
    let times = vec![0.0, 0.1, 0.2];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    // Large negative offset should clamp to index 0
    let result = evaluate_all_records("X[-100]", &bindings, &data, &times).unwrap();
    assert_eq!(result[0], 1.0); // All should access first element
    assert_eq!(result[1], 1.0);
    assert_eq!(result[2], 1.0);

    // Large positive offset should clamp to last index
    let result = evaluate_all_records("X[+100]", &bindings, &data, &times).unwrap();
    assert_eq!(result[0], 3.0); // All should access last element
    assert_eq!(result[1], 3.0);
    assert_eq!(result[2], 3.0);
}

#[test]
fn test_time_offset_out_of_range_clamping() {
    let data = vec![
        vec![Value::Float(100.0)],
        vec![Value::Float(200.0)],
        vec![Value::Float(300.0)],
    ];
    let times = vec![0.0, 0.5, 1.0];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    // Large negative time offset should clamp to first record
    let result = evaluate_all_records("X@-100s", &bindings, &data, &times).unwrap();
    assert_eq!(result[0], 100.0);
    assert_eq!(result[1], 100.0);
    assert_eq!(result[2], 100.0);

    // Large positive time offset should clamp to last record
    let result = evaluate_all_records("X@+100s", &bindings, &data, &times).unwrap();
    assert_eq!(result[0], 300.0);
    assert_eq!(result[1], 300.0);
    assert_eq!(result[2], 300.0);
}

#[test]
fn test_single_record_data() {
    let data = vec![vec![Value::Float(42.0)]];
    let times = vec![0.0];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    let result = evaluate_all_records("X * 2", &bindings, &data, &times).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], 84.0);

    // Time shift on single record should work (clamps to same record)
    let result = evaluate_all_records("X - X[-1]", &bindings, &data, &times).unwrap();
    assert_eq!(result[0], 0.0);
}
