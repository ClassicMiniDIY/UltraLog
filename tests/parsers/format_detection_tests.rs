//! Cross-format detection tests
//!
//! These tests verify that format detection is mutually exclusive
//! and correctly identifies each ECU format.

#[path = "../common/mod.rs"]
mod common;

use common::example_files::*;
use common::{example_file_exists, read_example_binary, read_example_file};
use ultralog::parsers::aim::Aim;
use ultralog::parsers::ecumaster::EcuMaster;
use ultralog::parsers::link::Link;
use ultralog::parsers::romraider::RomRaider;
use ultralog::parsers::speeduino::Speeduino;

// ============================================
// Format Marker Tests
// ============================================

#[test]
fn test_haltech_marker() {
    let haltech = "%DataLog%\nDataLogVersion : 1.1\n";
    assert!(
        haltech.starts_with("%DataLog%"),
        "Haltech should start with %DataLog%"
    );
}

#[test]
fn test_ecumaster_semicolon_detection() {
    let ecumaster = "TIME;engine/rpm;sensors/tps1\n0.0;1000;50\n";
    assert!(EcuMaster::detect(ecumaster), "Should detect ECUMaster");
}

#[test]
fn test_ecumaster_tab_detection() {
    let ecumaster = "TIME\tengine/rpm\n0.0\t1000\n";
    assert!(
        EcuMaster::detect(ecumaster),
        "Should detect tab-delimited ECUMaster"
    );
}

#[test]
fn test_romraider_detection() {
    let romraider = "Time,RPM,Load\n0,1000,50\n";
    assert!(RomRaider::detect(romraider), "Should detect RomRaider");
}

#[test]
fn test_speeduino_detection() {
    let mlg = b"MLVLG\x00\x00\x01";
    assert!(Speeduino::detect(mlg), "Should detect MLG format");
}

#[test]
fn test_aim_detection() {
    let xrk = b"<hCNF\x00\x3c\xa5\x00\x00";
    assert!(Aim::detect(xrk), "Should detect AiM XRK format");
}

#[test]
fn test_link_detection() {
    let mut llg = vec![0xd7, 0x00, 0x00, 0x00];
    llg.extend_from_slice(b"lf3");
    llg.extend_from_slice(&[0; 208]);
    assert!(Link::detect(&llg), "Should detect Link LLG format");
}

// ============================================
// Mutual Exclusion Tests
// ============================================

#[test]
fn test_haltech_not_detected_as_others() {
    let haltech = "%DataLog%\nDataLogVersion : 1.1\n";

    assert!(
        !EcuMaster::detect(haltech),
        "Haltech should not be ECUMaster"
    );
    assert!(
        !RomRaider::detect(haltech),
        "Haltech should not be RomRaider"
    );
    assert!(
        !Speeduino::detect(haltech.as_bytes()),
        "Haltech should not be MLG"
    );
    assert!(
        !Aim::detect(haltech.as_bytes()),
        "Haltech should not be AiM"
    );
    assert!(
        !Link::detect(haltech.as_bytes()),
        "Haltech should not be Link"
    );
}

#[test]
fn test_ecumaster_not_detected_as_others() {
    let ecumaster = "TIME;engine/rpm\n0.0;1000\n";

    assert!(
        !ecumaster.starts_with("%DataLog%"),
        "ECUMaster should not be Haltech"
    );
    assert!(
        !RomRaider::detect(ecumaster),
        "ECUMaster should not be RomRaider"
    );
    assert!(
        !Speeduino::detect(ecumaster.as_bytes()),
        "ECUMaster should not be MLG"
    );
    assert!(
        !Aim::detect(ecumaster.as_bytes()),
        "ECUMaster should not be AiM"
    );
    assert!(
        !Link::detect(ecumaster.as_bytes()),
        "ECUMaster should not be Link"
    );
}

#[test]
fn test_romraider_not_detected_as_others() {
    let romraider = "Time,RPM,Load\n0,1000,50\n";

    assert!(
        !romraider.starts_with("%DataLog%"),
        "RomRaider should not be Haltech"
    );
    assert!(
        !EcuMaster::detect(romraider),
        "RomRaider should not be ECUMaster"
    );
    assert!(
        !Speeduino::detect(romraider.as_bytes()),
        "RomRaider should not be MLG"
    );
    assert!(
        !Aim::detect(romraider.as_bytes()),
        "RomRaider should not be AiM"
    );
    assert!(
        !Link::detect(romraider.as_bytes()),
        "RomRaider should not be Link"
    );
}

