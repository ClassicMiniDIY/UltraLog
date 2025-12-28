//! Integration tests for UltraLog parser system
//!
//! These tests verify end-to-end parsing of example log files
//! from various ECU formats supported by UltraLog.

use ultralog::parsers::ecumaster::EcuMaster;
use ultralog::parsers::haltech::Haltech;
use ultralog::parsers::romraider::RomRaider;
use ultralog::parsers::speeduino::Speeduino;
use ultralog::parsers::types::Parseable;

/// Helper function to read a file, panicking with a clear message if not found.
/// This ensures CI catches missing example files instead of silently skipping tests.
fn read_example_file(file_path: &str) -> String {
    std::fs::read_to_string(file_path)
        .unwrap_or_else(|e| panic!("Failed to read example file '{}': {}", file_path, e))
}

/// Helper function to read a binary file, panicking with a clear message if not found.
fn read_example_binary(file_path: &str) -> Vec<u8> {
    std::fs::read(file_path)
        .unwrap_or_else(|e| panic!("Failed to read example file '{}': {}", file_path, e))
}

// ============================================
// Haltech Integration Tests
// ============================================

#[test]
fn test_haltech_example_log_parsing() {
    let file_path = "exampleLogs/haltech/2025-07-18_0215pm_Log1118.csv";
    let content = read_example_file(file_path);

    let parser = Haltech;
    let log = parser.parse(&content).expect("Should parse Haltech log");

    // Verify structure
    assert!(!log.channels.is_empty(), "Should have channels");
    assert!(!log.times.is_empty(), "Should have timestamps");
    assert!(!log.data.is_empty(), "Should have data records");

    // Verify data integrity
    assert_eq!(
        log.times.len(),
        log.data.len(),
        "Times and data should have same length"
    );

    // All records should have same number of values as channels
    let channel_count = log.channels.len();
    for record in &log.data {
        assert_eq!(
            record.len(),
            channel_count,
            "Each record should have {} values",
            channel_count
        );
    }

    // Timestamps should be monotonically increasing (relative to first)
    let times = log.get_times_as_f64();
    for window in times.windows(2) {
        assert!(
            window[1] >= window[0],
            "Timestamps should be monotonically increasing"
        );
    }

    eprintln!(
        "Haltech log: {} channels, {} records, time range: {:.2}s to {:.2}s",
        log.channels.len(),
        log.data.len(),
        times.first().unwrap_or(&0.0),
        times.last().unwrap_or(&0.0)
    );
}

