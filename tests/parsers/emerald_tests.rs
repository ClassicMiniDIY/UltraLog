//! Comprehensive tests for the Emerald ECU LG1/LG2 binary parser
//!
//! Tests cover:
//! - Binary format detection (24-byte records with OLE timestamps)
//! - LG2 channel definition parsing
//! - LG1 binary data parsing
//! - Channel ID mapping
//! - Real file parsing with example logs
//! - Edge cases and error handling

#[path = "../common/mod.rs"]
mod common;

use common::assertions::*;
use common::example_files::*;
use common::{example_file_exists, read_example_binary};
use std::path::Path;
use ultralog::parsers::emerald::Emerald;
use ultralog::parsers::types::Meta;

// ============================================
// Format Detection Tests
// ============================================

#[test]
fn test_emerald_detection_valid_lg1() {
    // Create minimal valid LG1 data (one record = 24 bytes)
    // OLE timestamp (f64) + 8 x u16 values
    let mut valid_data = Vec::with_capacity(24);

    // Valid OLE timestamp (e.g., 46022.5 = Dec 2025)
    let timestamp: f64 = 46022.5;
    valid_data.extend_from_slice(&timestamp.to_le_bytes());

    // 8 channel values (16 bytes)
    for i in 0..8 {
        let value: u16 = (i * 100) as u16;
        valid_data.extend_from_slice(&value.to_le_bytes());
    }

    assert_eq!(valid_data.len(), 24);
    assert!(
        Emerald::detect(&valid_data),
        "Should detect valid LG1 data with correct timestamp"
    );
}

#[test]
fn test_emerald_detection_multiple_records() {
    // Create 3 records of valid data
    let mut data = Vec::with_capacity(72);

    for record_idx in 0..3 {
        let timestamp: f64 = 46022.5 + (record_idx as f64 * 0.001); // ~1 minute apart
        data.extend_from_slice(&timestamp.to_le_bytes());

        for i in 0..8 {
            let value: u16 = ((record_idx * 8 + i) * 10) as u16;
            data.extend_from_slice(&value.to_le_bytes());
        }
    }

    assert_eq!(data.len(), 72);
    assert!(
        Emerald::detect(&data),
        "Should detect valid LG1 data with multiple records"
    );
}

#[test]
fn test_emerald_detection_empty() {
    assert!(!Emerald::detect(b""), "Should not detect empty data");
}

#[test]
fn test_emerald_detection_too_short() {
    let too_short = vec![0u8; 23];
    assert!(
        !Emerald::detect(&too_short),
        "Should not detect data shorter than 24 bytes"
    );
}

#[test]
fn test_emerald_detection_wrong_size() {
    // Not a multiple of 24 bytes
    let wrong_size = vec![0u8; 25];
    assert!(
        !Emerald::detect(&wrong_size),
        "Should not detect data with size not multiple of 24"
    );
}

#[test]
fn test_emerald_detection_invalid_timestamp_too_old() {
    let mut data = vec![0u8; 24];
    let old_timestamp: f64 = 1000.0; // Way too old (around year 1902)
    data[0..8].copy_from_slice(&old_timestamp.to_le_bytes());

    assert!(
        !Emerald::detect(&data),
        "Should not detect data with timestamp before 1995"
    );
}

#[test]
fn test_emerald_detection_invalid_timestamp_too_new() {
    let mut data = vec![0u8; 24];
    let future_timestamp: f64 = 100000.0; // Way too far in future (around year 2173)
    data[0..8].copy_from_slice(&future_timestamp.to_le_bytes());

    assert!(
        !Emerald::detect(&data),
        "Should not detect data with timestamp after 2050"
    );
}

#[test]
fn test_emerald_detection_divergent_timestamps() {
    // Two records with timestamps more than 1 day apart
    let mut data = vec![0u8; 48];

    let timestamp1: f64 = 46022.5;
    data[0..8].copy_from_slice(&timestamp1.to_le_bytes());

    let timestamp2: f64 = 46025.5; // 3 days later - suspicious
    data[24..32].copy_from_slice(&timestamp2.to_le_bytes());

    assert!(
        !Emerald::detect(&data),
        "Should not detect data with timestamps >1 day apart"
    );
}

