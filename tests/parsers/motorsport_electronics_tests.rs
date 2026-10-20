//! Comprehensive tests for the Motorsport Electronics (ME221/ME442) parser.
//!
//! Tests cover:
//! - Format detection (and rejection of similar CSVs like RomRaider)
//! - Unit inference from channel names (ME Tuner doesn't embed units)
//! - Time-in-seconds handling (regression test for 1000x scaling bug
//!   caused by the RomRaider parser treating Time as milliseconds)
//! - Real file parsing using the bundled ME221 and ME442 example logs

#[path = "../common/mod.rs"]
mod common;

use common::assertions::*;
use common::float_cmp::*;
use common::{example_file_exists, read_example_file};
use ultralog::parsers::motorsport_electronics::MotorsportElectronics;
use ultralog::parsers::types::Parseable;

const ME221_SMALL: &str = "exampleLogs/motorsportElectronics/ME221_2025_12_22_13_13_40.csv";
const ME221_LARGE: &str = "exampleLogs/motorsportElectronics/ME221_2026_04_12_11_59_52.csv";
/// Issue #95 log: an ME442 with a custom 8-channel list, 17,140 rows over
/// 1,878 seconds, CRLF line endings.
const ME442_ISSUE_95: &str =
    "exampleLogs/motorsportElectronics/ME442-V2B-PNP_2026_10_20_16_44_06.csv";

const SAMPLE_HEADER: &str =
    "Time, Marker, RPM, Sync status, Lost sync count, Injector duty, MAP Raw, MAP, TPS Raw, TPS";

fn synthetic_sample() -> String {
    format!(
        "{}\n0.00,0,1000.00,2.00,0.00,0.64,5908.00,27.05,5870.00,0.00\n\
         0.02,0,1050.00,2.00,0.00,0.65,5924.00,27.09,5881.00,0.00\n\
         0.04,0,1100.00,2.00,0.00,0.66,5931.00,27.14,5853.00,0.00\n",
        SAMPLE_HEADER
    )
}

// ============================================
// Detection
// ============================================

#[test]
fn detects_synthetic_me221_header() {
    assert!(MotorsportElectronics::detect(&synthetic_sample()));
}

#[test]
fn rejects_romraider_header() {
    let romraider = "Time (msec),Engine Speed (rpm),Throttle (%)\n0,1000,5\n";
    assert!(!MotorsportElectronics::detect(romraider));
}

#[test]
fn rejects_generic_time_csv() {
    // Starts with "Time," but lacks ME Tuner fingerprint columns.
    let generic = "Time,RPM,Load\n0,1000,50\n";
    assert!(!MotorsportElectronics::detect(generic));
}

// ============================================
// Time scaling regression
// ============================================

#[test]
fn time_column_is_seconds_not_milliseconds() {
    let parser = MotorsportElectronics;
    let log = parser.parse(&synthetic_sample()).expect("parses");

    // First row normalized to zero; the 0.04 second row should land near 40 ms.
    assert_approx_eq(log.times[0], 0.0, DEFAULT_TOLERANCE);
    assert_approx_eq(log.times[2], 0.04, 1e-6);
}

#[test]
fn normalizes_first_time_to_zero() {
    // ME Tuner starts the Time column at a non-zero offset (e.g. 0.06 s).
    let header = SAMPLE_HEADER;
    let body = "0.06,0,1000,2,0,0.64,5900,27,5870,0\n0.11,0,1050,2,0,0.65,5920,27,5880,0\n";
    let contents = format!("{}\n{}", header, body);

    let log = MotorsportElectronics.parse(&contents).expect("parses");
    assert_approx_eq(log.times[0], 0.0, 1e-9);
    assert_approx_eq(log.times[1], 0.05, 1e-6);
}

// ============================================
// Unit inference
// ============================================

#[test]
fn infers_units_for_common_channels() {
    let log = MotorsportElectronics
        .parse(&synthetic_sample())
        .expect("parses");

    let units: std::collections::HashMap<String, String> = log
        .channels
        .iter()
        .map(|c| (c.name(), c.unit().to_string()))
        .collect();

    assert_eq!(units.get("RPM").map(String::as_str), Some("RPM"));
    assert_eq!(units.get("MAP").map(String::as_str), Some("kPa"));
    assert_eq!(units.get("TPS").map(String::as_str), Some("%"));
    assert_eq!(units.get("Injector duty").map(String::as_str), Some("%"));
}

