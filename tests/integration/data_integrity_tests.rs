//! Data integrity tests across all file formats
//!
//! Tests verify that parsed data meets quality and consistency requirements.

#[path = "../common/mod.rs"]
mod common;

use common::example_files::*;
use common::{example_file_exists, get_example_file_path, read_example_binary, read_example_file};
use std::path::Path;
use ultralog::parsers::aim::Aim;
use ultralog::parsers::ecumaster::EcuMaster;
use ultralog::parsers::haltech::Haltech;
use ultralog::parsers::link::Link;
use ultralog::parsers::speeduino::Speeduino;
use ultralog::parsers::types::{Log, Parseable};

// ============================================
// Time Monotonicity Tests
// ============================================

fn assert_time_monotonicity(log: &Log, format: &str) {
    let times = log.get_times_as_f64();
    for (i, window) in times.windows(2).enumerate() {
        assert!(
            window[1] >= window[0],
            "{}: Timestamps at index {} should be monotonic: {} >= {}",
            format,
            i,
            window[1],
            window[0]
        );
    }
}

#[test]
fn test_haltech_time_monotonicity() {
    if !example_file_exists(HALTECH_SMALL) {
        return;
    }

    let content = read_example_file(HALTECH_SMALL);
    let log = Haltech.parse(&content).expect("Should parse");
    assert_time_monotonicity(&log, "Haltech");
}

#[test]
fn test_ecumaster_time_monotonicity() {
    if !example_file_exists(ECUMASTER_STANDARD) {
        return;
    }

    let content = read_example_file(ECUMASTER_STANDARD);
    let log = EcuMaster.parse(&content).expect("Should parse");
    assert_time_monotonicity(&log, "ECUMaster");
}

#[test]
fn test_speeduino_time_monotonicity() {
    if !example_file_exists(SPEEDUINO_MLG) {
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);
    let log = Speeduino::parse_binary(&data).expect("Should parse");
    assert_time_monotonicity(&log, "Speeduino");
}

#[test]
fn test_aim_time_monotonicity() {
    if !example_file_exists(AIM_GENERIC) {
        return;
    }

    let path = get_example_file_path(AIM_GENERIC);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse");
    assert_time_monotonicity(&log, "AiM");
}

#[test]
fn test_link_time_monotonicity() {
    if !example_file_exists(LINK_STANDARD) {
        return;
    }

    let data = read_example_binary(LINK_STANDARD);
    let log = Link::parse_binary(&data).expect("Should parse");
    assert_time_monotonicity(&log, "Link");
}

// ============================================
// Value Finiteness Tests
// ============================================

fn assert_all_values_finite(log: &Log, format: &str) {
    for (row_idx, row) in log.data.iter().enumerate() {
        for (col_idx, value) in row.iter().enumerate() {
            let f = value.as_f64();
            assert!(
                f.is_finite(),
                "{}: Value at row {}, col {} should be finite, got {}",
                format,
                row_idx,
                col_idx,
                f
            );
        }
    }
}

#[test]
fn test_haltech_values_finite() {
    if !example_file_exists(HALTECH_SMALL) {
        return;
    }

    let content = read_example_file(HALTECH_SMALL);
    let log = Haltech.parse(&content).expect("Should parse");
    assert_all_values_finite(&log, "Haltech");
}

#[test]
fn test_ecumaster_values_finite() {
    if !example_file_exists(ECUMASTER_STANDARD) {
        return;
    }

    let content = read_example_file(ECUMASTER_STANDARD);
    let log = EcuMaster.parse(&content).expect("Should parse");
    assert_all_values_finite(&log, "ECUMaster");
}

#[test]
fn test_speeduino_values_finite() {
    if !example_file_exists(SPEEDUINO_MLG) {
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);
    let log = Speeduino::parse_binary(&data).expect("Should parse");
    assert_all_values_finite(&log, "Speeduino");
}

#[test]
fn test_aim_values_finite() {
    if !example_file_exists(AIM_GENERIC) {
        return;
    }

    let path = get_example_file_path(AIM_GENERIC);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse");
    assert_all_values_finite(&log, "AiM");
}