#[test]
fn test_emerald_detection_other_formats() {
    assert!(
        !Emerald::detect(b"MLVLG"),
        "Should not detect Speeduino MLG format"
    );
    assert!(
        !Emerald::detect(b"<hCNF"),
        "Should not detect AiM XRK format"
    );
    assert!(
        !Emerald::detect(b"%DataLog%"),
        "Should not detect Haltech format"
    );

    // Create valid LLG header (Link format)
    let mut llg_header = vec![0xd7, 0x00, 0x00, 0x00];
    llg_header.extend_from_slice(b"lf3");
    llg_header.extend_from_slice(&[0; 17]); // Pad to 24 bytes
    assert!(
        !Emerald::detect(&llg_header),
        "Should not detect Link LLG format"
    );
}

// ============================================
// Path Detection Tests
// ============================================

#[test]
fn test_emerald_path_detection_lg1() {
    assert!(
        Emerald::is_emerald_path(Path::new("test.lg1")),
        "Should recognize .lg1 extension"
    );
    assert!(
        Emerald::is_emerald_path(Path::new("test.LG1")),
        "Should recognize .LG1 extension (uppercase)"
    );
    assert!(
        Emerald::is_emerald_path(Path::new("/path/to/file.lg1")),
        "Should recognize .lg1 with path"
    );
}

#[test]
fn test_emerald_path_detection_lg2() {
    assert!(
        Emerald::is_emerald_path(Path::new("test.lg2")),
        "Should recognize .lg2 extension"
    );
    assert!(
        Emerald::is_emerald_path(Path::new("test.LG2")),
        "Should recognize .LG2 extension (uppercase)"
    );
    assert!(
        Emerald::is_emerald_path(Path::new("/path/to/file.lg2")),
        "Should recognize .lg2 with path"
    );
}

#[test]
fn test_emerald_path_detection_negative() {
    assert!(
        !Emerald::is_emerald_path(Path::new("test.csv")),
        "Should not recognize .csv"
    );
    assert!(
        !Emerald::is_emerald_path(Path::new("test.llg")),
        "Should not recognize .llg (Link format)"
    );
    assert!(
        !Emerald::is_emerald_path(Path::new("test.mlg")),
        "Should not recognize .mlg (Speeduino format)"
    );
    assert!(
        !Emerald::is_emerald_path(Path::new("test")),
        "Should not recognize file without extension"
    );
    assert!(
        !Emerald::is_emerald_path(Path::new("lg1.txt")),
        "Should not recognize lg1 as part of filename"
    );
}

// ============================================
// LG2 Text Detection Tests
// ============================================

#[test]
fn test_emerald_lg2_detection_valid() {
    let lg2_content =
        "[chan1]\n19\n[chan2]\n46\n[chan3]\n2\n[chan4]\n20\n[chan5]\n1\n[chan6]\n31\n[chan7]\n32\n[chan8]\n17\n";

    assert!(
        Emerald::detect_lg2(lg2_content.as_bytes()),
        "Should detect valid LG2 format"
    );
}

#[test]
fn test_emerald_lg2_detection_with_extra_sections() {
    let lg2_content = "[chan1]\n19\n[chan2]\n46\n[chan3]\n2\n[chan4]\n20\n[chan5]\n1\n[chan6]\n31\n[chan7]\n32\n[chan8]\n17\n[ValU]\n0\n2\n0\n0\n0\n";

    assert!(
        Emerald::detect_lg2(lg2_content.as_bytes()),
        "Should detect LG2 format with ValU section"
    );
}

#[test]
fn test_emerald_lg2_detection_empty() {
    assert!(
        !Emerald::detect_lg2(b""),
        "Should not detect empty data as LG2"
    );
}

#[test]
fn test_emerald_lg2_detection_binary_data() {
    // Binary data should not be detected as LG2
    let mut binary = vec![0u8; 24];
    let timestamp: f64 = 46022.5;
    binary[0..8].copy_from_slice(&timestamp.to_le_bytes());

    assert!(
        !Emerald::detect_lg2(&binary),
        "Should not detect binary LG1 data as LG2"
    );
}

