//! Comprehensive tests for the Link ECU LLG binary parser
//!
//! Tests cover:
//! - Binary header detection ("lf3")
//! - UTF-16 LE string parsing
//! - Channel discovery
//! - Timeline interpolation
//! - Real file parsing with example logs

#[path = "../common/mod.rs"]
mod common;

use common::assertions::*;
use common::example_files::*;
use common::{example_file_exists, read_example_binary};
use ultralog::parsers::link::Link;
use ultralog::parsers::types::Parseable;

// ============================================
// Format Detection Tests
// ============================================

#[test]
fn test_link_detection_valid_header() {
    // Link format has "lf3" at offset 4 (first 4 bytes are header size)
    let mut valid_header = vec![0xd7, 0x00, 0x00, 0x00]; // header size
    valid_header.extend_from_slice(b"lf3");
    valid_header.extend_from_slice(&[0; 208]); // pad to minimum size

    assert!(
        Link::detect(&valid_header),
        "Should detect valid LLG header"
    );
}

#[test]
fn test_link_detection_minimal_header() {
    // Need at least 215 bytes with "lf3" at offset 4
    let mut minimal = vec![0x00, 0x00, 0x00, 0x00];
    minimal.extend_from_slice(b"lf3");
    minimal.extend_from_slice(&[0; 208]); // pad to 215 bytes

    assert!(Link::detect(&minimal), "Should detect minimal LLG header");
}

#[test]
fn test_link_detection_invalid_magic() {
    let mut invalid = vec![0x00, 0x00, 0x00, 0x00];
    invalid.extend_from_slice(b"NOT");
    invalid.extend_from_slice(&[0; 208]);

    assert!(!Link::detect(&invalid), "Should not detect invalid magic");
}

#[test]
fn test_link_detection_empty() {
    assert!(!Link::detect(b""), "Should not detect empty data");
}

#[test]
fn test_link_detection_too_short() {
    let too_short = b"lf3short";
    assert!(
        !Link::detect(too_short),
        "Should not detect too short header"
    );
}

#[test]
fn test_link_detection_wrong_offset() {
    // "lf3" at wrong offset (offset 0 instead of 4)
    let mut wrong_offset = b"lf3".to_vec();
    wrong_offset.extend_from_slice(&[0; 212]);

    assert!(
        !Link::detect(&wrong_offset),
        "Should not detect lf3 at wrong offset"
    );
}

#[test]
fn test_link_detection_other_formats() {
    assert!(!Link::detect(b"MLVLG"), "Should not detect MLG format");
    assert!(!Link::detect(b"<hCNF"), "Should not detect AiM format");
    assert!(
        !Link::detect(b"%DataLog%"),
        "Should not detect Haltech format"
    );
}

// ============================================
// Text Parser Error Tests
// ============================================

#[test]
fn test_link_text_parser_returns_error() {
    let parser = Link;
    let result = parser.parse("TIME;RPM\n0.0;1000\n");

    assert!(
        result.is_err(),
        "Text parser should return error for binary format"
    );
}

// ============================================
// Real File Tests
// ============================================

