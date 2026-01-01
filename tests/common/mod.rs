//! Common test utilities shared across all test modules
//!
//! This module provides helper functions for reading example files,
//! creating test fixtures, and other common testing operations.

use std::path::Path;

/// Helper function to read a text file, panicking with a clear message if not found.
/// This ensures CI catches missing example files instead of silently skipping tests.
pub fn read_example_file(file_path: &str) -> String {
    std::fs::read_to_string(file_path)
        .unwrap_or_else(|e| panic!("Failed to read example file '{}': {}", file_path, e))
}

/// Helper function to read a binary file, panicking with a clear message if not found.
pub fn read_example_binary(file_path: &str) -> Vec<u8> {
    std::fs::read(file_path)
        .unwrap_or_else(|e| panic!("Failed to read example file '{}': {}", file_path, e))
}

/// Helper function to quote channel names that contain spaces for use in formulas
pub fn quote_if_needed(name: &str) -> String {
    if name.contains(' ') {
        format!("\"{}\"", name)
    } else {
        name.to_string()
    }
}

/// Check if an example file exists (useful for conditional tests)
pub fn example_file_exists(file_path: &str) -> bool {
    Path::new(file_path).exists()
}

/// Get the absolute path to an example file
pub fn get_example_file_path(file_path: &str) -> String {
    file_path.to_string()
}

/// Example log file paths for each ECU type
pub mod example_files {
    // Haltech example files
    pub const HALTECH_SMALL: &str = "exampleLogs/haltech/2025-07-18_0215pm_Log1118.csv";
    pub const HALTECH_LARGE: &str = "exampleLogs/haltech/2025-03-06_0937pm_Logs658to874.csv";

    // ECUMaster example files
    pub const ECUMASTER_STANDARD: &str = "exampleLogs/ecumaster/2025_1218_1903.csv";
    pub const ECUMASTER_LARGE: &str = "exampleLogs/ecumaster/Largest.csv";

    // Speeduino example files
    pub const SPEEDUINO_MLG: &str = "exampleLogs/speeduino/speeduino.mlg";

    // rusEFI example files (same parser as Speeduino)
    pub const RUSEFI_MLG: &str = "exampleLogs/rusefi/rusefilog.mlg";
    pub const RUSEFI_LOG1: &str = "exampleLogs/rusefi/Log1.mlg";

    // AiM example files
    pub const AIM_GENERIC: &str = "exampleLogs/aim/BMW_THill 5mi_Generic testing_a_1033.xrk";
    pub const AIM_RACE_1: &str = "exampleLogs/aim/BMW_THill 5mi_BY_Race_a_1441.xrk";
    pub const AIM_RACE_2: &str = "exampleLogs/aim/BMW_THill 5mi_BY_Race_a_1448.xrk";

    // Link ECU example files
    pub const LINK_SMALL: &str = "exampleLogs/link/ECU Log 2024-02-8 3;56;20 pm.llg5";
    pub const LINK_MEDIUM: &str = "exampleLogs/link/ECU Log 2024-03-14 2;04;31 pm.llg5";
    pub const LINK_LARGE: &str = "exampleLogs/link/ECU Log 2024-03-22 11;20;32 am.llg5";
    pub const LINK_STANDARD: &str = "exampleLogs/link/linklog.llg";

    // RomRaider example files
    pub const ROMRAIDER_EUROPEAN: &str = "exampleLogs/romraider/romraiderlog_20251031_170713.csv";

    // Emerald ECU example files
    pub const EMERALD_IDLE_REV: &str = "exampleLogs/emerald/EM Log MG ZS Turbo idle and rev.lg1";
    pub const EMERALD_SHORT_DRIVE: &str = "exampleLogs/emerald/EM Log MG ZS Turbo short drive.lg1";
    pub const EMERALD_DIFF_CHANNELS: &str =
        "exampleLogs/emerald/EM Log MG ZS Turbo short drive back diff channels.lg1";
}