#[test]
fn test_emerald_lg2_detection_other_text_formats() {
    // CSV should not be detected as LG2
    assert!(
        !Emerald::detect_lg2(b"TIME,RPM,TPS\n0.0,1000,50\n"),
        "Should not detect CSV as LG2"
    );

    // INI-like but not Emerald
    assert!(
        !Emerald::detect_lg2(b"[section1]\nkey=value\n"),
        "Should not detect generic INI as LG2"
    );
}

#[test]
fn test_emerald_lg2_detection_partial_channels() {
    // Only 3 channels - should be rejected (need at least 4)
    let partial = "[chan1]\n19\n[chan2]\n46\n[chan3]\n2\n";

    assert!(
        !Emerald::detect_lg2(partial.as_bytes()),
        "Should not detect LG2 with fewer than 4 channels"
    );
}

#[test]
fn test_emerald_lg2_detection_minimum_channels() {
    // Exactly 4 channels - should pass
    let minimal = "[chan1]\n19\n[chan2]\n46\n[chan3]\n2\n[chan4]\n20\n";

    assert!(
        Emerald::detect_lg2(minimal.as_bytes()),
        "Should detect LG2 with exactly 4 channels"
    );
}

#[test]
fn test_emerald_parse_via_lg2_file() {
    // Test that parse_file works when given an LG2 path
    let lg2_path = "exampleLogs/emerald/EM Log MG ZS Turbo idle and rev.lg2";

    if !Path::new(lg2_path).exists() {
        eprintln!("Skipping test: {} not found", lg2_path);
        return;
    }

    // Read the LG2 file to verify detection
    let lg2_data = std::fs::read(lg2_path).expect("Should read LG2 file");
    assert!(
        Emerald::detect_lg2(&lg2_data),
        "Should detect LG2 file content"
    );

    // Parse should work when given LG2 path (it finds the matching LG1)
    let log = Emerald::parse_file(Path::new(lg2_path)).expect("Should parse from LG2 path");

    assert_eq!(log.channels.len(), 8, "Should have 8 channels");
    assert!(!log.data.is_empty(), "Should have data records");

    eprintln!(
        "Parsed via LG2: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

// ============================================
// Real File Tests
// ============================================

#[test]
fn test_emerald_idle_and_rev_file() {
    if !example_file_exists(EMERALD_IDLE_REV) {
        eprintln!("Skipping test: {} not found", EMERALD_IDLE_REV);
        return;
    }

    let data = read_example_binary(EMERALD_IDLE_REV);

    assert!(
        Emerald::detect(&data),
        "Should detect as Emerald LG1 format"
    );

    let log = Emerald::parse_file(Path::new(EMERALD_IDLE_REV)).expect("Should parse Emerald log");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);
    assert_valid_time_range(&log);
    assert_monotonic_times(&log);

    // Emerald logs always have 8 channels
    assert_eq!(log.channels.len(), 8, "Should have exactly 8 channels");

    // This file should have substantial data
    assert_minimum_records(&log, 2000);

    eprintln!(
        "Emerald idle/rev log: {} channels, {} records, {:.1}s duration",
        log.channels.len(),
        log.data.len(),
        log.times.last().unwrap_or(&0.0)
    );
}

#[test]
fn test_emerald_short_drive_file() {
    if !example_file_exists(EMERALD_SHORT_DRIVE) {
        eprintln!("Skipping test: {} not found", EMERALD_SHORT_DRIVE);
        return;
    }

    let data = read_example_binary(EMERALD_SHORT_DRIVE);

    assert!(
        Emerald::detect(&data),
        "Should detect as Emerald LG1 format"
    );

    let log =
        Emerald::parse_file(Path::new(EMERALD_SHORT_DRIVE)).expect("Should parse Emerald log");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);
    assert_valid_time_range(&log);
    assert_monotonic_times(&log);

    assert_eq!(log.channels.len(), 8, "Should have exactly 8 channels");
    assert_minimum_records(&log, 5000);

    eprintln!(
        "Emerald short drive log: {} channels, {} records, {:.1}s duration",
        log.channels.len(),
        log.data.len(),
        log.times.last().unwrap_or(&0.0)
    );
}

