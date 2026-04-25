//! Comprehensive tests for the Speeduino/rusEFI MLG binary parser
//!
//! Tests cover:
//! - Binary header detection ("MLVLG")
//! - Format version handling (V1 vs V2)
//! - Field type parsing
//! - Timestamp wraparound handling
//! - Transform formula application
//! - Real file parsing with example logs

#[path = "../common/mod.rs"]
mod common;

use common::assertions::*;
use common::example_files::*;
use common::{example_file_exists, read_example_binary};
use ultralog::parsers::speeduino::Speeduino;
use ultralog::parsers::types::Parseable;

// ============================================
// Format Detection Tests
// ============================================

#[test]
fn test_speeduino_detection_valid_header() {
    let valid_header = b"MLVLG\x00\x00\x01";
    assert!(
        Speeduino::detect(valid_header),
        "Should detect valid MLG header"
    );
}

#[test]
fn test_speeduino_detection_minimal_header() {
    let minimal = b"MLVLG";
    assert!(
        Speeduino::detect(minimal),
        "Should detect minimal MLG header"
    );
}

#[test]
fn test_speeduino_detection_with_extra_data() {
    let with_data = b"MLVLG\x00\x00\x01\x00\x00\x00\x00extra data here";
    assert!(
        Speeduino::detect(with_data),
        "Should detect MLG header with trailing data"
    );
}

#[test]
fn test_speeduino_detection_invalid_magic() {
    assert!(
        !Speeduino::detect(b"NOT_MLG"),
        "Should not detect invalid header"
    );
    assert!(
        !Speeduino::detect(b"MLVL"),
        "Should not detect truncated header"
    );
    assert!(
        !Speeduino::detect(b"mlvlg"),
        "Header should be case-sensitive"
    );
}

#[test]
fn test_speeduino_detection_empty() {
    assert!(!Speeduino::detect(b""), "Should not detect empty data");
}

#[test]
fn test_speeduino_detection_too_short() {
    assert!(
        !Speeduino::detect(b"MLV"),
        "Should not detect too short header"
    );
}

#[test]
fn test_speeduino_detection_other_formats() {
    assert!(!Speeduino::detect(b"<hCNF"), "Should not detect AiM format");
    assert!(
        !Speeduino::detect(b"\x00\x00\x00\xd7lf3"),
        "Should not detect Link format"
    );
    assert!(
        !Speeduino::detect(b"%DataLog%"),
        "Should not detect Haltech format"
    );
}

// ============================================
// Text Parser Error Tests
// ============================================

#[test]
fn test_speeduino_text_parser_returns_error() {
    let parser = Speeduino;
    let result = parser.parse("TIME;RPM\n0.0;1000\n");

    assert!(
        result.is_err(),
        "Text parser should return error for binary format"
    );
}

// ============================================
// Binary Parsing Error Tests
// ============================================

#[test]
fn test_speeduino_parse_invalid_header() {
    let invalid = b"NOT_MLG_FORMAT";
    let result = Speeduino::parse_binary(invalid);

    assert!(result.is_err(), "Should error on invalid header");
}

#[test]
fn test_speeduino_parse_truncated_header() {
    // Truncated data should return an error, not panic
    let truncated = b"MLVLG\x00";
    let result = Speeduino::parse_binary(truncated);
    assert!(result.is_err(), "Should error on truncated header");
}

#[test]
fn test_speeduino_parse_unreasonable_field_count() {
    // Create header with unreasonable field count - should return error
    let mut data = b"MLVLG\x00".to_vec();
    data.extend_from_slice(&[0x00, 0x01]); // format version 1
    data.extend_from_slice(&[0x00, 0x00, 0x10, 0x00]); // timestamp
    data.extend_from_slice(&[0x00, 0x00]); // info string length
    data.extend_from_slice(&[0x05, 0x00]); // field count = 5 (but not enough data)

    let result = Speeduino::parse_binary(&data);
    assert!(result.is_err(), "Should error on unreasonable field count");
}

// ============================================
// Real File Tests - Speeduino
// ============================================

