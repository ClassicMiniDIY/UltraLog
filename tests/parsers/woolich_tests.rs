//! Comprehensive tests for the Woolich Racing Tuned (WRT) parser.
//!
//! Tests cover:
//! - Format detection (and rejection of similar "Time"-prefixed CSVs)
//! - `HH:MM:SS.mmm` timestamp parsing normalized to relative seconds
//! - Boolean channel handling (`True`/`False` -> 1.0 / 0.0)
//! - Unit inference from channel names (WRT doesn't embed units)
//! - Real file parsing using the bundled Woolich example log

#[path = "../common/mod.rs"]
mod common;

use common::assertions::*;
use common::example_files::WOOLICH_STANDARD;
use common::float_cmp::*;
use common::{example_file_exists, read_example_file};
use ultralog::parsers::types::Parseable;
use ultralog::parsers::woolich::Woolich;

const SAMPLE: &str = "Log Time,RPM,TPS,IAP,AFR,Gear,Clutch In,Coolant Temp,IAT,\n\
    00:00:00.040,0,0.00,100.00,0.0,0,False,24.0,25.0,\n\
    00:00:00.100,1224,12.50,59.00,13.2,0,True,92.0,26.0,\n\
    00:00:00.160,3065,7.84,91.00,14.0,2,False,27.0,26.0,\n";

// ============================================
// Detection
// ============================================

#[test]
fn detects_synthetic_woolich_header() {
    assert!(Woolich::detect(SAMPLE));
}

#[test]
fn rejects_romraider_header() {
    let romraider = "Time (msec),Engine Speed (rpm),Throttle (%)\n0,1000,5\n";
    assert!(!Woolich::detect(romraider));
}

#[test]
fn rejects_me221_generic_time_csv() {
    let generic = "Time,RPM,Load\n0,1000,50\n";
    assert!(!Woolich::detect(generic));
}

#[test]
fn rejects_log_time_header_with_non_timestamp_rows() {
    // "Log Time" header but the data is plain seconds, not HH:MM:SS.
    assert!(!Woolich::detect("Log Time,RPM\n1.23,1000\n"));
}

// ============================================
// Timestamp handling
// ============================================

#[test]
fn normalizes_first_time_to_zero() {
    let log = Woolich.parse(SAMPLE).expect("parses");
    assert_approx_eq(log.times[0], 0.0, 1e-9);
    assert_approx_eq(log.times[1], 0.06, 1e-6);
    assert_approx_eq(log.times[2], 0.12, 1e-6);
}

// ============================================
// Boolean channels
// ============================================

#[test]
fn boolean_clutch_maps_to_one_and_zero() {
    let log = Woolich.parse(SAMPLE).expect("parses");
    let clutch_idx = log
        .channels
        .iter()
        .position(|c| c.name() == "Clutch In")
        .expect("Clutch In channel present");

    assert_approx_eq(log.data[0][clutch_idx].as_f64(), 0.0, 1e-9); // False
    assert_approx_eq(log.data[1][clutch_idx].as_f64(), 1.0, 1e-9); // True
}

// ============================================
// Unit inference
// ============================================

#[test]
fn infers_units_for_common_channels() {
    let log = Woolich.parse(SAMPLE).expect("parses");

    let units: std::collections::HashMap<String, String> = log
        .channels
        .iter()
        .map(|c| (c.name(), c.unit().to_string()))
        .collect();

    assert_eq!(units.get("RPM").map(String::as_str), Some("RPM"));
    assert_eq!(units.get("TPS").map(String::as_str), Some("%"));
    assert_eq!(units.get("IAP").map(String::as_str), Some("kPa"));
    assert_eq!(units.get("AFR").map(String::as_str), Some("AFR"));
    assert_eq!(units.get("Coolant Temp").map(String::as_str), Some("°C"));
    assert_eq!(units.get("IAT").map(String::as_str), Some("°C"));
}

// ============================================
// Trailing comma handling
// ============================================

#[test]
fn drops_trailing_empty_column() {
    // The header carries a trailing comma; it must not become a phantom channel.
    let log = Woolich.parse(SAMPLE).expect("parses");
    assert_eq!(log.channels.len(), 8);
    assert_eq!(log.data[0].len(), 8);
}

// ============================================
// Real example file
// ============================================

#[test]
fn parses_woolich_standard_example_file() {
    if !example_file_exists(WOOLICH_STANDARD) {
        eprintln!("Skipping test: {} not found", WOOLICH_STANDARD);
        return;
    }

    let content = read_example_file(WOOLICH_STANDARD);
    assert!(Woolich::detect(&content), "Woolich log should be detected");

    let log = Woolich.parse(&content).expect("Should parse Woolich log");

    assert_valid_log_structure(&log);
    assert_finite_values(&log);
    assert_monotonic_times(&log);
    assert_valid_time_range(&log);

    // The bundled log has 8 channels and ~28k records over several minutes.
    assert_eq!(log.channels.len(), 8);
    assert_minimum_records(&log, 1000);

    let last = *log.get_times_as_f64().last().unwrap();
    assert!(
        (10.0..=3600.0).contains(&last),
        "Last timestamp ({}) should be in seconds",
        last
    );
}
