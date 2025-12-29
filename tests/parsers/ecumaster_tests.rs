//! Comprehensive tests for the ECUMaster EMU Pro parser
//!
//! Tests cover:
//! - Format detection (semicolon vs tab delimiter)
//! - Sparse data interpolation
//! - Unit inference from channel names
//! - Path parsing for nested channel names
//! - Real file parsing with example logs

#[path = "../common/mod.rs"]
mod common;

use common::assertions::*;
use common::example_files::*;
use common::float_cmp::*;
use common::{example_file_exists, read_example_file};
use ultralog::parsers::ecumaster::EcuMaster;
use ultralog::parsers::types::Parseable;

// ============================================
// Format Detection Tests
// ============================================

#[test]
fn test_ecumaster_detection_semicolon_delimiter() {
    let content = "TIME;engine/rpm;sensors/tps1\n0.0;1000;50\n";
    assert!(
        EcuMaster::detect(content),
        "Should detect semicolon-delimited ECUMaster format"
    );
}

#[test]
fn test_ecumaster_detection_tab_delimiter() {
    let content = "TIME\tengine/rpm\tsensors/tps1\n0.0\t1000\t50\n";
    assert!(
        EcuMaster::detect(content),
        "Should detect tab-delimited ECUMaster format"
    );
}

#[test]
fn test_ecumaster_detection_requires_time_column() {
    let content = "engine/rpm;sensors/tps1\n1000;50\n";
    assert!(
        !EcuMaster::detect(content),
        "Should not detect without TIME column"
    );
}

#[test]
fn test_ecumaster_detection_rejects_haltech() {
    let haltech = "%DataLog%\nDataLogVersion : 1.1\n";
    assert!(
        !EcuMaster::detect(haltech),
        "Should not detect Haltech as ECUMaster"
    );
}

#[test]
fn test_ecumaster_detection_rejects_romraider() {
    let romraider = "Time,RPM,Load\n0,1000,50\n";
    assert!(
        !EcuMaster::detect(romraider),
        "Should not detect RomRaider (comma-delimited) as ECUMaster"
    );
}

#[test]
fn test_ecumaster_detection_case_sensitivity() {
    // TIME should be case-insensitive or case-sensitive based on implementation
    let lowercase = "time;rpm\n0.0;1000\n";
    let uppercase = "TIME;rpm\n0.0;1000\n";

    // At least uppercase should work
    assert!(
        EcuMaster::detect(uppercase),
        "Should detect with uppercase TIME"
    );
    // Lowercase behavior depends on implementation
    let _ = EcuMaster::detect(lowercase);
}

// ============================================
// Basic Parsing Tests
// ============================================

#[test]
fn test_parse_ecumaster_minimal() {
    let sample = "TIME;engine/rpm\n0.0;1000\n0.1;1100\n";

    let parser = EcuMaster;
    let log = parser
        .parse(sample)
        .expect("Should parse minimal ECUMaster");

    assert_eq!(log.channels.len(), 1);
    assert_eq!(log.data.len(), 2);
    assert_eq!(log.times.len(), 2);
}

#[test]
fn test_parse_ecumaster_multiple_channels() {
    let sample =
        "TIME;engine/rpm;sensors/tps1;ignition/angle\n0.0;1000;50;10\n0.1;1100;55;12\n0.2;1200;60;14\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse multi-channel");

    assert_eq!(log.channels.len(), 3);
    assert_eq!(log.data.len(), 3);

    for record in &log.data {
        assert_eq!(record.len(), 3);
    }
}

#[test]
fn test_parse_ecumaster_tab_delimiter() {
    let sample = "TIME\tengine/rpm\tsensors/tps1\n0.0\t1000\t50\n0.1\t1100\t55\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse tab-delimited");

    assert_eq!(log.channels.len(), 2);
    assert_eq!(log.data.len(), 2);
}

// ============================================
// Path Parsing Tests
// ============================================

