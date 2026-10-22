//! Torque Pro trackLog parser integration tests
//!
//! Covers detection against the other text formats, the issue #97 header-only
//! file, and a synthetic two-session log built on the issue #97 header row.

#[path = "../common/mod.rs"]
mod common;

use common::example_files::*;
use common::read_example_file;
use ultralog::parsers::torque::Torque;
use ultralog::parsers::types::Meta;
use ultralog::parsers::{BlueDriver, EcuMaster, MegaSquirt, Msl, Parseable, RaceChrono, RomRaider};

#[test]
fn test_detect_torque_example_files() {
    for path in [TORQUE_SYNTHETIC_TWO_SESSIONS, TORQUE_HEADER_ONLY_ISSUE_97] {
        let contents = read_example_file(path);
        assert!(Torque::detect(&contents), "Should detect {path}");
    }
}

#[test]
fn test_torque_not_claimed_by_earlier_or_catch_all_parsers() {
    let contents = read_example_file(TORQUE_SYNTHETIC_TWO_SESSIONS);
    assert!(!RaceChrono::detect(&contents));
    assert!(!MegaSquirt::detect(&contents));
    assert!(!Msl::detect(&contents));
    assert!(!EcuMaster::detect(&contents));
    assert!(!BlueDriver::detect(&contents));
    assert!(!RomRaider::detect(&contents));
}

#[test]
fn test_header_only_issue_97_file_reports_no_data() {
    let contents = read_example_file(TORQUE_HEADER_ONLY_ISSUE_97);
    let err = Torque.parse(&contents).unwrap_err();
    assert!(err.to_string().contains("no data rows"), "{err}");
}

#[test]
fn test_parse_synthetic_two_session_log() {
    let contents = read_example_file(TORQUE_SYNTHETIC_TWO_SESSIONS);
    let log = Torque
        .parse(&contents)
        .expect("Failed to parse Torque example");

    // 160 rows, a repeated header, then 80 rows.
    assert_eq!(log.data.len(), 240);
    assert_eq!(log.times.len(), log.data.len());
    for row in &log.data {
        assert_eq!(row.len(), log.channels.len());
    }

    // 10 fixed GPS/G columns + the 17 PIDs the fixture fills. The other
    // ~200 PIDs in the header are `-` throughout and are dropped.
    assert_eq!(log.channels.len(), 27);
    let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    assert!(names.contains(&"Engine RPM".to_string()));
    assert!(!names.iter().any(|n| n.starts_with("DPF")));

    // Track Map matches these exact names (find_gps_channels).
    assert_eq!(log.channels[0].name(), "Longitude");
    assert_eq!(log.channels[1].name(), "Latitude");

    let rpm = log.find_channel_index("Engine RPM").unwrap();
    assert_eq!(log.channels[rpm].unit(), "rpm");
    let ect = log
        .find_channel_index("Engine Coolant Temperature")
        .unwrap();
    assert_eq!(log.channels[ect].unit(), "°C");

    // Device Time base, millisecond steps, the 30 s restart gap preserved.
    assert_eq!(log.times[0], 0.0);
    assert!(log.times.windows(2).all(|w| w[0] < w[1]));
    let gap = log.times[160] - log.times[159];
    assert!((30.9..=31.1).contains(&gap), "restart gap {gap}");

    if let Meta::Torque(meta) = &log.meta {
        assert_eq!(meta.sessions, 2);
        assert_eq!(meta.start_time, "22-Oct-2026 16:23:12.345");
    } else {
        panic!("Expected Torque metadata");
    }
}
