//! File loading integration tests
//!
//! Tests for end-to-end file loading cycles across all supported formats.

#[path = "../common/mod.rs"]
mod common;

use common::assertions::*;
use common::example_files::*;
use common::{example_file_exists, get_example_file_path, read_example_binary, read_example_file};
use std::path::Path;
use ultralog::parsers::aim::Aim;
use ultralog::parsers::ecumaster::EcuMaster;
use ultralog::parsers::haltech::Haltech;
use ultralog::parsers::link::Link;
use ultralog::parsers::romraider::RomRaider;
use ultralog::parsers::speeduino::Speeduino;
use ultralog::parsers::types::Parseable;

// ============================================
// Text Format Loading Tests
// ============================================

#[test]
fn test_load_haltech_complete_cycle() {
    if !example_file_exists(HALTECH_SMALL) {
        eprintln!("Skipping: {} not found", HALTECH_SMALL);
        return;
    }

    let content = read_example_file(HALTECH_SMALL);

    // Detect format
    assert!(content.starts_with("%DataLog%"), "Should detect Haltech");

    // Parse
    let parser = Haltech;
    let log = parser.parse(&content).expect("Should parse");

    // Validate
    assert_valid_log_structure(&log);
    assert_monotonic_times(&log);
    assert_finite_values(&log);

    // Access data
    for idx in 0..log.channels.len() {
        let data = log.get_channel_data(idx);
        assert_eq!(data.len(), log.data.len());
    }
}

#[test]
fn test_load_ecumaster_complete_cycle() {
    if !example_file_exists(ECUMASTER_STANDARD) {
        eprintln!("Skipping: {} not found", ECUMASTER_STANDARD);
        return;
    }

    let content = read_example_file(ECUMASTER_STANDARD);

    // Detect format
    assert!(EcuMaster::detect(&content), "Should detect ECUMaster");

    // Parse
    let parser = EcuMaster;
    let log = parser.parse(&content).expect("Should parse");

    // Validate
    assert_valid_log_structure(&log);
    assert_finite_values(&log);

    // Access data
    for idx in 0..log.channels.len() {
        let data = log.get_channel_data(idx);
        assert_eq!(data.len(), log.data.len());
    }
}

#[test]
fn test_load_romraider_synthetic() {
    let content = "Time (msec),Engine Speed (rpm),Engine Load (%)\n\
                   0,1000,20\n\
                   100,1100,25\n\
                   200,1200,30\n";

    // Detect format
    assert!(RomRaider::detect(content), "Should detect RomRaider");

    // Parse
    let parser = RomRaider;
    let log = parser.parse(content).expect("Should parse");

    // Validate
    assert_valid_log_structure(&log);
    assert_monotonic_times(&log);
    assert_finite_values(&log);
}

// ============================================
// Binary Format Loading Tests
// ============================================

#[test]
fn test_load_speeduino_complete_cycle() {
    if !example_file_exists(SPEEDUINO_MLG) {
        eprintln!("Skipping: {} not found", SPEEDUINO_MLG);
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);

    // Detect format
    assert!(Speeduino::detect(&data), "Should detect Speeduino MLG");

    // Parse
    let log = Speeduino::parse_binary(&data).expect("Should parse");

    // Validate
    assert_valid_log_structure(&log);
    assert_finite_values(&log);

    // Access data
    for idx in 0..log.channels.len() {
        let channel_data = log.get_channel_data(idx);
        assert_eq!(channel_data.len(), log.data.len());
    }
}

#[test]
fn test_load_aim_complete_cycle() {
    if !example_file_exists(AIM_GENERIC) {
        eprintln!("Skipping: {} not found", AIM_GENERIC);
        return;
    }

    let data = read_example_binary(AIM_GENERIC);

    // Detect format
    assert!(Aim::detect(&data), "Should detect AiM XRK");

    // Parse
    let path = get_example_file_path(AIM_GENERIC);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse");

    // Validate
    assert_valid_log_structure(&log);
    assert_finite_values(&log);
}

#[test]
fn test_load_link_complete_cycle() {
    if !example_file_exists(LINK_STANDARD) {
        eprintln!("Skipping: {} not found", LINK_STANDARD);
        return;
    }

    let data = read_example_binary(LINK_STANDARD);

    // Detect format
    assert!(Link::detect(&data), "Should detect Link LLG");

    // Parse
    let log = Link::parse_binary(&data).expect("Should parse");

    // Validate
    assert_valid_log_structure(&log);
    assert_finite_values(&log);
}