#[test]
fn test_emerald_diff_channels_file() {
    if !example_file_exists(EMERALD_DIFF_CHANNELS) {
        eprintln!("Skipping test: {} not found", EMERALD_DIFF_CHANNELS);
        return;
    }

    let data = read_example_binary(EMERALD_DIFF_CHANNELS);

    assert!(
        Emerald::detect(&data),
        "Should detect as Emerald LG1 format"
    );

    let log =
        Emerald::parse_file(Path::new(EMERALD_DIFF_CHANNELS)).expect("Should parse Emerald log");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);
    assert_valid_time_range(&log);
    assert_monotonic_times(&log);

    assert_eq!(log.channels.len(), 8, "Should have exactly 8 channels");
    assert_minimum_records(&log, 3000);

    // This file has different channel configuration
    // Verify we got different channel names than the other files
    let channel_names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    eprintln!("Diff channels file has: {:?}", channel_names);

    eprintln!(
        "Emerald diff channels log: {} channels, {} records, {:.1}s duration",
        log.channels.len(),
        log.data.len(),
        log.times.last().unwrap_or(&0.0)
    );
}

// ============================================
// Channel Tests
// ============================================

#[test]
fn test_emerald_channel_properties() {
    if !example_file_exists(EMERALD_IDLE_REV) {
        eprintln!("Skipping test: {} not found", EMERALD_IDLE_REV);
        return;
    }

    let log = Emerald::parse_file(Path::new(EMERALD_IDLE_REV)).expect("Should parse");

    for channel in &log.channels {
        let name = channel.name();
        let unit = channel.unit();

        // Names should be non-empty
        assert!(!name.is_empty(), "Channel should have a name");

        // Names should be printable
        for c in name.chars() {
            assert!(
                c.is_ascii_graphic() || c == ' ' || c == '°',
                "Channel name should contain printable chars: {:?}",
                name
            );
        }

        // Units should be valid
        for c in unit.chars() {
            assert!(
                c.is_ascii_graphic() || c == ' ' || c == '°' || c == '%' || c == 'λ',
                "Channel unit should contain valid chars: {:?}",
                unit
            );
        }

        eprintln!("  Channel: {} [{}]", name, unit);
    }
}

#[test]
fn test_emerald_expected_channels() {
    if !example_file_exists(EMERALD_IDLE_REV) {
        eprintln!("Skipping test: {} not found", EMERALD_IDLE_REV);
        return;
    }

    let log = Emerald::parse_file(Path::new(EMERALD_IDLE_REV)).expect("Should parse");

    let channel_names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();

    // These files typically have standard channels
    let has_rpm = channel_names.iter().any(|n| n.contains("RPM"));
    let has_temp = channel_names
        .iter()
        .any(|n| n.contains("Temp") || n.contains("temp"));

    assert!(has_rpm, "Should have an RPM channel: {:?}", channel_names);
    assert!(
        has_temp,
        "Should have a temperature channel: {:?}",
        channel_names
    );
}

