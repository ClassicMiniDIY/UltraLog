//! MHD Tuning parser integration tests
//!
//! Covers detection, parsing of the example export, and the leading `#`
//! comment block handling reported in issue #78.

#[path = "../common/mod.rs"]
mod common;

use common::example_files::*;
use common::read_example_file;
use ultralog::parsers::mhd::Mhd;
use ultralog::parsers::{strip_leading_comment_lines, Parseable};

#[test]
fn test_detect_mhd_example_file() {
    let contents = read_example_file(MHD_N55_PULL);
    assert!(
        Mhd::detect(&contents),
        "Should detect the MHD example export"
    );
}

#[test]
fn test_parse_mhd_example_file() {
    let contents = read_example_file(MHD_N55_PULL);
    let log = Mhd.parse(&contents).expect("Failed to parse MHD example");

    // Header row is Time plus 31 columns; the trailing tune-file column is
    // captured as metadata rather than exposed as a channel
    assert_eq!(log.channels.len(), 30, "Expected 30 channels");
    assert!(
        !log.channels
            .iter()
            .any(|c| c.name().to_lowercase().ends_with(".bin")),
        "the tune file column must not become a channel"
    );
    assert_eq!(log.data.len(), 228, "Expected 228 data rows");
    assert_eq!(log.times.len(), log.data.len());

    // Every row is aligned with the channel list
    for row in &log.data {
        assert_eq!(row.len(), log.channels.len());
    }

    // Times are relative to the first sample and monotonically increasing
    assert_eq!(log.times[0], 0.0);
    assert!(log.times.windows(2).all(|w| w[0] <= w[1]));
    assert!((log.times.last().unwrap() - 29.438).abs() < 0.001);
}

#[test]
fn test_mhd_channel_names_and_units() {
    let contents = read_example_file(MHD_N55_PULL);
    let log = Mhd.parse(&contents).expect("Failed to parse MHD example");

    let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    for expected in ["Boost", "Coolant", "RPM", "Gear", "Lambda bank 1"] {
        assert!(
            names.iter().any(|n| n == expected),
            "Expected channel '{}' in {:?}",
            expected,
            names
        );
    }

    let unit_of = |name: &str| {
        log.channels
            .iter()
            .find(|c| c.name() == name)
            .unwrap_or_else(|| panic!("Missing channel '{}'", name))
            .unit()
            .to_string()
    };

    assert_eq!(unit_of("Boost"), "Bar");
    // MHD writes '*' where other tools write the degree sign
    assert_eq!(unit_of("Coolant"), "°C");
    assert_eq!(unit_of("RPM"), "RPM");
    // "(-)" marks a unitless channel
    assert_eq!(unit_of("Gear"), "");
}

#[test]
fn test_mhd_values_align_with_channels() {
    let contents = read_example_file(MHD_N55_PULL);
    let log = Mhd.parse(&contents).expect("Failed to parse MHD example");

    let index_of = |name: &str| {
        log.channels
            .iter()
            .position(|c| c.name() == name)
            .unwrap_or_else(|| panic!("Missing channel '{}'", name))
    };

    // First data row: ...,89 coolant,...,1346 rpm,...
    assert_eq!(log.data[0][index_of("Coolant")].as_f64(), 89.0);
    assert_eq!(log.data[0][index_of("RPM")].as_f64(), 1346.0);
    assert_eq!(log.data[0][index_of("Gear")].as_f64(), 2.0);
}

#[test]
fn test_mhd_not_confused_with_other_time_prefixed_formats() {
    // RomRaider-style export - no MHD fingerprint
    assert!(!Mhd::detect("Time,RPM,Load\n0,1000,50\n"));
    // Haltech
    assert!(!Mhd::detect("%DataLog%\nDataLogVersion : 1.1\n"));
    // ECUMaster
    assert!(!Mhd::detect("TIME;engine/rpm\n0.0;1000\n"));
}

#[test]
fn test_leading_comment_block_is_stripped_for_other_formats() {
    // Issue #78: exporters such as OBDLink write a '#' metadata preamble
    // ahead of the header row, which hid the header from every parser.
    let contents = "#Header info\n#More info\nTime,RPM,Load\n0,1000,50\n";
    assert_eq!(
        strip_leading_comment_lines(contents),
        "Time,RPM,Load\n0,1000,50\n"
    );
    assert!(ultralog::parsers::RomRaider::detect(
        strip_leading_comment_lines(contents)
    ));
}