#[test]
fn test_link_values_finite() {
    if !example_file_exists(LINK_STANDARD) {
        return;
    }

    let data = read_example_binary(LINK_STANDARD);
    let log = Link::parse_binary(&data).expect("Should parse");
    assert_all_values_finite(&log, "Link");
}

// ============================================
// Data Alignment Tests
// ============================================

fn assert_data_alignment(log: &Log, format: &str) {
    // Times and data should have same length
    assert_eq!(
        log.times.len(),
        log.data.len(),
        "{}: Times and data should have same length",
        format
    );

    // Each record should have correct number of values
    let channel_count = log.channels.len();
    for (i, record) in log.data.iter().enumerate() {
        assert_eq!(
            record.len(),
            channel_count,
            "{}: Record {} should have {} values, got {}",
            format,
            i,
            channel_count,
            record.len()
        );
    }
}

#[test]
fn test_haltech_data_alignment() {
    if !example_file_exists(HALTECH_SMALL) {
        return;
    }

    let content = read_example_file(HALTECH_SMALL);
    let log = Haltech.parse(&content).expect("Should parse");
    assert_data_alignment(&log, "Haltech");
}

#[test]
fn test_ecumaster_data_alignment() {
    if !example_file_exists(ECUMASTER_STANDARD) {
        return;
    }

    let content = read_example_file(ECUMASTER_STANDARD);
    let log = EcuMaster.parse(&content).expect("Should parse");
    assert_data_alignment(&log, "ECUMaster");
}

#[test]
fn test_speeduino_data_alignment() {
    if !example_file_exists(SPEEDUINO_MLG) {
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);
    let log = Speeduino::parse_binary(&data).expect("Should parse");
    assert_data_alignment(&log, "Speeduino");
}

#[test]
fn test_aim_data_alignment() {
    if !example_file_exists(AIM_GENERIC) {
        return;
    }

    let path = get_example_file_path(AIM_GENERIC);
    let log = Aim::parse_file(Path::new(&path)).expect("Should parse");
    assert_data_alignment(&log, "AiM");
}

#[test]
fn test_link_data_alignment() {
    if !example_file_exists(LINK_STANDARD) {
        return;
    }

    let data = read_example_binary(LINK_STANDARD);
    let log = Link::parse_binary(&data).expect("Should parse");
    assert_data_alignment(&log, "Link");
}

// ============================================
// Time Range Validity Tests
// ============================================

fn assert_valid_time_range_values(log: &Log, format: &str) {
    let times = log.get_times_as_f64();
    if times.is_empty() {
        return;
    }

    // First timestamp should be non-negative (relative to start)
    assert!(
        times[0] >= 0.0,
        "{}: First timestamp should be non-negative, got {}",
        format,
        times[0]
    );

    // All timestamps should be finite
    for (i, &t) in times.iter().enumerate() {
        assert!(
            t.is_finite(),
            "{}: Timestamp {} should be finite, got {}",
            format,
            i,
            t
        );
    }

    // Time range should be reasonable (not more than 24 hours for most logs)
    let duration = times.last().unwrap() - times[0];
    assert!(
        duration < 86400.0,
        "{}: Log duration {} seconds seems unreasonable",
        format,
        duration
    );
}

#[test]
fn test_haltech_time_range() {
    if !example_file_exists(HALTECH_SMALL) {
        return;
    }

    let content = read_example_file(HALTECH_SMALL);
    let log = Haltech.parse(&content).expect("Should parse");
    assert_valid_time_range_values(&log, "Haltech");
}

#[test]
fn test_ecumaster_time_range() {
    if !example_file_exists(ECUMASTER_STANDARD) {
        return;
    }

    let content = read_example_file(ECUMASTER_STANDARD);
    let log = EcuMaster.parse(&content).expect("Should parse");
    assert_valid_time_range_values(&log, "ECUMaster");
}

#[test]
fn test_speeduino_time_range() {
    if !example_file_exists(SPEEDUINO_MLG) {
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);
    let log = Speeduino::parse_binary(&data).expect("Should parse");
    assert_valid_time_range_values(&log, "Speeduino");
}