#[test]
fn test_haltech_multi_log_file() {
    // This file contains multiple logs
    let file_path = "exampleLogs/haltech/2025-03-06_0937pm_Logs658to874.csv";
    let content = read_example_file(file_path);

    let parser = Haltech;
    let log = parser
        .parse(&content)
        .expect("Should parse multi-log Haltech file");

    assert!(!log.channels.is_empty(), "Should have channels");
    assert!(!log.data.is_empty(), "Should have data records");

    eprintln!(
        "Haltech multi-log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

// ============================================
// ECUMaster Integration Tests
// ============================================

#[test]
fn test_ecumaster_example_log_parsing() {
    let file_path = "exampleLogs/ecumaster/2025_1218_1903.csv";
    let content = read_example_file(file_path);

    // First verify detection
    assert!(
        EcuMaster::detect(&content),
        "Should detect as ECUMaster format"
    );

    let parser = EcuMaster;
    let log = parser.parse(&content).expect("Should parse ECUMaster log");

    assert!(!log.channels.is_empty(), "Should have channels");
    assert!(!log.times.is_empty(), "Should have timestamps");
    assert!(!log.data.is_empty(), "Should have data records");

    // Verify data integrity
    assert_eq!(
        log.times.len(),
        log.data.len(),
        "Times and data should have same length"
    );

    eprintln!(
        "ECUMaster log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

#[test]
fn test_ecumaster_large_file() {
    let file_path = "exampleLogs/ecumaster/Largest.csv";
    let content = read_example_file(file_path);

    // Format detection should always succeed for known ECUMaster files
    assert!(
        EcuMaster::detect(&content),
        "File '{}' should be detected as ECUMaster format",
        file_path
    );

    let parser = EcuMaster;
    let log = parser
        .parse(&content)
        .expect("Should parse large ECUMaster log");

    assert!(!log.channels.is_empty(), "Should have channels");
    assert!(!log.data.is_empty(), "Should have data records");

    eprintln!(
        "ECUMaster large log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

// ============================================
// RomRaider Integration Tests
// ============================================

#[test]
fn test_romraider_detection() {
    // Test positive detection - valid RomRaider samples
    let valid_romraider_simple = "Time,RPM,Load\n0,1000,50\n";
    let valid_romraider_with_units = "Time (msec),Engine Speed (rpm),Engine Load (%)\n0,1000,50\n";

    assert!(
        RomRaider::detect(valid_romraider_simple),
        "Should detect simple RomRaider format"
    );
    assert!(
        RomRaider::detect(valid_romraider_with_units),
        "Should detect RomRaider format with units in headers"
    );

    // Test negative detection - other formats should not be detected as RomRaider
    let haltech_sample = "%DataLog%\nDataLogVersion : 1.1\n";
    let ecumaster_sample = "TIME;RPM;MAP\n0.0;1000;50\n";

    assert!(
        !RomRaider::detect(haltech_sample),
        "Should not detect Haltech as RomRaider"
    );
    assert!(
        !RomRaider::detect(ecumaster_sample),
        "Should not detect ECUMaster as RomRaider"
    );
}

#[test]
fn test_romraider_parsing() {
    // Test with synthetic RomRaider data since no example file exists
    let sample_data =
        "Time (msec),Engine Speed (rpm),Engine Load (%),Coolant Temp (C),Battery Voltage (V)\n\
                       0,850,15.5,85.0,14.2\n\
                       20,900,18.0,85.5,14.1\n\
                       40,950,20.5,86.0,14.3\n\
                       60,1000,22.0,86.5,14.2\n";

    // Verify detection
    assert!(
        RomRaider::detect(sample_data),
        "Should detect as RomRaider format"
    );

    // Parse the data
    let parser = RomRaider;
    let log = parser
        .parse(sample_data)
        .expect("Should parse RomRaider log");

    // Verify structure
    assert_eq!(log.channels.len(), 4, "Should have 4 channels");
    assert_eq!(log.times.len(), 4, "Should have 4 timestamps");
    assert_eq!(log.data.len(), 4, "Should have 4 data records");

    // Verify channel names (units should be stripped)
    assert_eq!(log.channels[0].name(), "Engine Speed");
    assert_eq!(log.channels[1].name(), "Engine Load");
    assert_eq!(log.channels[2].name(), "Coolant Temp");
    assert_eq!(log.channels[3].name(), "Battery Voltage");

    // Verify timestamps (converted from ms to seconds)
    let times = log.get_times_as_f64();
    assert!(
        (times[0] - 0.0).abs() < 0.001,
        "First timestamp should be 0"
    );
    assert!(
        (times[1] - 0.020).abs() < 0.001,
        "Second timestamp should be 0.020"
    );
    assert!(
        (times[3] - 0.060).abs() < 0.001,
        "Fourth timestamp should be 0.060"
    );

    // Verify data values
    assert_eq!(log.data[0][0].as_f64(), 850.0);
    assert_eq!(log.data[3][0].as_f64(), 1000.0);

    // Verify data integrity
    assert_eq!(
        log.times.len(),
        log.data.len(),
        "Times and data should have same length"
    );

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

    eprintln!(
        "RomRaider log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

// ============================================
// Speeduino/rusEFI Integration Tests
// ============================================

#[test]
fn test_speeduino_mlg_parsing() {
    let file_path = "exampleLogs/speeduino/speeduino.mlg";
    let data = read_example_binary(file_path);

    assert!(Speeduino::detect(&data), "Should detect as MLG format");

    let log = Speeduino::parse_binary(&data).expect("Should parse Speeduino MLG");

    assert!(!log.channels.is_empty(), "Should have channels");
    assert!(!log.times.is_empty(), "Should have timestamps");
    assert!(!log.data.is_empty(), "Should have data records");

    // Verify data integrity
    assert_eq!(
        log.times.len(),
        log.data.len(),
        "Times and data should have same length"
    );

    // Verify all records have correct number of values
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

    eprintln!(
        "Speeduino log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

#[test]
fn test_rusefi_mlg_parsing() {
    let file_path = "exampleLogs/rusefi/rusefilog.mlg";
    let data = read_example_binary(file_path);

    assert!(Speeduino::detect(&data), "Should detect as MLG format");

    let log = Speeduino::parse_binary(&data).expect("Should parse rusEFI MLG");

    assert!(!log.channels.is_empty(), "Should have channels");
    assert!(!log.times.is_empty(), "Should have timestamps");
    assert!(!log.data.is_empty(), "Should have data records");

    // rusEFI logs can be large - verify we parsed a reasonable amount
    assert!(log.data.len() > 100, "Should have substantial data records");

    eprintln!(
        "rusEFI log: {} channels, {} records",
        log.channels.len(),
        log.data.len()
    );
}

// ============================================
// Cross-Format Tests
// ============================================

#[test]
fn test_format_detection_mutual_exclusion() {
    // Haltech format markers
    let haltech_data = "%DataLog%\nDataLogVersion : 1.1\n";

    // ECUMaster uses semicolon-separated CSV starting with "TIME"
    let ecumaster_data = "TIME;Engine speed\n0.0;1000\n";

    // RomRaider uses comma-separated CSV starting with "Time"
    let romraider_data = "Time,RPM,Load\n0,1000,50\n";

    // MLG binary format
    let mlg_data = b"MLVLG\x00\x00\x01";

    // Haltech detection
    assert!(
        haltech_data.starts_with("%DataLog%"),
        "Haltech marker should be present"
    );

    // ECUMaster detection
    assert!(
        EcuMaster::detect(ecumaster_data),
        "ECUMaster format should be detected"
    );
    assert!(
        !EcuMaster::detect(haltech_data),
        "Haltech should not be detected as ECUMaster"
    );
    assert!(
        !EcuMaster::detect(romraider_data),
        "RomRaider should not be detected as ECUMaster"
    );

    // RomRaider detection
    assert!(
        RomRaider::detect(romraider_data),
        "RomRaider format should be detected"
    );
    assert!(
        !RomRaider::detect(haltech_data),
        "Haltech should not be detected as RomRaider"
    );
    assert!(
        !RomRaider::detect(ecumaster_data),
        "ECUMaster should not be detected as RomRaider"
    );

    // Speeduino/MLG detection
    assert!(Speeduino::detect(mlg_data), "MLG format should be detected");
    assert!(
        !Speeduino::detect(haltech_data.as_bytes()),
        "Haltech should not be detected as MLG"
    );
}

// ============================================
// Channel Data Access Tests
// ============================================

#[test]
fn test_channel_data_extraction() {
    let file_path = "exampleLogs/haltech/2025-07-18_0215pm_Log1118.csv";
    let content = read_example_file(file_path);

    let parser = Haltech;
    let log = parser.parse(&content).expect("Should parse log");

    // Test get_channel_data for each channel
    for (idx, channel) in log.channels.iter().enumerate() {
        let data = log.get_channel_data(idx);
        assert_eq!(
            data.len(),
            log.data.len(),
            "Channel {} ({}) data length should match record count",
            idx,
            channel.name()
        );
    }

    // Test out of bounds access
    let oob_data = log.get_channel_data(999);
    assert!(
        oob_data.is_empty(),
        "Out of bounds access should return empty"
    );
}

#[test]
fn test_find_channel_index() {
    let file_path = "exampleLogs/haltech/2025-07-18_0215pm_Log1118.csv";
    let content = read_example_file(file_path);

    let parser = Haltech;
    let log = parser.parse(&content).expect("Should parse log");

    // Test finding channels that exist
    if !log.channels.is_empty() {
        let first_channel_name = log.channels[0].name();
        let found_idx = log.find_channel_index(&first_channel_name);
        assert_eq!(found_idx, Some(0), "Should find first channel at index 0");
    }

    // Test finding channel that doesn't exist
    let not_found = log.find_channel_index("NonExistentChannel12345");
    assert_eq!(
        not_found, None,
        "Should return None for non-existent channel"
    );
}

// ============================================
// Time Range Tests
// ============================================

#[test]
fn test_time_range_validity() {
    let file_path = "exampleLogs/speeduino/speeduino.mlg";
    let data = read_example_binary(file_path);

    let log = Speeduino::parse_binary(&data).expect("Should parse log");
    let times = log.get_times_as_f64();

    // Verify first timestamp is non-negative
    if !times.is_empty() {
        assert!(times[0] >= 0.0, "First timestamp should be non-negative");
    }

    // Verify all timestamps are finite
    for (i, &t) in times.iter().enumerate() {
        assert!(t.is_finite(), "Timestamp {} should be finite", i);
    }
}

// ============================================
// Data Integrity Tests
// ============================================

#[test]
fn test_data_values_are_finite() {
    let file_path = "exampleLogs/haltech/2025-07-18_0215pm_Log1118.csv";
    let content = read_example_file(file_path);

    let parser = Haltech;
    let log = parser.parse(&content).expect("Should parse log");

    // Verify all data values are finite (not NaN or Infinity)
    for (row_idx, row) in log.data.iter().enumerate() {
        for (col_idx, value) in row.iter().enumerate() {
            let f = value.as_f64();
            assert!(
                f.is_finite(),
                "Value at row {}, col {} should be finite, got {}",
                row_idx,
                col_idx,
                f
            );
        }
    }
}