/// Test data generators for synthetic tests
pub mod synthetic {
    use ultralog::parsers::types::Value;

    /// Create a simple data matrix with linear values
    pub fn linear_data(channels: usize, records: usize) -> Vec<Vec<Value>> {
        (0..records)
            .map(|r| {
                (0..channels)
                    .map(|c| Value::Float((r * channels + c) as f64))
                    .collect()
            })
            .collect()
    }

    /// Create a time array with uniform spacing
    pub fn uniform_times(count: usize, interval: f64) -> Vec<f64> {
        (0..count).map(|i| i as f64 * interval).collect()
    }

    /// Create a simple Haltech-style CSV header
    pub fn haltech_header() -> &'static str {
        "%DataLog%\nDataLogVersion : 1.1\n"
    }

    /// Create a simple RomRaider-style CSV header
    pub fn romraider_header(columns: &[&str]) -> String {
        format!("Time (msec),{}\n", columns.join(","))
    }

    /// Create an ECUMaster-style CSV header with semicolon delimiter
    pub fn ecumaster_header(columns: &[&str]) -> String {
        format!("TIME;{}\n", columns.join(";"))
    }
}

/// Assertion helpers for common test patterns
pub mod assertions {
    use ultralog::parsers::types::Log;

    /// Assert that a log has valid structure (channels, times, data all present and aligned)
    pub fn assert_valid_log_structure(log: &Log) {
        assert!(!log.channels.is_empty(), "Log should have channels");
        assert!(!log.times.is_empty(), "Log should have timestamps");
        assert!(!log.data.is_empty(), "Log should have data records");

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
                "Record {} should have {} values, got {}",
                i,
                channel_count,
                record.len()
            );
        }
    }

    /// Assert that all timestamps are monotonically increasing
    pub fn assert_monotonic_times(log: &Log) {
        let times = log.get_times_as_f64();
        for (i, window) in times.windows(2).enumerate() {
            assert!(
                window[1] >= window[0],
                "Timestamps at index {} should be monotonically increasing: {} >= {}",
                i,
                window[1],
                window[0]
            );
        }
    }

    /// Assert that all data values are finite (not NaN or Infinity)
    pub fn assert_finite_values(log: &Log) {
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

    /// Assert that a log has at least the minimum expected records
    pub fn assert_minimum_records(log: &Log, min_records: usize) {
        assert!(
            log.data.len() >= min_records,
            "Expected at least {} records, got {}",
            min_records,
            log.data.len()
        );
    }

    /// Assert that a log has at least the minimum expected channels
    pub fn assert_minimum_channels(log: &Log, min_channels: usize) {
        assert!(
            log.channels.len() >= min_channels,
            "Expected at least {} channels, got {}",
            min_channels,
            log.channels.len()
        );
    }

    /// Assert that time range is valid (first timestamp non-negative, last >= first)
    pub fn assert_valid_time_range(log: &Log) {
        let times = log.get_times_as_f64();
        if !times.is_empty() {
            assert!(
                times[0] >= 0.0,
                "First timestamp should be non-negative, got {}",
                times[0]
            );
            let last = times.last().unwrap();
            assert!(
                *last >= times[0],
                "Last timestamp {} should be >= first timestamp {}",
                last,
                times[0]
            );
        }
    }
}

/// Float comparison helpers for testing
pub mod float_cmp {
    /// Check if two floats are approximately equal within a tolerance
    pub fn approx_eq(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() < tolerance
    }

    /// Assert that two floats are approximately equal
    pub fn assert_approx_eq(a: f64, b: f64, tolerance: f64) {
        assert!(
            approx_eq(a, b, tolerance),
            "Values not approximately equal: {} vs {} (tolerance: {})",
            a,
            b,
            tolerance
        );
    }

    /// Default tolerance for float comparisons (0.0001)
    pub const DEFAULT_TOLERANCE: f64 = 0.0001;
}