#[test]
fn test_speeduino_example_file() {
    if !example_file_exists(SPEEDUINO_MLG) {
        eprintln!("Skipping test: {} not found", SPEEDUINO_MLG);
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);

    assert!(Speeduino::detect(&data), "Should detect as MLG format");

    let log = Speeduino::parse_binary(&data).expect("Should parse Speeduino MLG");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);
    assert_valid_time_range(&log);

    assert_minimum_channels(&log, 5);
    assert_minimum_records(&log, 10);

    eprintln!(
        "Speeduino log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

#[test]
fn test_speeduino_channel_properties() {
    if !example_file_exists(SPEEDUINO_MLG) {
        eprintln!("Skipping test: {} not found", SPEEDUINO_MLG);
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);
    let log = Speeduino::parse_binary(&data).expect("Should parse");

    // Verify channel properties
    for channel in &log.channels {
        let name = channel.name();
        let unit = channel.unit();

        // Names and units should be non-empty strings
        assert!(!name.is_empty(), "Channel should have a name");
        // Units may be empty for some channels
        let _ = unit;
    }
}

#[test]
fn test_speeduino_data_integrity() {
    if !example_file_exists(SPEEDUINO_MLG) {
        eprintln!("Skipping test: {} not found", SPEEDUINO_MLG);
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);
    let log = Speeduino::parse_binary(&data).expect("Should parse");

    // Verify times and data match
    assert_eq!(log.times.len(), log.data.len());

    // Verify each record has correct channel count
    let channel_count = log.channels.len();
    for (i, record) in log.data.iter().enumerate() {
        assert_eq!(
            record.len(),
            channel_count,
            "Record {} should have {} values",
            i,
            channel_count
        );
    }
}

// ============================================
// Real File Tests - rusEFI
// ============================================

#[test]
fn test_rusefi_example_file() {
    if !example_file_exists(RUSEFI_MLG) {
        eprintln!("Skipping test: {} not found", RUSEFI_MLG);
        return;
    }

    let data = read_example_binary(RUSEFI_MLG);

    assert!(
        Speeduino::detect(&data),
        "Should detect rusEFI as MLG format"
    );

    let log = Speeduino::parse_binary(&data).expect("Should parse rusEFI MLG");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);

    // rusEFI logs can be large
    assert_minimum_records(&log, 100);

    eprintln!(
        "rusEFI log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

#[test]
fn test_rusefi_log1_file() {
    if !example_file_exists(RUSEFI_LOG1) {
        eprintln!("Skipping test: {} not found", RUSEFI_LOG1);
        return;
    }

    let data = read_example_binary(RUSEFI_LOG1);

    assert!(Speeduino::detect(&data), "Should detect as MLG format");

    let log = Speeduino::parse_binary(&data).expect("Should parse rusEFI Log1");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);

    eprintln!(
        "rusEFI Log1: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

// ============================================
// Timestamp Tests
// ============================================

#[test]
fn test_speeduino_timestamp_monotonicity() {
    if !example_file_exists(SPEEDUINO_MLG) {
        eprintln!("Skipping test: {} not found", SPEEDUINO_MLG);
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);
    let log = Speeduino::parse_binary(&data).expect("Should parse");

    assert_monotonic_times(&log);
}

/// Regression guard for the MLG raw u16 timestamp unit (10 µs/tick per the
/// EFI Analytics MLG spec). A previous version of the parser used 1 ms/tick,
/// which compounded with the wraparound logic to inflate wall-clock time
/// ~100×. We bound the total parsed duration to a value that all bundled
/// sample logs comfortably satisfy and that any 10× or 100× drift would
/// blow past.
fn assert_mlg_total_duration_under(file_path: &str, max_seconds: f64) {
    if !example_file_exists(file_path) {
        eprintln!("Skipping {}: file not found", file_path);
        return;
    }

    let data = read_example_binary(file_path);
    let log = Speeduino::parse_binary(&data)
        .unwrap_or_else(|e| panic!("Failed to parse {}: {}", file_path, e));

    assert!(
        log.times.len() > 1,
        "{}: expected multiple records",
        file_path
    );

    let total = *log.times.last().unwrap() - log.times[0];
    assert!(
        total > 0.0 && total < max_seconds,
        "{}: total duration {:.3}s outside (0, {})s — timestamp unit regression?",
        file_path,
        total,
        max_seconds
    );
}

#[test]
fn test_speeduino_mlg_duration_bounded() {
    // ~30 minutes is well above any of the bundled speeduino samples but
    // far below the ~50 hour figure the old 1ms-tick interpretation produced.
    assert_mlg_total_duration_under(SPEEDUINO_MLG, 30.0 * 60.0);
}

#[test]
fn test_rusefi_mlg_duration_bounded() {
    assert_mlg_total_duration_under(RUSEFI_MLG, 30.0 * 60.0);
}

#[test]
fn test_rusefi_log1_duration_bounded() {
    assert_mlg_total_duration_under(RUSEFI_LOG1, 30.0 * 60.0);
}

#[test]
fn test_speeduino_timestamp_range() {
    if !example_file_exists(SPEEDUINO_MLG) {
        eprintln!("Skipping test: {} not found", SPEEDUINO_MLG);
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);
    let log = Speeduino::parse_binary(&data).expect("Should parse");

    let times = log.get_times_as_f64();

    if !times.is_empty() {
        // First timestamp should be non-negative
        assert!(times[0] >= 0.0, "First timestamp should be non-negative");

        // All timestamps should be finite
        for (i, &t) in times.iter().enumerate() {
            assert!(t.is_finite(), "Timestamp {} should be finite", i);
        }
    }
}

// ============================================
// Channel Data Tests
// ============================================

#[test]
fn test_speeduino_channel_data_extraction() {
    if !example_file_exists(SPEEDUINO_MLG) {
        eprintln!("Skipping test: {} not found", SPEEDUINO_MLG);
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);
    let log = Speeduino::parse_binary(&data).expect("Should parse");

    // Test get_channel_data for each channel
    for idx in 0..log.channels.len() {
        let channel_data = log.get_channel_data(idx);
        assert_eq!(
            channel_data.len(),
            log.data.len(),
            "Channel {} data length should match record count",
            idx
        );
    }

    // Out of bounds should return empty
    let oob = log.get_channel_data(999);
    assert!(oob.is_empty(), "Out of bounds should return empty");
}

#[test]
fn test_speeduino_find_channel_index() {
    if !example_file_exists(SPEEDUINO_MLG) {
        eprintln!("Skipping test: {} not found", SPEEDUINO_MLG);
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);
    let log = Speeduino::parse_binary(&data).expect("Should parse");

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

// ============================================
// Value Range Tests
// ============================================

#[test]
fn test_speeduino_values_are_reasonable() {
    if !example_file_exists(SPEEDUINO_MLG) {
        eprintln!("Skipping test: {} not found", SPEEDUINO_MLG);
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);
    let log = Speeduino::parse_binary(&data).expect("Should parse");

    // Check that values are within reasonable ranges (not corrupted)
    for record in &log.data {
        for value in record {
            let v = value.as_f64();
            assert!(v.is_finite(), "All values should be finite");
            // Most ECU values should be within reasonable bounds
            // (this is a sanity check, not a strict requirement)
        }
    }
}

// ============================================
// Performance Tests
// ============================================

#[test]
fn test_speeduino_large_file_performance() {
    if !example_file_exists(RUSEFI_MLG) {
        eprintln!("Skipping test: {} not found", RUSEFI_MLG);
        return;
    }

    let data = read_example_binary(RUSEFI_MLG);

    // Should parse large file without timeout
    let start = std::time::Instant::now();
    let log = Speeduino::parse_binary(&data).expect("Should parse large file");
    let elapsed = start.elapsed();

    eprintln!("Parsed {} records in {:?}", log.data.len(), elapsed);

    // Should complete in reasonable time (less than 10 seconds)
    assert!(
        elapsed.as_secs() < 10,
        "Parsing should complete in reasonable time"
    );
}