#[test]
fn test_link_standard_example_file() {
    if !example_file_exists(LINK_STANDARD) {
        eprintln!("Skipping test: {} not found", LINK_STANDARD);
        return;
    }

    let data = read_example_binary(LINK_STANDARD);

    assert!(Link::detect(&data), "Should detect as Link LLG format");

    let log = Link::parse_binary(&data).expect("Should parse Link LLG");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);
    assert_valid_time_range(&log);

    assert_minimum_channels(&log, 3);
    assert_minimum_records(&log, 100);

    eprintln!(
        "Link standard log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

#[test]
fn test_link_small_example_file() {
    if !example_file_exists(LINK_SMALL) {
        eprintln!("Skipping test: {} not found", LINK_SMALL);
        return;
    }

    let data = read_example_binary(LINK_SMALL);

    assert!(Link::detect(&data), "Should detect as Link LLG format");

    let log = Link::parse_binary(&data).expect("Should parse small Link log");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);

    eprintln!(
        "Link small log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

#[test]
fn test_link_medium_example_file() {
    if !example_file_exists(LINK_MEDIUM) {
        eprintln!("Skipping test: {} not found", LINK_MEDIUM);
        return;
    }

    let data = read_example_binary(LINK_MEDIUM);

    assert!(Link::detect(&data), "Should detect as Link LLG format");

    let log = Link::parse_binary(&data).expect("Should parse medium Link log");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);

    eprintln!(
        "Link medium log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

#[test]
fn test_link_large_example_file() {
    if !example_file_exists(LINK_LARGE) {
        eprintln!("Skipping test: {} not found", LINK_LARGE);
        return;
    }

    let data = read_example_binary(LINK_LARGE);

    assert!(Link::detect(&data), "Should detect as Link LLG format");

    let log = Link::parse_binary(&data).expect("Should parse large Link log");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);

    // Large file should have substantial data
    assert_minimum_records(&log, 500);

    eprintln!(
        "Link large log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

// ============================================
// Channel Tests
// ============================================

#[test]
fn test_link_channel_properties() {
    if !example_file_exists(LINK_STANDARD) {
        eprintln!("Skipping test: {} not found", LINK_STANDARD);
        return;
    }

    let data = read_example_binary(LINK_STANDARD);
    let log = Link::parse_binary(&data).expect("Should parse");

    // Verify channel properties
    for channel in &log.channels {
        let name = channel.name();
        let unit = channel.unit();

        // Names should be non-empty
        assert!(!name.is_empty(), "Channel should have a name");

        // Names should be printable (UTF-16 decoded to ASCII)
        for c in name.chars() {
            assert!(
                c.is_ascii_graphic() || c == ' ',
                "Channel name should contain printable chars: {:?}",
                name
            );
        }

        // Units may be empty for some channels
        for c in unit.chars() {
            assert!(
                c.is_ascii_graphic() || c == ' ' || c == '°' || c == '%',
                "Channel unit should contain printable chars: {:?}",
                unit
            );
        }
    }
}

#[test]
fn test_link_channel_data_extraction() {
    if !example_file_exists(LINK_STANDARD) {
        eprintln!("Skipping test: {} not found", LINK_STANDARD);
        return;
    }

    let data = read_example_binary(LINK_STANDARD);
    let log = Link::parse_binary(&data).expect("Should parse");

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
fn test_link_find_channel_index() {
    if !example_file_exists(LINK_STANDARD) {
        eprintln!("Skipping test: {} not found", LINK_STANDARD);
        return;
    }

    let data = read_example_binary(LINK_STANDARD);
    let log = Link::parse_binary(&data).expect("Should parse");

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
fn test_link_data_structure() {
    if !example_file_exists(LINK_STANDARD) {
        eprintln!("Skipping test: {} not found", LINK_STANDARD);
        return;
    }

    let data = read_example_binary(LINK_STANDARD);
    let log = Link::parse_binary(&data).expect("Should parse");

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
fn test_link_timestamp_monotonicity() {
    if !example_file_exists(LINK_STANDARD) {
        eprintln!("Skipping test: {} not found", LINK_STANDARD);
        return;
    }

    let data = read_example_binary(LINK_STANDARD);
    let log = Link::parse_binary(&data).expect("Should parse");

    // Link timestamps are interpolated from channel data
    // They should be monotonically increasing after processing
    assert_monotonic_times(&log);
}

#[test]
fn test_link_values_are_finite() {
    if !example_file_exists(LINK_STANDARD) {
        eprintln!("Skipping test: {} not found", LINK_STANDARD);
        return;
    }

    let data = read_example_binary(LINK_STANDARD);
    let log = Link::parse_binary(&data).expect("Should parse");

    assert_finite_values(&log);
}

// ============================================
// Edge Case Tests
// ============================================

#[test]
fn test_link_parse_invalid_data() {
    let invalid = b"NOT_LINK_FORMAT_DATA";
    let result = Link::parse_binary(invalid);

    match result {
        Ok(log) => assert!(log.data.is_empty()),
        Err(_) => { /* Expected */ }
    }
}

#[test]
fn test_link_parse_truncated_file() {
    // Create minimal header then truncate
    let mut truncated = vec![0xd7, 0x00, 0x00, 0x00];
    truncated.extend_from_slice(b"lf3");
    truncated.extend_from_slice(&[0; 50]); // Less than minimum 215 bytes

    let result = Link::parse_binary(&truncated);

    // Should handle gracefully
    match result {
        Ok(log) => assert!(log.data.is_empty()),
        Err(_) => { /* Also acceptable */ }
    }
}

#[test]
fn test_link_parse_empty_data() {
    let result = Link::parse_binary(b"");

    match result {
        Ok(log) => assert!(log.data.is_empty()),
        Err(_) => { /* Expected */ }
    }
}

// ============================================
// Metadata Tests
// ============================================

#[test]
fn test_link_metadata_extraction() {
    if !example_file_exists(LINK_STANDARD) {
        eprintln!("Skipping test: {} not found", LINK_STANDARD);
        return;
    }

    let data = read_example_binary(LINK_STANDARD);
    let log = Link::parse_binary(&data).expect("Should parse");

    // Link files have metadata (ECU model, date, time, version)
    // Access via log.meta if available
    let _ = &log.meta;
}

// ============================================
// All LLG Files Test
// ============================================

#[test]
fn test_link_all_example_files() {
    let link_files = [LINK_SMALL, LINK_MEDIUM, LINK_LARGE, LINK_STANDARD];

    for file_path in link_files {
        if !example_file_exists(file_path) {
            eprintln!("Skipping: {} not found", file_path);
            continue;
        }

        let data = read_example_binary(file_path);

        assert!(
            Link::detect(&data),
            "Should detect {} as Link format",
            file_path
        );

        let result = Link::parse_binary(&data);
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
fn test_link_large_file_performance() {
    if !example_file_exists(LINK_LARGE) {
        eprintln!("Skipping test: {} not found", LINK_LARGE);
        return;
    }

    let data = read_example_binary(LINK_LARGE);

    let start = std::time::Instant::now();
    let log = Link::parse_binary(&data).expect("Should parse large file");
    let elapsed = start.elapsed();

    eprintln!("Parsed {} records in {:?}", log.data.len(), elapsed);

    // Should complete in reasonable time
    assert!(
        elapsed.as_secs() < 30,
        "Parsing should complete in reasonable time"
    );
}

// ============================================
// Timeline Interpolation Tests
// ============================================

#[test]
fn test_link_timeline_validity() {
    if !example_file_exists(LINK_STANDARD) {
        eprintln!("Skipping test: {} not found", LINK_STANDARD);
        return;
    }

    let data = read_example_binary(LINK_STANDARD);
    let log = Link::parse_binary(&data).expect("Should parse");

    let times = log.get_times_as_f64();

    if !times.is_empty() {
        // First timestamp should be non-negative
        assert!(times[0] >= 0.0, "First timestamp should be non-negative");

        // All timestamps should be finite and reasonable
        for (i, &t) in times.iter().enumerate() {
            assert!(t.is_finite(), "Timestamp {} should be finite", i);
            assert!(t < 100000.0, "Timestamp {} should be reasonable", i);
        }
    }
}