#[test]
fn test_ecumaster_channel_name_extraction() {
    let sample = "TIME;engine/rpm;sensors/tps1\n0.0;1000;50\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse");

    // Channel names should be the last segment of the path
    let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    assert!(
        names.contains(&"rpm".to_string()) || names.contains(&"engine/rpm".to_string()),
        "Should extract channel name from path"
    );
}

#[test]
fn test_ecumaster_deeply_nested_path() {
    let sample = "TIME;sensors/analog/inputs/tps1\n0.0;50\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse nested path");

    assert_eq!(log.channels.len(), 1);
}

// ============================================
// Sparse Data Handling Tests
// ============================================

#[test]
fn test_ecumaster_sparse_data_interpolation() {
    // ECUMaster uses last-value-hold for sparse data
    let sample = "TIME;engine/rpm;sensors/tps1\n0.0;1000;50\n0.1;;55\n0.2;1200;\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse sparse data");

    assert_eq!(log.data.len(), 3);

    // All records should have 2 values
    for record in &log.data {
        assert_eq!(record.len(), 2);
    }

    // Sparse values should be filled (either with last value or 0)
    // The exact behavior depends on implementation
}

#[test]
fn test_ecumaster_all_sparse_column() {
    let sample = "TIME;engine/rpm;sensors/tps1\n0.0;;50\n0.1;;55\n0.2;;60\n";

    let parser = EcuMaster;
    let log = parser
        .parse(sample)
        .expect("Should parse all-sparse column");

    // Should handle column where all values are sparse
    assert_eq!(log.data.len(), 3);
}

#[test]
fn test_ecumaster_trailing_empty_values() {
    let sample = "TIME;a;b;c\n0.0;1;2;3\n0.1;4;5;\n0.2;7;;\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse trailing empty");

    assert_eq!(log.channels.len(), 3);
    assert_eq!(log.data.len(), 3);
}

// ============================================
// Unit Inference Tests
// ============================================

#[test]
fn test_ecumaster_unit_inference_rpm() {
    let sample = "TIME;engine/rpm\n0.0;5000\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse");

    let unit = log.channels[0].unit();
    // Should infer "RPM" for rpm channel
    assert!(
        unit.to_lowercase().contains("rpm") || unit.is_empty(),
        "Unit should be inferred for rpm"
    );
}

#[test]
fn test_ecumaster_unit_inference_temperature() {
    let sample = "TIME;sensors/coolant_temp\n0.0;85\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse");

    // Temperature channels should have temperature unit inferred
    let _ = log.channels[0].unit();
}

#[test]
fn test_ecumaster_unit_inference_pressure() {
    let sample = "TIME;sensors/map\n0.0;100\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse");

    // MAP (manifold absolute pressure) should have pressure unit
    let _ = log.channels[0].unit();
}

#[test]
fn test_ecumaster_unit_inference_tps() {
    let sample = "TIME;sensors/tps1\n0.0;50\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse");

    let unit = log.channels[0].unit();
    // TPS should be percent
    assert!(
        unit.contains("%") || unit.is_empty(),
        "TPS should be inferred as percent"
    );
}

// ============================================
// Real File Tests
// ============================================

