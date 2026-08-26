//! Comprehensive tests for the TunerStudio MSL (legacy ASCII) parser.
//!
//! Tests cover:
//! - Format detection, including the header/units row pair that separates
//!   MSL from every other tab-delimited `Time`-first export
//! - Mutual exclusion with the parsers that share a leading `Time` column
//!   (ECUMaster, RomRaider, Motorsport Electronics) and with MegaSquirt,
//!   which is the same vendor but the comma-delimited CSV export
//! - Real file parsing using the bundled RealDash/Speeduino example log
//! - Data integrity: monotonic times, finite values, GPS column names

#[path = "../common/mod.rs"]
mod common;

use common::assertions::*;
use common::example_files::*;
use common::float_cmp::*;
use common::read_example_file;
use ultralog::parsers::ecumaster::EcuMaster;
use ultralog::parsers::megasquirt::MegaSquirt;
use ultralog::parsers::motorsport_electronics::MotorsportElectronics;
use ultralog::parsers::msl::Msl;
use ultralog::parsers::romraider::RomRaider;
use ultralog::parsers::types::Parseable;
use ultralog::parsers::woolich::Woolich;

const SYNTHETIC: &str = "Time\tSecL\tRPM\tMAP\tAFR\n\
                         sec\ts\tRPM\tkPa\tAFR\n\
                         0.0\t202.0\t965.0\t33.0\t13.7\n\
                         0.5\t202.0\t970.0\t34.0\t13.8\n\
                         1.0\t203.0\t980.0\t35.0\t13.6\n";

// ============================================
// Detection
// ============================================

#[test]
fn detects_synthetic_msl() {
    assert!(Msl::detect(SYNTHETIC));
}

#[test]
fn detects_real_realdash_export() {
    let contents = read_example_file(MSL_REALDASH_EXCERPT);
    assert!(Msl::detect(&contents), "Should detect the RealDash MSL log");
}

#[test]
fn rejects_ecumaster_tab_export() {
    // Same delimiter and a leading time column, but no units row.
    let ecumaster = "TIME\tengine/rpm\n0.0\t1000\n";
    assert!(!Msl::detect(ecumaster));
}

#[test]
fn rejects_romraider_csv() {
    assert!(!Msl::detect("Time (msec),RPM,Load\n0,1000,50\n"));
}

#[test]
fn rejects_megasquirt_csv() {
    let megasquirt = "Tune Datalog export of DL\n\
                      Tune file: test.bin\n\
                      Total Frames: 1\n\
                      Duration :00:00:01.000\n\
                      Frame,Duration,RPM\n\
                      0,0.0,1800\n";
    assert!(!Msl::detect(megasquirt));
}

#[test]
fn rejects_haltech_marker_file() {
    assert!(!Msl::detect("%DataLog%\nDataLogVersion : 1.1\n"));
}

// ============================================
// Mutual exclusion — no other parser may claim an MSL log
// ============================================

#[test]
fn other_text_parsers_reject_msl() {
    let contents = read_example_file(MSL_REALDASH_EXCERPT);

    assert!(
        !EcuMaster::detect(&contents),
        "ECUMaster must not claim an MSL log"
    );
    assert!(
        !RomRaider::detect(&contents),
        "RomRaider must not claim an MSL log"
    );
    assert!(
        !MotorsportElectronics::detect(&contents),
        "Motorsport Electronics must not claim an MSL log"
    );
    assert!(
        !MegaSquirt::detect(&contents),
        "MegaSquirt must not claim an MSL log"
    );
    assert!(
        !Woolich::detect(&contents),
        "Woolich must not claim an MSL log"
    );
}

// ============================================
// Parsing — synthetic
// ============================================

#[test]
fn parses_units_from_the_units_row() {
    let log = Msl.parse(SYNTHETIC).expect("parses");

    assert_eq!(log.channels.len(), 4, "Time is the axis, not a channel");
    assert_eq!(log.channels[1].name(), "RPM");
    assert_eq!(log.channels[1].unit(), "RPM");
    assert_eq!(log.channels[2].name(), "MAP");
    assert_eq!(log.channels[2].unit(), "kPa");
    assert_eq!(log.channels[3].unit(), "AFR");
}

