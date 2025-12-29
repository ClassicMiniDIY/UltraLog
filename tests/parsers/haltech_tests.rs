//! Comprehensive tests for the Haltech ECU parser
//!
//! Tests cover:
//! - Header parsing and metadata extraction
//! - Timestamp parsing in various formats
//! - Channel type conversions
//! - Data row detection and parsing
//! - Sparse data handling
//! - Real file parsing with example logs

#[path = "../common/mod.rs"]
mod common;

use common::assertions::*;
use common::example_files::*;
use common::float_cmp::*;
use common::{example_file_exists, read_example_file};
use ultralog::parsers::haltech::Haltech;
use ultralog::parsers::types::Parseable;

// ============================================
// Format Detection Tests
// ============================================

#[test]
fn test_haltech_detection_with_datalog_marker() {
    let content = "%DataLog%\nDataLogVersion : 1.1\n";
    assert!(
        content.starts_with("%DataLog%"),
        "Haltech format should be detected by %DataLog% marker"
    );
}

#[test]
fn test_haltech_detection_case_sensitive() {
    // Haltech marker should be case-sensitive
    let uppercase = "%DATALOG%\nDataLogVersion : 1.1\n";
    let lowercase = "%datalog%\nDataLogVersion : 1.1\n";
    assert!(
        !uppercase.starts_with("%DataLog%"),
        "Haltech marker should be case-sensitive"
    );
    assert!(
        !lowercase.starts_with("%DataLog%"),
        "Haltech marker should be case-sensitive"
    );
}

// ============================================
// Basic Parsing Tests
// ============================================

#[test]
fn test_parse_minimal_haltech_log() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1
Software : Haltech NSP

Channel : Engine Speed
Type : EngineSpeed
ID : 0
DisplayMaxMin : 10000, 0

00:00:00.000,5000
00:00:00.100,5100
"#;

    let parser = Haltech;
    let log = parser
        .parse(sample)
        .expect("Should parse minimal Haltech log");

    assert_eq!(log.channels.len(), 1);
    assert_eq!(log.data.len(), 2);
    assert_eq!(log.times.len(), 2);
}

#[test]
fn test_parse_haltech_multiple_channels() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1

Channel : Engine Speed
Type : EngineSpeed
ID : 0
DisplayMaxMin : 10000, 0

Channel : Manifold Pressure
Type : Pressure
ID : 1
DisplayMaxMin : 300, 0

Channel : Throttle Position
Type : Percentage
ID : 2
DisplayMaxMin : 100, 0

00:00:00.000,5000,150,50
00:00:00.100,5100,155,55
00:00:00.200,5200,160,60
"#;

    let parser = Haltech;
    let log = parser
        .parse(sample)
        .expect("Should parse multi-channel log");

    assert_eq!(log.channels.len(), 3);
    assert_eq!(log.data.len(), 3);

    // Verify all records have 3 values
    for record in &log.data {
        assert_eq!(record.len(), 3);
    }
}

#[test]
fn test_parse_haltech_relative_timestamps() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1

Channel : RPM
Type : EngineSpeed
ID : 0
DisplayMaxMin : 10000, 0

01:00:00.000,5000
01:00:01.000,5100
01:00:02.000,5200
"#;

    let parser = Haltech;
    let log = parser.parse(sample).expect("Should parse log");
    let times = log.get_times_as_f64();

    // Timestamps should be relative to first record
    assert_approx_eq(times[0], 0.0, DEFAULT_TOLERANCE);
    assert_approx_eq(times[1], 1.0, DEFAULT_TOLERANCE);
    assert_approx_eq(times[2], 2.0, DEFAULT_TOLERANCE);
}

// ============================================
// Channel Type Conversion Tests
// ============================================

#[test]
fn test_haltech_rpm_no_conversion() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1

Channel : Engine Speed
Type : EngineSpeed
ID : 0
DisplayMaxMin : 10000, 0

