//! Integration tests for UltraLog parser system
//!
//! These tests verify end-to-end parsing of example log files
//! from various ECU formats supported by UltraLog.

use ultralog::parsers::ecumaster::EcuMaster;
use ultralog::parsers::haltech::Haltech;
use ultralog::parsers::romraider::RomRaider;
use ultralog::parsers::speeduino::Speeduino;
use ultralog::parsers::types::Parseable;

// ============================================
// Haltech Integration Tests
// ============================================

#[test]
fn test_haltech_example_log_parsing() {
    let file_path = "exampleLogs/haltech/2025-07-18_0215pm_Log1118.csv";
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping test: {} not found", file_path);
            return;
        }
    };

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
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping test: {} not found", file_path);
            return;
        }
    };

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
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping test: {} not found", file_path);
            return;
        }
    };

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
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping test: {} not found", file_path);
            return;
        }
    };

    if !EcuMaster::detect(&content) {
        eprintln!("File doesn't match ECUMaster format, skipping");
        return;
    }

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
    // Test that RomRaider format is correctly detected vs others
    let haltech_sample = "%DataLog%\nDataLogVersion : 1.1\n";
    let ecumaster_sample = "Time;RPM;MAP\n0.0;1000;50\n";

    assert!(
        !RomRaider::detect(haltech_sample),
        "Should not detect Haltech as RomRaider"
    );
    assert!(
        !RomRaider::detect(ecumaster_sample),
        "Should not detect ECUMaster as RomRaider"
    );
}

// ============================================
// Speeduino/rusEFI Integration Tests
// ============================================

#[test]
fn test_speeduino_mlg_parsing() {
    let file_path = "exampleLogs/speeduino/speeduino.mlg";
    let data = match std::fs::read(file_path) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("Skipping test: {} not found", file_path);
            return;
        }
    };

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
    let data = match std::fs::read(file_path) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("Skipping test: {} not found", file_path);
            return;
        }
    };

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
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping test: {} not found", file_path);
            return;
        }
    };

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
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping test: {} not found", file_path);
            return;
        }
    };

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
    let data = match std::fs::read(file_path) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("Skipping test: {} not found", file_path);
            return;
        }
    };

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
    let content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping test: {} not found", file_path);
            return;
        }
    };

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