#[test]
fn time_column_is_seconds_not_milliseconds() {
    let log = Msl.parse(SYNTHETIC).expect("parses");

    assert_approx_eq(log.times[0], 0.0, DEFAULT_TOLERANCE);
    assert_approx_eq(log.times[2], 1.0, 1e-9);
}

// ============================================
// Parsing — real file
// ============================================

#[test]
fn parses_real_file_structure() {
    let contents = read_example_file(MSL_REALDASH_EXCERPT);
    let log = Msl.parse(&contents).expect("parses");

    assert_valid_log_structure(&log);
    assert_monotonic_times(&log);
    assert_finite_values(&log);
    assert_minimum_records(&log, 1000);
    assert_minimum_channels(&log, 20);
    assert_valid_time_range(&log);
}

#[test]
fn real_file_keeps_gps_column_names_for_the_track_map() {
    let contents = read_example_file(MSL_REALDASH_EXCERPT);
    let log = Msl.parse(&contents).expect("parses");

    let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    // The Track Map widget matches these names exactly (lowercased), so a
    // renamed or suffixed column would silently disable GPS mapping.
    assert!(
        names.iter().any(|n| n == "GPS Latitude"),
        "GPS Latitude column must keep its exported name"
    );
    assert!(
        names.iter().any(|n| n == "GPS Longitude"),
        "GPS Longitude column must keep its exported name"
    );
}

#[test]
fn real_file_gps_coordinates_are_plausible_decimal_degrees() {
    let contents = read_example_file(MSL_REALDASH_EXCERPT);
    let log = Msl.parse(&contents).expect("parses");

    let lat_idx = log
        .channels
        .iter()
        .position(|c| c.name() == "GPS Latitude")
        .expect("latitude channel");
    let lon_idx = log
        .channels
        .iter()
        .position(|c| c.name() == "GPS Longitude")
        .expect("longitude channel");

    for row in &log.data {
        let lat = row[lat_idx].as_f64();
        let lon = row[lon_idx].as_f64();
        assert!(
            (-90.0..=90.0).contains(&lat),
            "latitude out of range: {lat}"
        );
        assert!(
            (-180.0..=180.0).contains(&lon),
            "longitude out of range: {lon}"
        );
    }
}

/// Every bundled example log of another format must be rejected. Adding a
/// parser to the dispatch chain is the moment a detector can start stealing
/// other formats' files, so this walks the whole `exampleLogs/` tree.
///
/// Only the head of each file is read: detection looks at the preamble, the
/// header/units pair and the first data row, and some fixtures are hundreds
/// of megabytes.
#[test]
fn does_not_claim_any_other_bundled_example_log() {
    use std::io::Read;

    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }

    let mut files = Vec::new();
    walk(std::path::Path::new("exampleLogs"), &mut files);
    assert!(!files.is_empty(), "expected bundled example logs");

    let mut checked = 0;
    for path in files {
        // Skip the MSL fixtures themselves and non-log files.
        if path.components().any(|c| c.as_os_str() == "msl") {
            continue;
        }
        if path.file_name().is_some_and(|n| n == ".DS_Store") {
            continue;
        }

        let Ok(mut file) = std::fs::File::open(&path) else {
            continue;
        };
        let mut head = vec![0u8; 64 * 1024];
        let Ok(read) = file.read(&mut head) else {
            continue;
        };
        head.truncate(read);
        let Ok(text) = String::from_utf8(head) else {
            continue; // binary format, never reaches the text dispatch chain
        };

        checked += 1;
        assert!(
            !Msl::detect(&text),
            "MSL parser must not claim {}",
            path.display()
        );
    }

    assert!(checked > 0, "expected to check at least one text log");
}