#[test]
fn test_mlg_not_detected_as_others() {
    let mlg = b"MLVLG\x00\x00\x01";

    assert!(!Aim::detect(mlg), "MLG should not be AiM");
    assert!(!Link::detect(mlg), "MLG should not be Link");
}

#[test]
fn test_aim_not_detected_as_others() {
    let xrk = b"<hCNF\x00\x3c\xa5\x00\x00";

    assert!(!Speeduino::detect(xrk), "AiM should not be MLG");
    assert!(!Link::detect(xrk), "AiM should not be Link");
}

#[test]
fn test_link_not_detected_as_others() {
    let mut llg = vec![0xd7, 0x00, 0x00, 0x00];
    llg.extend_from_slice(b"lf3");
    llg.extend_from_slice(&[0; 208]);

    assert!(!Speeduino::detect(&llg), "Link should not be MLG");
    assert!(!Aim::detect(&llg), "Link should not be AiM");
}

// ============================================
// Real File Detection Tests
// ============================================

#[test]
fn test_detect_haltech_example_file() {
    if !example_file_exists(HALTECH_SMALL) {
        eprintln!("Skipping: {} not found", HALTECH_SMALL);
        return;
    }

    let content = read_example_file(HALTECH_SMALL);

    assert!(content.starts_with("%DataLog%"), "Should detect as Haltech");
    assert!(
        !EcuMaster::detect(&content),
        "Should not detect as ECUMaster"
    );
    assert!(
        !RomRaider::detect(&content),
        "Should not detect as RomRaider"
    );
}

#[test]
fn test_detect_ecumaster_example_file() {
    if !example_file_exists(ECUMASTER_STANDARD) {
        eprintln!("Skipping: {} not found", ECUMASTER_STANDARD);
        return;
    }

    let content = read_example_file(ECUMASTER_STANDARD);

    assert!(EcuMaster::detect(&content), "Should detect as ECUMaster");
    assert!(
        !content.starts_with("%DataLog%"),
        "Should not detect as Haltech"
    );
    assert!(
        !RomRaider::detect(&content),
        "Should not detect as RomRaider"
    );
}

#[test]
fn test_detect_speeduino_example_file() {
    if !example_file_exists(SPEEDUINO_MLG) {
        eprintln!("Skipping: {} not found", SPEEDUINO_MLG);
        return;
    }

    let data = read_example_binary(SPEEDUINO_MLG);

    assert!(Speeduino::detect(&data), "Should detect as Speeduino MLG");
    assert!(!Aim::detect(&data), "Should not detect as AiM");
    assert!(!Link::detect(&data), "Should not detect as Link");
}

#[test]
fn test_detect_aim_example_file() {
    if !example_file_exists(AIM_GENERIC) {
        eprintln!("Skipping: {} not found", AIM_GENERIC);
        return;
    }

    let data = read_example_binary(AIM_GENERIC);

    assert!(Aim::detect(&data), "Should detect as AiM");
    assert!(!Speeduino::detect(&data), "Should not detect as Speeduino");
    assert!(!Link::detect(&data), "Should not detect as Link");
}

#[test]
fn test_detect_link_example_file() {
    if !example_file_exists(LINK_STANDARD) {
        eprintln!("Skipping: {} not found", LINK_STANDARD);
        return;
    }

    let data = read_example_binary(LINK_STANDARD);

    assert!(Link::detect(&data), "Should detect as Link");
    assert!(!Speeduino::detect(&data), "Should not detect as Speeduino");
    assert!(!Aim::detect(&data), "Should not detect as AiM");
}

// ============================================
// Edge Cases
// ============================================

#[test]
fn test_empty_data_detection() {
    assert!(!EcuMaster::detect(""), "Empty should not be ECUMaster");
    assert!(!RomRaider::detect(""), "Empty should not be RomRaider");
    assert!(!Speeduino::detect(b""), "Empty should not be MLG");
    assert!(!Aim::detect(b""), "Empty should not be AiM");
    assert!(!Link::detect(b""), "Empty should not be Link");
}