#[test]
fn test_emerald_channel_data_extraction() {
    if !example_file_exists(EMERALD_IDLE_REV) {
        eprintln!("Skipping test: {} not found", EMERALD_IDLE_REV);
        return;
    }

    let log = Emerald::parse_file(Path::new(EMERALD_IDLE_REV)).expect("Should parse");

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
fn test_emerald_find_channel_index() {
    if !example_file_exists(EMERALD_IDLE_REV) {
        eprintln!("Skipping test: {} not found", EMERALD_IDLE_REV);
        return;
    }

    let log = Emerald::parse_file(Path::new(EMERALD_IDLE_REV)).expect("Should parse");

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
// Data Value Tests
// ============================================

#[test]
fn test_emerald_data_values_reasonable() {
    if !example_file_exists(EMERALD_IDLE_REV) {
        eprintln!("Skipping test: {} not found", EMERALD_IDLE_REV);
        return;
    }

    let log = Emerald::parse_file(Path::new(EMERALD_IDLE_REV)).expect("Should parse");

    // Find RPM channel and check values are reasonable
    for (idx, channel) in log.channels.iter().enumerate() {
        if channel.name().contains("RPM") {
            let rpm_data = log.get_channel_data(idx);
            let min = rpm_data.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = rpm_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            eprintln!("RPM range: {} to {}", min, max);

            // RPM should be in reasonable range (0-15000)
            assert!(min >= 0.0, "RPM min should be >= 0");
            assert!(max <= 15000.0, "RPM max should be <= 15000");
            break;
        }
    }

    // Find Coolant Temp and check values are reasonable
    for (idx, channel) in log.channels.iter().enumerate() {
        if channel.name().contains("Coolant") {
            let temp_data = log.get_channel_data(idx);
            let min = temp_data.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = temp_data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            eprintln!("Coolant temp range: {} to {} °C", min, max);

            // Temperature should be in reasonable range (-40°C to 200°C)
            assert!(min >= -40.0, "Coolant temp min should be >= -40");
            assert!(max <= 200.0, "Coolant temp max should be <= 200");
            break;
        }
    }
}

// ============================================
// Metadata Tests
// ============================================

#[test]
fn test_emerald_metadata_extraction() {
    if !example_file_exists(EMERALD_IDLE_REV) {
        eprintln!("Skipping test: {} not found", EMERALD_IDLE_REV);
        return;
    }

    let log = Emerald::parse_file(Path::new(EMERALD_IDLE_REV)).expect("Should parse");

    // Check metadata
    if let Meta::Emerald(meta) = &log.meta {
        eprintln!("Source file: {}", meta.source_file);
        eprintln!("Record count: {}", meta.record_count);
        eprintln!("Duration: {:.1}s", meta.duration_seconds);
        eprintln!("Sample rate: {:.1} Hz", meta.sample_rate_hz);

        assert!(
            meta.record_count > 0,
            "Should have positive record count: {}",
            meta.record_count
        );
        assert!(
            meta.duration_seconds > 0.0,
            "Should have positive duration: {}",
            meta.duration_seconds
        );
        assert!(
            meta.sample_rate_hz > 0.0,
            "Should have positive sample rate: {}",
            meta.sample_rate_hz
        );

        // Sample rate should be reasonable (10-100 Hz typical)
        assert!(
            meta.sample_rate_hz >= 5.0 && meta.sample_rate_hz <= 200.0,
            "Sample rate should be reasonable: {} Hz",
            meta.sample_rate_hz
        );
    } else {
        panic!("Expected Emerald metadata variant");
    }
}

// ============================================
// Data Integrity Tests
// ============================================

#[test]
fn test_emerald_data_structure() {
    if !example_file_exists(EMERALD_IDLE_REV) {
        eprintln!("Skipping test: {} not found", EMERALD_IDLE_REV);
        return;
    }

    let log = Emerald::parse_file(Path::new(EMERALD_IDLE_REV)).expect("Should parse");

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
fn test_emerald_timestamp_monotonicity() {
    if !example_file_exists(EMERALD_IDLE_REV) {
        eprintln!("Skipping test: {} not found", EMERALD_IDLE_REV);
        return;
    }

    let log = Emerald::parse_file(Path::new(EMERALD_IDLE_REV)).expect("Should parse");
    assert_monotonic_times(&log);
}

#[test]
fn test_emerald_values_are_finite() {
    if !example_file_exists(EMERALD_IDLE_REV) {
        eprintln!("Skipping test: {} not found", EMERALD_IDLE_REV);
        return;
    }

    let log = Emerald::parse_file(Path::new(EMERALD_IDLE_REV)).expect("Should parse");
    assert_finite_values(&log);
}

// ============================================
// Timeline Tests
// ============================================

#[test]
fn test_emerald_timeline_validity() {
    if !example_file_exists(EMERALD_IDLE_REV) {
        eprintln!("Skipping test: {} not found", EMERALD_IDLE_REV);
        return;
    }

    let log = Emerald::parse_file(Path::new(EMERALD_IDLE_REV)).expect("Should parse");

    let times = log.get_times_as_f64();

    if !times.is_empty() {
        // First timestamp should be 0 (relative to start)
        assert!(
            times[0] == 0.0,
            "First timestamp should be 0, got {}",
            times[0]
        );

        // All timestamps should be finite and reasonable
        for (i, &t) in times.iter().enumerate() {
            assert!(t.is_finite(), "Timestamp {} should be finite", i);
            assert!(
                t < 10000.0,
                "Timestamp {} should be reasonable (<10000s)",
                i
            );
        }

        // Last timestamp indicates duration
        let duration = times.last().unwrap();
        eprintln!("Log duration: {:.1} seconds", duration);
    }
}

// ============================================
// All Files Test
// ============================================

#[test]
fn test_emerald_all_example_files() {
    let emerald_files = [EMERALD_IDLE_REV, EMERALD_SHORT_DRIVE, EMERALD_DIFF_CHANNELS];

    for file_path in emerald_files {
        if !example_file_exists(file_path) {
            eprintln!("Skipping: {} not found", file_path);
            continue;
        }

        let data = read_example_binary(file_path);

        assert!(
            Emerald::detect(&data),
            "Should detect {} as Emerald format",
            file_path
        );

        let result = Emerald::parse_file(Path::new(file_path));
        assert!(result.is_ok(), "Should parse {} without error", file_path);

        let log = result.unwrap();

        assert_eq!(
            log.channels.len(),
            8,
            "{} should have 8 channels",
            file_path
        );
        assert!(!log.data.is_empty(), "{} should have data", file_path);

        eprintln!(
            "{}: {} channels, {} records, {:.1}s",
            file_path,
            log.channels.len(),
            log.data.len(),
            log.times.last().unwrap_or(&0.0)
        );
    }
}

// ============================================
// Error Handling Tests
// ============================================

#[test]
fn test_emerald_missing_lg2_file() {
    // Try to parse a non-existent file
    let result = Emerald::parse_file(Path::new("/nonexistent/path/test.lg1"));

    assert!(result.is_err(), "Should fail for missing files");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("lg2") || err_msg.contains("LG2"),
        "Error should mention missing LG2 file: {}",
        err_msg
    );
}

#[test]
fn test_emerald_parse_invalid_binary_data() {
    // Invalid data should fail detection
    let invalid = b"NOT_EMERALD_FORMAT_DATA";
    assert!(
        !Emerald::detect(invalid),
        "Should not detect invalid data as Emerald"
    );
}

// ============================================
// Performance Tests
// ============================================

#[test]
fn test_emerald_parse_performance() {
    if !example_file_exists(EMERALD_SHORT_DRIVE) {
        eprintln!("Skipping test: {} not found", EMERALD_SHORT_DRIVE);
        return;
    }

    let start = std::time::Instant::now();
    let log = Emerald::parse_file(Path::new(EMERALD_SHORT_DRIVE)).expect("Should parse");
    let elapsed = start.elapsed();

    eprintln!("Parsed {} records in {:?}", log.data.len(), elapsed);

    // Should complete in reasonable time (well under 1 second for small files)
    assert!(
        elapsed.as_secs() < 5,
        "Parsing should complete in reasonable time"
    );
}

// ============================================
// Channel Configuration Variation Tests
// ============================================

#[test]
fn test_emerald_different_channel_configs() {
    // Compare channel names between different log files
    // to verify we correctly parse different channel configurations

    let files = [
        (EMERALD_IDLE_REV, "idle/rev"),
        (EMERALD_DIFF_CHANNELS, "diff channels"),
    ];

    let mut configs: Vec<(String, Vec<String>)> = Vec::new();

    for (file_path, name) in files {
        if !example_file_exists(file_path) {
            eprintln!("Skipping: {} not found", file_path);
            continue;
        }

        let log = Emerald::parse_file(Path::new(file_path)).expect("Should parse");
        let channel_names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();

        eprintln!("{} channels: {:?}", name, channel_names);
        configs.push((name.to_string(), channel_names));
    }

    // If we have both files, verify they have different configurations
    if configs.len() >= 2 {
        let config1 = &configs[0].1;
        let config2 = &configs[1].1;

        // They should have some different channels
        let different = config1.iter().zip(config2.iter()).any(|(a, b)| a != b);

        assert!(
            different,
            "Different log files should have different channel configurations"
        );
    }
}