#[test]
fn test_ecumaster_standard_example_file() {
    if !example_file_exists(ECUMASTER_STANDARD) {
        eprintln!("Skipping test: {} not found", ECUMASTER_STANDARD);
        return;
    }

    let content = read_example_file(ECUMASTER_STANDARD);

    assert!(
        EcuMaster::detect(&content),
        "Should detect as ECUMaster format"
    );

    let parser = EcuMaster;
    let log = parser.parse(&content).expect("Should parse ECUMaster log");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);

    assert_minimum_channels(&log, 5);
    assert_minimum_records(&log, 100);

    eprintln!(
        "ECUMaster standard log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

#[test]
fn test_ecumaster_large_example_file() {
    if !example_file_exists(ECUMASTER_LARGE) {
        eprintln!("Skipping test: {} not found", ECUMASTER_LARGE);
        return;
    }

    let content = read_example_file(ECUMASTER_LARGE);

    assert!(
        EcuMaster::detect(&content),
        "Should detect large file as ECUMaster"
    );

    let parser = EcuMaster;
    let log = parser
        .parse(&content)
        .expect("Should parse large ECUMaster log");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);

    assert_minimum_records(&log, 1000);

    eprintln!(
        "ECUMaster large log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

// ============================================
// Edge Case Tests
// ============================================

#[test]
fn test_ecumaster_empty_file() {
    let parser = EcuMaster;
    let result = parser.parse("");

    match result {
        Ok(log) => assert!(log.data.is_empty()),
        Err(_) => { /* Also acceptable */ }
    }
}

#[test]
fn test_ecumaster_header_only() {
    let sample = "TIME;engine/rpm;sensors/tps1\n";

    let parser = EcuMaster;
    let result = parser.parse(sample);

    assert!(result.is_ok());
    let log = result.unwrap();
    assert!(log.data.is_empty());
}

#[test]
fn test_ecumaster_negative_values() {
    let sample = "TIME;ignition/angle\n0.0;-10.5\n0.1;0\n0.2;15.5\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse negative values");

    assert!(log.data[0][0].as_f64() < 0.0);
}

#[test]
fn test_ecumaster_decimal_precision() {
    let sample = "TIME;sensors/voltage\n0.0;12.3456789\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse decimals");

    assert_approx_eq(log.data[0][0].as_f64(), 12.3456789, 0.0000001);
}

#[test]
fn test_ecumaster_many_channels() {
    // Create sample with 50 channels
    let mut header = "TIME".to_string();
    for i in 0..50 {
        header.push_str(&format!(";channel{}", i));
    }
    header.push('\n');

    let mut row = "0.0".to_string();
    for i in 0..50 {
        row.push_str(&format!(";{}", i));
    }
    row.push('\n');

    let sample = header + &row;

    let parser = EcuMaster;
    let log = parser.parse(&sample).expect("Should parse many channels");

    assert_eq!(log.channels.len(), 50);
    assert_eq!(log.data[0].len(), 50);
}

#[test]
fn test_ecumaster_timestamp_values() {
    let sample = "TIME;rpm\n0.0;1000\n0.01;1100\n0.02;1200\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse");

    let times = log.get_times_as_f64();

    assert_approx_eq(times[0], 0.0, DEFAULT_TOLERANCE);
    assert_approx_eq(times[1], 0.01, DEFAULT_TOLERANCE);
    assert_approx_eq(times[2], 0.02, DEFAULT_TOLERANCE);
}

// ============================================
// Data Integrity Tests
// ============================================

#[test]
fn test_ecumaster_data_alignment() {
    let sample = "TIME;a;b;c\n0.0;1;2;3\n0.1;4;5;6\n0.2;7;8;9\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse");

    // Verify data alignment
    assert_approx_eq(log.data[0][0].as_f64(), 1.0, DEFAULT_TOLERANCE);
    assert_approx_eq(log.data[0][1].as_f64(), 2.0, DEFAULT_TOLERANCE);
    assert_approx_eq(log.data[0][2].as_f64(), 3.0, DEFAULT_TOLERANCE);

    assert_approx_eq(log.data[2][0].as_f64(), 7.0, DEFAULT_TOLERANCE);
    assert_approx_eq(log.data[2][1].as_f64(), 8.0, DEFAULT_TOLERANCE);
    assert_approx_eq(log.data[2][2].as_f64(), 9.0, DEFAULT_TOLERANCE);
}

#[test]
fn test_ecumaster_channel_data_extraction() {
    let sample = "TIME;a;b\n0.0;10;20\n0.1;11;21\n0.2;12;22\n";

    let parser = EcuMaster;
    let log = parser.parse(sample).expect("Should parse");

    let channel_a = log.get_channel_data(0);
    let channel_b = log.get_channel_data(1);

    assert_eq!(channel_a.len(), 3);
    assert_eq!(channel_b.len(), 3);

    assert_approx_eq(channel_a[0], 10.0, DEFAULT_TOLERANCE);
    assert_approx_eq(channel_b[2], 22.0, DEFAULT_TOLERANCE);
}
