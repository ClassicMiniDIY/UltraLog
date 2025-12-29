//! Comprehensive tests for the AiM XRK/DRK binary parser
//!
//! Tests cover:
//! - Binary header detection ("<hCNF")
//! - Real file parsing with example logs
//! - Channel and data validation

#[path = "../common/mod.rs"]
mod common;

use common::assertions::*;
use common::example_files::*;
use common::{example_file_exists, get_example_file_path, read_example_binary};
use std::path::Path;
use ultralog::parsers::aim::Aim;

// ============================================
// Format Detection Tests
// ============================================

#[test]
fn test_aim_detection_valid_xrk_header() {
    let valid_header = b"<hCNF\x00\x3c\xa5\x00\x00";
    assert!(Aim::detect(valid_header), "Should detect valid XRK header");
}

#[test]
fn test_aim_detection_minimal_header() {
    let minimal = b"<hCNF";
    assert!(Aim::detect(minimal), "Should detect minimal XRK header");
}

#[test]
fn test_aim_detection_with_extra_data() {
    let with_data = b"<hCNF\x00extra binary data here";
    assert!(
        Aim::detect(with_data),
        "Should detect XRK header with trailing data"
    );
}

#[test]
fn test_aim_detection_invalid_magic() {
    assert!(!Aim::detect(b"MLVLG"), "Should not detect Speeduino format");
    assert!(!Aim::detect(b"<hCN"), "Should not detect truncated header");
    assert!(!Aim::detect(b"<HCNF"), "Header should be case-sensitive");
}

#[test]
fn test_aim_detection_empty() {
    assert!(!Aim::detect(b""), "Should not detect empty data");
}

#[test]
fn test_aim_detection_too_short() {
    assert!(!Aim::detect(b"<hC"), "Should not detect too short header");
}

#[test]
fn test_aim_detection_other_formats() {
    assert!(!Aim::detect(b"MLVLG"), "Should not detect MLG format");
    assert!(
        !Aim::detect(b"\x00\x00\x00\xd7lf3"),
        "Should not detect Link format"
    );
    assert!(
        !Aim::detect(b"%DataLog%"),
        "Should not detect Haltech format"
    );
}

// ============================================
// Real File Tests (using parse_file)
// ============================================