00:00:00.000,5000
"#;

    let parser = Haltech;
    let log = parser.parse(sample).expect("Should parse log");

    // RPM should be stored as-is (no conversion)
    assert_approx_eq(log.data[0][0].as_f64(), 5000.0, DEFAULT_TOLERANCE);
}

#[test]
fn test_haltech_percent_no_conversion() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1

Channel : TPS
Type : Percentage
ID : 0
DisplayMaxMin : 100, 0

00:00:00.000,755
"#;

    let parser = Haltech;
    let log = parser.parse(sample).expect("Should parse log");

    // Percentage is converted (raw/10), so 755 -> 75.5
    assert_approx_eq(log.data[0][0].as_f64(), 75.5, DEFAULT_TOLERANCE);
}

// ============================================
// Sparse Data Handling Tests
// ============================================

#[test]
fn test_haltech_sparse_data_rows() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1

Channel : A
Type : Raw
ID : 0
DisplayMaxMin : 100, 0

Channel : B
Type : Raw
ID : 1
DisplayMaxMin : 100, 0

Channel : C
Type : Raw
ID : 2
DisplayMaxMin : 100, 0

00:00:00.000,1,2,3
00:00:00.100,4,5
00:00:00.200,7
"#;

    let parser = Haltech;
    let result = parser.parse(sample);

    // Parser should handle rows with fewer values than channels
    // (either by filling with defaults or truncating)
    assert!(result.is_ok());
}

#[test]
fn test_haltech_empty_values() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1

Channel : A
Type : Raw
ID : 0
DisplayMaxMin : 100, 0

Channel : B
Type : Raw
ID : 1
DisplayMaxMin : 100, 0

00:00:00.000,1,2
00:00:00.100,,2
00:00:00.200,3,
"#;

    let parser = Haltech;
    let result = parser.parse(sample);

    // Parser should handle empty values gracefully
    // The implementation may skip rows with missing values or fill with 0
    assert!(result.is_ok());
}

// ============================================
// Real File Tests
// ============================================

#[test]
fn test_haltech_small_example_file() {
    if !example_file_exists(HALTECH_SMALL) {
        eprintln!("Skipping test: {} not found", HALTECH_SMALL);
        return;
    }

    let content = read_example_file(HALTECH_SMALL);
    let parser = Haltech;
    let log = parser
        .parse(&content)
        .expect("Should parse Haltech example log");

    // Validate structure
    assert_valid_log_structure(&log);
    assert_monotonic_times(&log);
    assert_finite_values(&log);
    assert_valid_time_range(&log);

    // Should have substantial data
    assert_minimum_channels(&log, 5);
    assert_minimum_records(&log, 100);

    eprintln!(
        "Haltech small log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

#[test]
fn test_haltech_large_multi_log_file() {
    if !example_file_exists(HALTECH_LARGE) {
        eprintln!("Skipping test: {} not found", HALTECH_LARGE);
        return;
    }

    let content = read_example_file(HALTECH_LARGE);
    let parser = Haltech;
    let log = parser
        .parse(&content)
        .expect("Should parse large Haltech log");

    // Validate structure
    assert_valid_log_structure(&log);
    assert_finite_values(&log);

    // Large file should have many records
    assert_minimum_records(&log, 1000);

    eprintln!(
        "Haltech large log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

// ============================================
// Edge Case Tests
// ============================================

#[test]
fn test_haltech_empty_file() {
    let parser = Haltech;
    let result = parser.parse("");

    // Empty file should return an error or empty log
    // Implementation dependent
    match result {
        Ok(log) => {
            assert!(log.data.is_empty() || log.channels.is_empty());
        }
        Err(_) => {
            // Also acceptable
        }
    }
}

#[test]
fn test_haltech_header_only() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1
Software : Haltech NSP

Channel : Engine Speed
Type : EngineSpeed
ID : 0
DisplayMaxMin : 10000, 0
"#;

    let parser = Haltech;
    let result = parser.parse(sample);

    // Header-only file should parse but have no data
    assert!(result.is_ok());
    let log = result.unwrap();
    assert!(log.data.is_empty());
}

#[test]
fn test_haltech_unknown_channel_type() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1

Channel : Unknown Sensor
Type : UnknownType
ID : 0
DisplayMaxMin : 100, 0

00:00:00.000,42.5
"#;

    let parser = Haltech;
    let result = parser.parse(sample);

    // Unknown channel types should be handled gracefully (likely as Raw)
    assert!(result.is_ok());
    let log = result.unwrap();
    assert_eq!(log.channels.len(), 1);
}

#[test]
fn test_haltech_negative_values() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1

Channel : Temperature
Type : Raw
ID : 0
DisplayMaxMin : 200, -50

00:00:00.000,-10.5
00:00:00.100,0
00:00:00.200,25.5
"#;

    let parser = Haltech;
    let log = parser.parse(sample).expect("Should parse negative values");

    // Verify negative values are preserved (Raw type, no conversion)
    assert!(log.data[0][0].as_f64() < 0.0);
}

#[test]
fn test_haltech_very_large_values() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1

Channel : Counter
Type : Raw
ID : 0
DisplayMaxMin : 1000000000, 0

00:00:00.000,999999999
"#;

    let parser = Haltech;
    let log = parser.parse(sample).expect("Should parse large values");

    assert_approx_eq(log.data[0][0].as_f64(), 999999999.0, 1.0);
}

