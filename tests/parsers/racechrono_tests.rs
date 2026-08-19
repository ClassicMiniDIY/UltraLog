//! RaceChrono CSV v3 parser integration tests
//!
//! Covers detection, parsing of the example session excerpt, duplicate
//! column-name disambiguation, and the unix-timestamp time base.

#[path = "../common/mod.rs"]
mod common;

use common::example_files::*;
use common::read_example_file;
use ultralog::parsers::racechrono::RaceChrono;
use ultralog::parsers::types::Meta;
use ultralog::parsers::{EcuMaster, Mhd, Parseable, RomRaider};

#[test]
fn test_detect_racechrono_example_file() {
    let contents = read_example_file(RACECHRONO_V3_EXCERPT);
    assert!(
        RaceChrono::detect(&contents),
        "Should detect the RaceChrono example export"
    );
}

#[test]
fn test_parse_racechrono_example_file() {
    let contents = read_example_file(RACECHRONO_V3_EXCERPT);
    let log = RaceChrono
        .parse(&contents)
        .expect("Failed to parse RaceChrono example");

    // Header row is `timestamp` plus 34 channel columns
    assert_eq!(log.channels.len(), 34, "Expected 34 channels");
    assert_eq!(log.data.len(), 400, "Expected 400 data rows");
    assert_eq!(log.times.len(), log.data.len());

    // Every row is aligned with the channel list
    for row in &log.data {
        assert_eq!(row.len(), log.channels.len());
    }

    // Times are relative to the first sample and monotonically increasing.
    // The excerpt splices two slices of the session (pit lane + lap 29), so
    // the span covers the fragment gap between them.
    assert_eq!(log.times[0], 0.0);
    assert!(log.times.windows(2).all(|w| w[0] <= w[1]));
    assert!((log.times.last().unwrap() - 4095.307).abs() < 0.001);
}

#[test]
fn test_racechrono_metadata() {
    let contents = read_example_file(RACECHRONO_V3_EXCERPT);
    let log = RaceChrono
        .parse(&contents)
        .expect("Failed to parse RaceChrono example");

    match log.meta {
        Meta::RaceChrono(meta) => {
            assert_eq!(meta.format_version, 3);
            assert_eq!(meta.session_title, "Copia di Massa Finalese");
            assert_eq!(meta.session_type, "Lap timing");
            assert_eq!(meta.track_name, "Copia di Massa Finalese");
            assert!(meta.creator.contains("RaceChrono"));
            assert_eq!(meta.channel_count, 34);
            assert_eq!(meta.data_points, 400);
        }
        other => panic!("expected RaceChrono metadata, got {:?}", other),
    }
}

#[test]
fn test_racechrono_duplicate_names_disambiguated_by_source() {
    let contents = read_example_file(RACECHRONO_V3_EXCERPT);
    let log = RaceChrono
        .parse(&contents)
        .expect("Failed to parse RaceChrono example");

    let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();

    // `speed` comes from both GPS and the calc pipeline
    for expected in ["speed (gps)", "speed (calc)"] {
        assert!(
            names.iter().any(|n| n == expected),
            "Expected channel '{}' in {:?}",
            expected,
            names
        );
    }

    // `device_update_rate` is exported once per sensor source
    for expected in [
        "device_update_rate (gps)",
        "device_update_rate (calc)",
        "device_update_rate (acc)",
        "device_update_rate (gyro)",
        "device_update_rate (magn)",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "Expected channel '{}' in {:?}",
            expected,
            names
        );
    }

    // No two channels may share a display name
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "channel names must be unique");
}

#[test]
fn test_racechrono_gps_channels_keep_plain_names() {
    // The GPS Track Map matches these names exactly — a source suffix on
    // either would silently disable the track map for RaceChrono logs.
    let contents = read_example_file(RACECHRONO_V3_EXCERPT);
    let log = RaceChrono
        .parse(&contents)
        .expect("Failed to parse RaceChrono example");

    let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    assert!(names.iter().any(|n| n == "latitude"));
    assert!(names.iter().any(|n| n == "longitude"));
}

#[test]
fn test_racechrono_units_normalized() {
    let contents = read_example_file(RACECHRONO_V3_EXCERPT);
    let log = RaceChrono
        .parse(&contents)
        .expect("Failed to parse RaceChrono example");

    let unit_of = |name: &str| {
        log.channels
            .iter()
            .find(|c| c.name() == name)
            .unwrap_or_else(|| panic!("Missing channel '{}'", name))
            .unit()
            .to_string()
    };

    assert_eq!(unit_of("latitude"), "deg");
    assert_eq!(unit_of("speed (gps)"), "m/s");
    assert_eq!(unit_of("rpm"), "rpm");
    // RaceChrono's CSV export writes '.C' where the app shows °C
    assert_eq!(unit_of("engine_oil_temp"), "°C");
}

#[test]
fn test_racechrono_values_align_with_channels() {
    let contents = read_example_file(RACECHRONO_V3_EXCERPT);
    let log = RaceChrono
        .parse(&contents)
        .expect("Failed to parse RaceChrono example");

    let index_of = |name: &str| {
        log.channels
            .iter()
            .position(|c| c.name() == name)
            .unwrap_or_else(|| panic!("Missing channel '{}'", name))
    };

    // First data row: parked in the pit lane before the session starts
    assert_eq!(log.data[0][index_of("fragment_id")].as_f64(), 0.0);
    assert_eq!(log.data[0][index_of("lap_number")].as_f64(), 0.0);
    assert!((log.data[0][index_of("latitude")].as_f64() - 44.831825).abs() < 1e-6);
    assert!((log.data[0][index_of("longitude")].as_f64() - 11.224415).abs() < 1e-6);
    assert_eq!(log.data[0][index_of("speed (gps)")].as_f64(), 0.0);

    // Row 200 is the first spliced mid-session record: fragment 2, lap 29,
    // on track at 6420 rpm.
    assert_eq!(log.data[200][index_of("fragment_id")].as_f64(), 2.0);
    assert_eq!(log.data[200][index_of("lap_number")].as_f64(), 29.0);
    assert_eq!(log.data[200][index_of("rpm")].as_f64(), 6420.0);
    assert!((log.data[200][index_of("speed (gps)")].as_f64() - 7.26131).abs() < 1e-6);
}

#[test]
fn test_racechrono_not_confused_with_other_formats() {
    let contents = read_example_file(RACECHRONO_V3_EXCERPT);

    assert!(
        !Mhd::detect(&contents),
        "RaceChrono should not be detected as MHD"
    );
    assert!(
        !EcuMaster::detect(&contents),
        "RaceChrono should not be detected as ECUMaster"
    );
    assert!(
        !RomRaider::detect(&contents),
        "RaceChrono should not be detected as RomRaider"
    );

    // And the reverse: other formats keep their identity
    assert!(!RaceChrono::detect("Time,RPM,Load\n0,1000,50\n"));
    assert!(!RaceChrono::detect("%DataLog%\nDataLogVersion : 1.1\n"));
    assert!(!RaceChrono::detect("TIME;engine/rpm\n0.0;1000\n"));
}