#[test]
fn test_aim_generic_example_file() {
    if !example_file_exists(AIM_GENERIC) {
        eprintln!("Skipping test: {} not found", AIM_GENERIC);
        return;
    }

    // First verify detection
    let data = read_example_binary(AIM_GENERIC);
    assert!(Aim::detect(&data), "Should detect as AiM XRK format");

    // Parse using parse_file
    let path = get_example_file_path(AIM_GENERIC);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse AiM XRK");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);
    assert_valid_time_range(&log);

    assert_minimum_channels(&log, 3);
    assert_minimum_records(&log, 100);

    eprintln!(
        "AiM generic log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

#[test]
fn test_aim_race_file_1() {
    if !example_file_exists(AIM_RACE_1) {
        eprintln!("Skipping test: {} not found", AIM_RACE_1);
        return;
    }

    let data = read_example_binary(AIM_RACE_1);
    assert!(Aim::detect(&data), "Should detect as AiM XRK format");

    let path = get_example_file_path(AIM_RACE_1);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse AiM race file");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);

    eprintln!(
        "AiM race 1 log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

#[test]
fn test_aim_race_file_2() {
    if !example_file_exists(AIM_RACE_2) {
        eprintln!("Skipping test: {} not found", AIM_RACE_2);
        return;
    }

    let data = read_example_binary(AIM_RACE_2);
    assert!(Aim::detect(&data), "Should detect as AiM XRK format");

    let path = get_example_file_path(AIM_RACE_2);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse AiM race file 2");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);

    eprintln!(
        "AiM race 2 log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

// ============================================
// Channel Tests
// ============================================

#[test]
fn test_aim_channel_properties() {
    if !example_file_exists(AIM_GENERIC) {
        eprintln!("Skipping test: {} not found", AIM_GENERIC);
        return;
    }

    let path = get_example_file_path(AIM_GENERIC);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse");

    // Verify channel properties
    for channel in &log.channels {
        let name = channel.name();

        // Names should be non-empty
        assert!(!name.is_empty(), "Channel should have a name");

        // Names should be printable ASCII (or at least not contain control chars)
        for c in name.chars() {
            assert!(
                c.is_ascii_graphic() || c == ' ',
                "Channel name should contain printable chars: {:?}",
                name
            );
        }
    }
}

#[test]
fn test_aim_channel_data_extraction() {
    if !example_file_exists(AIM_GENERIC) {
        eprintln!("Skipping test: {} not found", AIM_GENERIC);
        return;
    }

    let path = get_example_file_path(AIM_GENERIC);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse");

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
fn test_aim_find_channel_index() {
    if !example_file_exists(AIM_GENERIC) {
        eprintln!("Skipping test: {} not found", AIM_GENERIC);
        return;
    }

    let path = get_example_file_path(AIM_GENERIC);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse");

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
// Data Integrity Tests
// ============================================

#[test]
fn test_aim_data_structure() {
    if !example_file_exists(AIM_GENERIC) {
        eprintln!("Skipping test: {} not found", AIM_GENERIC);
        return;
    }

    let path = get_example_file_path(AIM_GENERIC);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse");

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

#[test]
fn test_aim_timestamp_monotonicity() {
    if !example_file_exists(AIM_GENERIC) {
        eprintln!("Skipping test: {} not found", AIM_GENERIC);
        return;
    }

    let path = get_example_file_path(AIM_GENERIC);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse");

    // AiM timestamps are generated from sample index
    // They should be monotonically increasing
    assert_monotonic_times(&log);
}

#[test]
fn test_aim_values_are_finite() {
    if !example_file_exists(AIM_GENERIC) {
        eprintln!("Skipping test: {} not found", AIM_GENERIC);
        return;
    }

    let path = get_example_file_path(AIM_GENERIC);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse");

    assert_finite_values(&log);
}

// ============================================
// Metadata Tests
// ============================================

#[test]
fn test_aim_metadata_extraction() {
    if !example_file_exists(AIM_GENERIC) {
        eprintln!("Skipping test: {} not found", AIM_GENERIC);
        return;
    }

    let path = get_example_file_path(AIM_GENERIC);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse");

    // AiM files may have vehicle, venue, championship metadata
    // Access via log.meta if available
    let _ = &log.meta;
}

// ============================================
// All XRK Files Test
// ============================================

#[test]
fn test_aim_all_example_files() {
    let aim_files = [AIM_GENERIC, AIM_RACE_1, AIM_RACE_2];

    for file_path in aim_files {
        if !example_file_exists(file_path) {
            eprintln!("Skipping: {} not found", file_path);
            continue;
        }

        let data = read_example_binary(file_path);

        assert!(
            Aim::detect(&data),
            "Should detect {} as AiM format",
            file_path
        );

        let path = get_example_file_path(file_path);
        let result = Aim::parse_file(Path::new(&path));
        assert!(result.is_ok(), "Should parse {} without error", file_path);

        let log = result.unwrap();

        assert!(
            !log.channels.is_empty(),
            "{} should have channels",
            file_path
        );
        assert!(!log.data.is_empty(), "{} should have data", file_path);

        eprintln!(
            "{}: {} channels, {} records",
            file_path,
            log.channels.len(),
            log.data.len()
        );
    }
}

// ============================================
// Performance Tests
// ============================================

#[test]
fn test_aim_large_file_performance() {
    // Use the largest AiM file available
    if !example_file_exists(AIM_RACE_2) {
        eprintln!("Skipping test: {} not found", AIM_RACE_2);
        return;
    }

    let path = get_example_file_path(AIM_RACE_2);

    let start = std::time::Instant::now();
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse large file");
    let elapsed = start.elapsed();

    eprintln!("Parsed {} records in {:?}", log.data.len(), elapsed);

    // Should complete in reasonable time
    assert!(
        elapsed.as_secs() < 30,
        "Parsing should complete in reasonable time"
    );
}