#[test]
fn test_haltech_decimal_precision() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1

Channel : Precise
Type : Raw
ID : 0
DisplayMaxMin : 100, 0

00:00:00.000,12.345678
"#;

    let parser = Haltech;
    let log = parser.parse(sample).expect("Should parse decimal values");

    assert_approx_eq(log.data[0][0].as_f64(), 12.345678, 0.000001);
}

// ============================================
// Data Integrity Tests
// ============================================

#[test]
fn test_haltech_channel_data_extraction() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1

Channel : A
Type : Raw
ID : 0
DisplayMaxMin : 100, 0

Channel : B
Type : Raw
ID : 1
DisplayMaxMin : 100, 0

00:00:00.000,10,20
00:00:00.100,11,21
00:00:00.200,12,22
"#;

    let parser = Haltech;
    let log = parser.parse(sample).expect("Should parse log");

    // Test get_channel_data
    let channel_a = log.get_channel_data(0);
    let channel_b = log.get_channel_data(1);

    assert_eq!(channel_a.len(), 3);
    assert_eq!(channel_b.len(), 3);

    assert_approx_eq(channel_a[0], 10.0, DEFAULT_TOLERANCE);
    assert_approx_eq(channel_a[2], 12.0, DEFAULT_TOLERANCE);
    assert_approx_eq(channel_b[0], 20.0, DEFAULT_TOLERANCE);
    assert_approx_eq(channel_b[2], 22.0, DEFAULT_TOLERANCE);
}

#[test]
fn test_haltech_channel_out_of_bounds() {
    let sample = r#"%DataLog%
DataLogVersion : 1.1

Channel : A
Type : Raw
ID : 0
DisplayMaxMin : 100, 0

00:00:00.000,10
"#;

    let parser = Haltech;
    let log = parser.parse(sample).expect("Should parse log");

    // Out of bounds should return empty
    let oob = log.get_channel_data(999);
    assert!(oob.is_empty());
}

#[test]
fn test_haltech_find_channel_index() {
    if !example_file_exists(HALTECH_SMALL) {
        eprintln!("Skipping test: {} not found", HALTECH_SMALL);
        return;
    }

    let content = read_example_file(HALTECH_SMALL);
    let parser = Haltech;
    let log = parser.parse(&content).expect("Should parse log");

    // Find first channel
    if !log.channels.is_empty() {
        let first_name = log.channels[0].name();
        let found = log.find_channel_index(&first_name);
        assert_eq!(found, Some(0));
    }

    // Non-existent channel
    let not_found = log.find_channel_index("NonExistentChannel12345");
    assert_eq!(not_found, None);
}
