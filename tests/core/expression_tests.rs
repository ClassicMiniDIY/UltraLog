//! Comprehensive tests for formula parsing and evaluation
//!
//! Tests cover:
//! - Channel reference extraction
//! - Formula validation
//! - Formula evaluation with various operators
//! - Time-shift resolution (index and time-based)
//! - Edge cases and error handling

use std::collections::HashMap;
use ultralog::computed::TimeShift;
use ultralog::expression::{
    build_channel_bindings, evaluate_all_records, extract_channel_references, generate_preview,
    validate_formula,
};
use ultralog::parsers::types::Value;

// ============================================
// Channel Reference Extraction Tests
// ============================================

#[test]
fn test_extract_simple_reference() {
    let refs = extract_channel_references("RPM");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "RPM");
    assert_eq!(refs[0].time_shift, TimeShift::None);
}

#[test]
fn test_extract_multiple_references() {
    let refs = extract_channel_references("RPM + Boost - TPS");
    assert_eq!(refs.len(), 3);

    let names: Vec<&str> = refs.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"RPM"));
    assert!(names.contains(&"Boost"));
    assert!(names.contains(&"TPS"));
}

#[test]
fn test_extract_quoted_reference() {
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
fn test_extract_index_offset_no_sign() {
    let refs = extract_channel_references("RPM[3]");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].time_shift, TimeShift::IndexOffset(3));
}

#[test]
fn test_extract_time_offset_negative() {
    let refs = extract_channel_references("RPM@-0.1s");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].time_shift, TimeShift::TimeOffset(-0.1));
}

#[test]
fn test_extract_time_offset_positive() {
    let refs = extract_channel_references("RPM@+0.5s");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].time_shift, TimeShift::TimeOffset(0.5));
}

#[test]
fn test_extract_time_offset_integer() {
    let refs = extract_channel_references("RPM@-1s");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].time_shift, TimeShift::TimeOffset(-1.0));
}

#[test]
fn test_extract_mixed_time_shifts() {
    let refs = extract_channel_references("RPM - RPM[-1] + Boost@-0.1s");
    assert_eq!(refs.len(), 3);

    let has_rpm_current = refs
        .iter()
        .any(|r| r.name == "RPM" && r.time_shift == TimeShift::None);
    let has_rpm_prev = refs
        .iter()
        .any(|r| r.name == "RPM" && r.time_shift == TimeShift::IndexOffset(-1));
    let has_boost = refs
        .iter()
        .any(|r| r.name == "Boost" && r.time_shift == TimeShift::TimeOffset(-0.1));

    assert!(has_rpm_current);
    assert!(has_rpm_prev);
    assert!(has_boost);
}

#[test]
fn test_extract_quoted_with_time_shift() {
    let refs = extract_channel_references("\"Engine Speed\"[-1]");
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "Engine Speed");
    assert_eq!(refs[0].time_shift, TimeShift::IndexOffset(-1));
}

#[test]
fn test_skip_reserved_functions() {
    let refs = extract_channel_references("sin(RPM) + cos(Boost) + sqrt(TPS)");

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
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].name, "RPM");
}

#[test]
fn test_extract_complex_formula() {
    let formula = "(RPM - RPM[-1]) / 0.01 * \"Throttle Position\"@-0.05s";
    let refs = extract_channel_references(formula);

    assert_eq!(refs.len(), 3);
}

#[test]
fn test_extract_empty_formula() {
    let refs = extract_channel_references("");
    assert!(refs.is_empty());
}

#[test]
fn test_extract_constants_only() {
    let refs = extract_channel_references("10 + 20 * 3.14159");
    assert!(refs.is_empty());
}

// ============================================
// Formula Validation Tests
// ============================================