#[test]
fn test_random_data_detection() {
    let random = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09";

    assert!(!Speeduino::detect(random), "Random should not be MLG");
    assert!(!Aim::detect(random), "Random should not be AiM");
    assert!(!Link::detect(random), "Random should not be Link");
}

#[test]
fn test_whitespace_detection() {
    let whitespace = "   \n\t\r\n   ";

    assert!(
        !EcuMaster::detect(whitespace),
        "Whitespace should not be ECUMaster"
    );
    assert!(
        !RomRaider::detect(whitespace),
        "Whitespace should not be RomRaider"
    );
}

#[test]
fn test_partial_header_detection() {
    // Partial headers should not match
    assert!(!Speeduino::detect(b"MLV"), "Partial MLVLG should not match");
    assert!(!Aim::detect(b"<hCN"), "Partial <hCNF should not match");

    let partial_link = vec![0xd7, 0x00, 0x00, 0x00, b'l', b'f'];
    assert!(!Link::detect(&partial_link), "Partial lf3 should not match");
}

// ============================================
// Comprehensive Detection Matrix
// ============================================

#[test]
fn test_all_formats_detection_matrix() {
    // Create sample data for each format
    let haltech = "%DataLog%\nDataLogVersion : 1.1\n";
    let ecumaster = "TIME;rpm\n0.0;1000\n";
    let romraider = "Time,RPM\n0,1000\n";
    let mlg = b"MLVLG\x00\x00\x01";
    let xrk = b"<hCNF\x00\x3c\xa5";
    let mut llg = vec![0xd7, 0x00, 0x00, 0x00];
    llg.extend_from_slice(b"lf3");
    llg.extend_from_slice(&[0; 208]);

    // Haltech detection
    assert!(haltech.starts_with("%DataLog%"), "Haltech self-detect");

    // ECUMaster detection
    assert!(EcuMaster::detect(ecumaster), "ECUMaster self-detect");
    assert!(!EcuMaster::detect(haltech), "ECUMaster rejects Haltech");
    assert!(!EcuMaster::detect(romraider), "ECUMaster rejects RomRaider");

    // RomRaider detection
    assert!(RomRaider::detect(romraider), "RomRaider self-detect");
    assert!(!RomRaider::detect(haltech), "RomRaider rejects Haltech");
    assert!(!RomRaider::detect(ecumaster), "RomRaider rejects ECUMaster");

    // MLG detection
    assert!(Speeduino::detect(mlg), "MLG self-detect");
    assert!(!Speeduino::detect(xrk), "MLG rejects XRK");
    assert!(!Speeduino::detect(&llg), "MLG rejects LLG");

    // XRK detection
    assert!(Aim::detect(xrk), "XRK self-detect");
    assert!(!Aim::detect(mlg), "XRK rejects MLG");
    assert!(!Aim::detect(&llg), "XRK rejects LLG");

    // LLG detection
    assert!(Link::detect(&llg), "LLG self-detect");
    assert!(!Link::detect(mlg), "LLG rejects MLG");
    assert!(!Link::detect(xrk), "LLG rejects XRK");
}

// ============================================
// Binary vs Text Format Tests
// ============================================

#[test]
fn test_binary_formats_reject_text() {
    let text_samples = [
        "TIME;rpm\n0.0;1000\n",
        "Time,RPM\n0,1000\n",
        "%DataLog%\n",
        "plain text data",
    ];

    for sample in &text_samples {
        assert!(
            !Speeduino::detect(sample.as_bytes()),
            "MLG should reject text: {}",
            sample
        );
        assert!(
            !Aim::detect(sample.as_bytes()),
            "AiM should reject text: {}",
            sample
        );
        assert!(
            !Link::detect(sample.as_bytes()),
            "Link should reject text: {}",
            sample
        );
    }
}

#[test]
fn test_text_formats_handle_binary() {
    let binary_samples: &[&[u8]] = &[
        b"MLVLG\x00\x00\x01",
        b"<hCNF\x00\x3c\xa5",
        b"\x00\x01\x02\x03\x04",
    ];

    for sample in binary_samples {
        // Text format detectors should handle binary gracefully
        let as_str = String::from_utf8_lossy(sample);
        let _ = EcuMaster::detect(&as_str);
        let _ = RomRaider::detect(&as_str);
    }
}