// ============================================
// Real example files
// ============================================

#[test]
fn parses_me221_small_example_file() {
    if !example_file_exists(ME221_SMALL) {
        eprintln!("Skipping test: {} not found", ME221_SMALL);
        return;
    }

    let content = read_example_file(ME221_SMALL);
    assert!(
        MotorsportElectronics::detect(&content),
        "Small ME221 log should be detected"
    );

    let log = MotorsportElectronics
        .parse(&content)
        .expect("Should parse small ME221 log");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);
    assert_monotonic_times(&log);
    assert_valid_time_range(&log);

    // The sample is a short idle snippet but should still have plenty of channels.
    assert_minimum_channels(&log, 50);
    assert_minimum_records(&log, 10);

    // Sanity-check the time range sits in real seconds, not milliseconds.
    let last = *log.get_times_as_f64().last().unwrap();
    assert!(
        (0.1..=3600.0).contains(&last),
        "Last timestamp ({}) should be in seconds, not ms",
        last
    );
}

#[test]
fn parses_me221_large_example_file() {
    if !example_file_exists(ME221_LARGE) {
        eprintln!("Skipping test: {} not found", ME221_LARGE);
        return;
    }

    let content = read_example_file(ME221_LARGE);
    assert!(
        MotorsportElectronics::detect(&content),
        "Large ME221 log should be detected"
    );

    let log = MotorsportElectronics
        .parse(&content)
        .expect("Should parse large ME221 log");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);
    assert_monotonic_times(&log);

    assert_minimum_channels(&log, 100);
    assert_minimum_records(&log, 1000);

    let last = *log.get_times_as_f64().last().unwrap();
    assert!(
        last > 10.0,
        "Large ME221 log should span more than 10s (got {} s)",
        last
    );
}

#[test]
fn parses_me442_issue_95_example_file() {
    if !example_file_exists(ME442_ISSUE_95) {
        eprintln!("Skipping test: {} not found", ME442_ISSUE_95);
        return;
    }

    let content = read_example_file(ME442_ISSUE_95);
    assert!(
        MotorsportElectronics::detect(&content),
        "ME442 log with a custom channel list should be detected"
    );

    let log = MotorsportElectronics
        .parse(&content)
        .expect("Should parse ME442 log");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);
    assert_monotonic_times(&log);

    assert_eq!(log.channels.len(), 8);
    assert_eq!(log.data.len(), 17_140);

    // The bug: RomRaider read the bare "Time" column as ms and showed
    // 1.878 s. The log spans 0.11 s to 1878.42 s.
    let last = *log.get_times_as_f64().last().unwrap();
    assert_approx_eq(last, 1878.31, 1e-6);

    let units: std::collections::HashMap<String, String> = log
        .channels
        .iter()
        .map(|c| (c.name(), c.unit().to_string()))
        .collect();
    assert_eq!(units.get("RPM").map(String::as_str), Some("RPM"));
    assert_eq!(units.get("MAP").map(String::as_str), Some("kPa"));
    assert_eq!(units.get("Coolant Temp.").map(String::as_str), Some("°C"));
    assert_eq!(
        units.get("Lambda Ext. AFR").map(String::as_str),
        Some("AFR")
    );
    assert_eq!(units.get("ALS Status").map(String::as_str), Some(""));

    // First data row: 0.11,0,1004.00,35.70,59.00,0.00,12.04,0.00,0.00
    let rpm_idx = log.channels.iter().position(|c| c.name() == "RPM").unwrap();
    let afr_idx = log
        .channels
        .iter()
        .position(|c| c.name() == "Lambda Ext. AFR")
        .unwrap();
    assert_approx_eq(log.data[0][rpm_idx].as_f64(), 1004.0, 1e-9);
    assert_approx_eq(log.data[0][afr_idx].as_f64(), 12.04, 1e-9);
}