// ============================================
// Error Handling Tests
// ============================================

#[test]
fn test_load_empty_text_file() {
    let parser = Haltech;
    let result = parser.parse("");

    // Should return error or empty log
    match result {
        Ok(log) => assert!(log.data.is_empty()),
        Err(_) => { /* Expected */ }
    }
}

#[test]
fn test_load_empty_binary_file() {
    let result = Speeduino::parse_binary(b"");

    match result {
        Ok(log) => assert!(log.data.is_empty()),
        Err(_) => { /* Expected */ }
    }
}

#[test]
fn test_load_corrupted_text() {
    let corrupted = "\x00\x01\x02\x03\x04\x05";

    let haltech = Haltech;
    let ecumaster = EcuMaster;
    let romraider = RomRaider;

    // Should handle gracefully (not panic)
    let _ = haltech.parse(corrupted);
    let _ = ecumaster.parse(corrupted);
    let _ = romraider.parse(corrupted);
}

#[test]
fn test_load_corrupted_binary() {
    let corrupted = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09";

    // Should handle gracefully (not panic)
    // Note: Aim uses parse_file which takes a path, so we can't test corrupted bytes directly
    let _ = Speeduino::parse_binary(corrupted);
    let _ = Link::parse_binary(corrupted);
}

// ============================================
// Large File Tests
// ============================================

#[test]
fn test_load_large_haltech_file() {
    if !example_file_exists(HALTECH_LARGE) {
        eprintln!("Skipping: {} not found", HALTECH_LARGE);
        return;
    }

    let start = std::time::Instant::now();
    let content = read_example_file(HALTECH_LARGE);
    let read_time = start.elapsed();

    let start = std::time::Instant::now();
    let parser = Haltech;
    let log = parser.parse(&content).expect("Should parse large file");
    let parse_time = start.elapsed();

    assert_valid_log_structure(&log);
    assert_minimum_records(&log, 1000);

    eprintln!(
        "Large Haltech: {} records, read: {:?}, parse: {:?}",
        log.data.len(),
        read_time,
        parse_time
    );
}

#[test]
fn test_load_large_ecumaster_file() {
    if !example_file_exists(ECUMASTER_LARGE) {
        eprintln!("Skipping: {} not found", ECUMASTER_LARGE);
        return;
    }

    let start = std::time::Instant::now();
    let content = read_example_file(ECUMASTER_LARGE);
    let read_time = start.elapsed();

    let start = std::time::Instant::now();
    let parser = EcuMaster;
    let log = parser.parse(&content).expect("Should parse large file");
    let parse_time = start.elapsed();

    assert_valid_log_structure(&log);
    assert_minimum_records(&log, 1000);

    eprintln!(
        "Large ECUMaster: {} records, read: {:?}, parse: {:?}",
        log.data.len(),
        read_time,
        parse_time
    );
}

// ============================================
// Channel Information Tests
// ============================================

#[test]
fn test_channel_properties_after_load() {
    if !example_file_exists(HALTECH_SMALL) {
        eprintln!("Skipping: {} not found", HALTECH_SMALL);
        return;
    }

    let content = read_example_file(HALTECH_SMALL);
    let parser = Haltech;
    let log = parser.parse(&content).expect("Should parse");

    for channel in &log.channels {
        // Every channel should have a name
        assert!(!channel.name().is_empty(), "Channel should have name");

        // IDs should be present (String type)
        assert!(!channel.id().is_empty(), "Channel ID should be non-empty");

        // Type name should be non-empty
        assert!(
            !channel.type_name().is_empty(),
            "Channel type name should exist"
        );
    }
}

#[test]
fn test_find_channel_after_load() {
    if !example_file_exists(HALTECH_SMALL) {
        eprintln!("Skipping: {} not found", HALTECH_SMALL);
        return;
    }

    let content = read_example_file(HALTECH_SMALL);
    let parser = Haltech;
    let log = parser.parse(&content).expect("Should parse");

    // Should find first channel
    if !log.channels.is_empty() {
        let first_name = log.channels[0].name();
        let idx = log.find_channel_index(&first_name);
        assert_eq!(idx, Some(0));
    }

    // Should not find non-existent channel
    let not_found = log.find_channel_index("NonExistentChannel12345");
    assert_eq!(not_found, None);
}