// ============================================
// Channel Data Consistency Tests
// ============================================

#[test]
fn test_channel_data_extraction_consistency() {
    if !example_file_exists(HALTECH_SMALL) {
        return;
    }

    let content = read_example_file(HALTECH_SMALL);
    let log = Haltech.parse(&content).expect("Should parse");

    // Verify get_channel_data returns consistent data
    for idx in 0..log.channels.len() {
        let data = log.get_channel_data(idx);

        // Length should match data rows
        assert_eq!(data.len(), log.data.len());

        // Values should match direct access
        for (i, &value) in data.iter().enumerate() {
            let direct = log.data[i][idx].as_f64();
            assert_eq!(value, direct, "Channel {} data at row {} mismatch", idx, i);
        }
    }
}

#[test]
fn test_out_of_bounds_channel_access() {
    if !example_file_exists(HALTECH_SMALL) {
        return;
    }

    let content = read_example_file(HALTECH_SMALL);
    let log = Haltech.parse(&content).expect("Should parse");

    // Out of bounds should return empty, not panic
    let oob = log.get_channel_data(999);
    assert!(oob.is_empty());

    let oob = log.get_channel_data(usize::MAX);
    assert!(oob.is_empty());
}

// ============================================
// Cross-Format Consistency Tests
// ============================================

#[test]
fn test_all_formats_have_channels() {
    // Text formats
    if example_file_exists(HALTECH_SMALL) {
        let content = read_example_file(HALTECH_SMALL);
        let log = Haltech.parse(&content).expect("Haltech");
        assert!(!log.channels.is_empty(), "Haltech should have channels");
    }

    if example_file_exists(ECUMASTER_STANDARD) {
        let content = read_example_file(ECUMASTER_STANDARD);
        let log = EcuMaster.parse(&content).expect("ECUMaster");
        assert!(!log.channels.is_empty(), "ECUMaster should have channels");
    }

    // Binary formats
    if example_file_exists(SPEEDUINO_MLG) {
        let data = read_example_binary(SPEEDUINO_MLG);
        let log = Speeduino::parse_binary(&data).expect("Speeduino");
        assert!(!log.channels.is_empty(), "Speeduino should have channels");
    }

    if example_file_exists(AIM_GENERIC) {
        let path = get_example_file_path(AIM_GENERIC);
        let log = Aim::parse_file(Path::new(&path)).expect("AiM");
        assert!(!log.channels.is_empty(), "AiM should have channels");
    }

    if example_file_exists(LINK_STANDARD) {
        let data = read_example_binary(LINK_STANDARD);
        let log = Link::parse_binary(&data).expect("Link");
        assert!(!log.channels.is_empty(), "Link should have channels");
    }
}

#[test]
fn test_all_formats_have_data() {
    // Text formats
    if example_file_exists(HALTECH_SMALL) {
        let content = read_example_file(HALTECH_SMALL);
        let log = Haltech.parse(&content).expect("Haltech");
        assert!(!log.data.is_empty(), "Haltech should have data");
    }

    if example_file_exists(ECUMASTER_STANDARD) {
        let content = read_example_file(ECUMASTER_STANDARD);
        let log = EcuMaster.parse(&content).expect("ECUMaster");
        assert!(!log.data.is_empty(), "ECUMaster should have data");
    }

    // Binary formats
    if example_file_exists(SPEEDUINO_MLG) {
        let data = read_example_binary(SPEEDUINO_MLG);
        let log = Speeduino::parse_binary(&data).expect("Speeduino");
        assert!(!log.data.is_empty(), "Speeduino should have data");
    }

    if example_file_exists(AIM_GENERIC) {
        let path = get_example_file_path(AIM_GENERIC);
        let log = Aim::parse_file(Path::new(&path)).expect("AiM");
        assert!(!log.data.is_empty(), "AiM should have data");
    }

    if example_file_exists(LINK_STANDARD) {
        let data = read_example_binary(LINK_STANDARD);
        let log = Link::parse_binary(&data).expect("Link");
        assert!(!log.data.is_empty(), "Link should have data");
    }
}