#[test]
fn test_validate_simple_formula() {
    let channels = vec!["RPM".to_string(), "Boost".to_string()];
    let result = validate_formula("RPM + Boost", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_validate_all_operators() {
    let channels = vec!["A".to_string(), "B".to_string()];
    let result = validate_formula("A + B - A * B / A ^ B", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_validate_with_functions() {
    let channels = vec!["X".to_string()];
    let result = validate_formula("sin(X) + cos(X) + sqrt(abs(X))", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_validate_with_time_shifts() {
    let channels = vec!["RPM".to_string()];
    let result = validate_formula("RPM - RPM[-1]", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_validate_missing_channel() {
    let channels = vec!["RPM".to_string()];
    let result = validate_formula("RPM + MissingChannel", &channels);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown"));
}

#[test]
fn test_validate_empty_formula() {
    let channels = vec!["RPM".to_string()];
    let result = validate_formula("", &channels);
    assert!(result.is_err());
}

#[test]
fn test_validate_whitespace_formula() {
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
fn test_validate_case_insensitive() {
    let channels = vec!["RPM".to_string()];
    let result = validate_formula("rpm + Rpm + RPM", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_validate_parentheses() {
    let channels = vec!["X".to_string(), "Y".to_string()];
    let result = validate_formula("((X + Y) * (X - Y))", &channels);
    assert!(result.is_ok());
}

#[test]
fn test_validate_negative_numbers() {
    let channels = vec!["X".to_string()];
    let result = validate_formula("X + (-10)", &channels);
    assert!(result.is_ok());
}

// ============================================
// Formula Evaluation Tests
// ============================================

fn create_test_data() -> (Vec<Vec<Value>>, Vec<f64>) {
    let data = vec![
        vec![Value::Float(100.0), Value::Float(10.0)],
        vec![Value::Float(200.0), Value::Float(20.0)],
        vec![Value::Float(300.0), Value::Float(30.0)],
    ];
    let times = vec![0.0, 0.1, 0.2];
    (data, times)
}

#[test]
fn test_evaluate_addition() {
    let (data, times) = create_test_data();
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
    let (data, times) = create_test_data();
    let mut bindings = HashMap::new();
    bindings.insert("A".to_string(), 0);
    bindings.insert("B".to_string(), 1);

    let result = evaluate_all_records("A * B", &bindings, &data, &times).unwrap();

    assert_eq!(result[0], 1000.0); // 100 * 10
    assert_eq!(result[1], 4000.0); // 200 * 20
    assert_eq!(result[2], 9000.0); // 300 * 30
}

#[test]
fn test_evaluate_with_constants() {
    let (data, times) = create_test_data();
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    let result = evaluate_all_records("X * 0.5 + 10", &bindings, &data, &times).unwrap();

    assert_eq!(result[0], 60.0); // 100 * 0.5 + 10
    assert_eq!(result[1], 110.0); // 200 * 0.5 + 10
    assert_eq!(result[2], 160.0); // 300 * 0.5 + 10
}

#[test]
fn test_evaluate_sqrt() {
    let data = vec![
        vec![Value::Float(4.0)],
        vec![Value::Float(9.0)],
        vec![Value::Float(16.0)],
    ];
    let times = vec![0.0, 0.1, 0.2];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    let result = evaluate_all_records("sqrt(X)", &bindings, &data, &times).unwrap();

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

    let result = evaluate_all_records("RPM - RPM[-1]", &bindings, &data, &times).unwrap();

    assert_eq!(result[0], 0.0); // 1000 - 1000 (clamped)
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

    let result = evaluate_all_records("RPM[+1] - RPM", &bindings, &data, &times).unwrap();

    assert_eq!(result[0], 1000.0); // 2000 - 1000
    assert_eq!(result[1], 1000.0); // 3000 - 2000
    assert_eq!(result[2], 0.0); // 3000 - 3000 (clamped)
}

#[test]
fn test_evaluate_time_offset() {
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

    let result = evaluate_all_records("X - X@-0.1s", &bindings, &data, &times).unwrap();

    assert_eq!(result[0], 0.0); // Clamped to same
    assert_eq!(result[1], 100.0); // 200 - 100
    assert_eq!(result[2], 100.0); // 300 - 200
}

#[test]
fn test_evaluate_division_by_zero() {
    let data = vec![vec![Value::Float(0.0)], vec![Value::Float(1.0)]];
    let times = vec![0.0, 0.1];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    let result = evaluate_all_records("1/X", &bindings, &data, &times).unwrap();

    // Infinity should be converted to 0
    assert_eq!(result[0], 0.0);
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

#[test]
fn test_evaluate_large_offset_clamping() {
    let data = vec![
        vec![Value::Float(1.0)],
        vec![Value::Float(2.0)],
        vec![Value::Float(3.0)],
    ];
    let times = vec![0.0, 0.1, 0.2];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    // Large negative offset clamps to first
    let result = evaluate_all_records("X[-100]", &bindings, &data, &times).unwrap();
    assert_eq!(result[0], 1.0);
    assert_eq!(result[1], 1.0);
    assert_eq!(result[2], 1.0);

    // Large positive offset clamps to last
    let result = evaluate_all_records("X[+100]", &bindings, &data, &times).unwrap();
    assert_eq!(result[0], 3.0);
    assert_eq!(result[1], 3.0);
    assert_eq!(result[2], 3.0);
}

#[test]
fn test_evaluate_single_record() {
    let data = vec![vec![Value::Float(42.0)]];
    let times = vec![0.0];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    let result = evaluate_all_records("X * 2", &bindings, &data, &times).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], 84.0);

    // Time shift on single record
    let result = evaluate_all_records("X - X[-1]", &bindings, &data, &times).unwrap();
    assert_eq!(result[0], 0.0);
}

// ============================================
// Channel Bindings Tests
// ============================================

#[test]
fn test_build_bindings() {
    let channels = vec!["RPM".to_string(), "Boost".to_string(), "TPS".to_string()];
    let refs = extract_channel_references("RPM + Boost");
    let bindings = build_channel_bindings(&refs, &channels).unwrap();

    assert_eq!(bindings.get("RPM"), Some(&0));
    assert_eq!(bindings.get("Boost"), Some(&1));
}

#[test]
fn test_build_bindings_case_insensitive() {
    let channels = vec!["RPM".to_string(), "BOOST".to_string()];
    let refs = extract_channel_references("rpm + boost");
    let bindings = build_channel_bindings(&refs, &channels).unwrap();

    assert_eq!(bindings.get("rpm"), Some(&0));
    assert_eq!(bindings.get("boost"), Some(&1));
}

#[test]
fn test_build_bindings_missing_channel() {
    let channels = vec!["RPM".to_string()];
    let refs = extract_channel_references("RPM + Missing");
    let result = build_channel_bindings(&refs, &channels);

    assert!(result.is_err());
}

// ============================================
// Preview Generation Tests
// ============================================

#[test]
fn test_generate_preview() {
    let data: Vec<Vec<Value>> = (0..10).map(|i| vec![Value::Float(i as f64)]).collect();
    let times: Vec<f64> = (0..10).map(|i| i as f64 * 0.1).collect();
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    let preview = generate_preview("X * 2", &bindings, &data, &times, 5).unwrap();

    assert_eq!(preview.len(), 5);
    assert_eq!(preview[0], 0.0);
    assert_eq!(preview[1], 2.0);
    assert_eq!(preview[2], 4.0);
}

#[test]
fn test_generate_preview_more_than_available() {
    let data = vec![vec![Value::Float(1.0)], vec![Value::Float(2.0)]];
    let times = vec![0.0, 0.1];
    let mut bindings = HashMap::new();
    bindings.insert("X".to_string(), 0);

    let preview = generate_preview("X", &bindings, &data, &times, 100).unwrap();

    // Should return all available data
    assert_eq!(preview.len(), 2);
}

// Note: find_record_at_time tests removed - function is private
// Time offset evaluation is tested indirectly through evaluate_all_records
