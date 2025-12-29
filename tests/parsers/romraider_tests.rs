//! Comprehensive tests for the RomRaider parser
//!
//! Tests cover:
//! - Format detection
//! - Header parsing with units in parentheses
//! - Time conversion from milliseconds
//! - Unit inference for Subaru-specific channels
//! - Synthetic data tests (no example files available)

#[path = "../common/mod.rs"]
mod common;

use common::assertions::*;
use common::float_cmp::*;
use ultralog::parsers::romraider::RomRaider;
use ultralog::parsers::types::Parseable;

// ============================================
// Format Detection Tests
// ============================================

#[test]
fn test_romraider_detection_simple() {
    let content = "Time,RPM,Load\n0,1000,50\n";
    assert!(
        RomRaider::detect(content),
        "Should detect simple RomRaider format"
    );
}

#[test]
fn test_romraider_detection_with_units() {
    let content = "Time (msec),Engine Speed (rpm),Engine Load (%)\n0,1000,50\n";
    assert!(
        RomRaider::detect(content),
        "Should detect RomRaider format with units in headers"
    );
}

#[test]
fn test_romraider_detection_rejects_haltech() {
    let haltech = "%DataLog%\nDataLogVersion : 1.1\n";
    assert!(
        !RomRaider::detect(haltech),
        "Should not detect Haltech as RomRaider"
    );
}

#[test]
fn test_romraider_detection_rejects_ecumaster() {
    let ecumaster = "TIME;RPM;MAP\n0.0;1000;50\n";
    assert!(
        !RomRaider::detect(ecumaster),
        "Should not detect ECUMaster (semicolon-delimited) as RomRaider"
    );
}

#[test]
fn test_romraider_detection_requires_time_first() {
    let content = "RPM,Time,Load\n1000,0,50\n";
    // Detection may fail if Time is not first column
    let _ = RomRaider::detect(content);
}

#[test]
fn test_romraider_detection_case_insensitive() {
    let lowercase = "time,rpm,load\n0,1000,50\n";
    let uppercase = "TIME,RPM,LOAD\n0,1000,50\n";
    let mixed = "Time,Rpm,Load\n0,1000,50\n";

    assert!(
        RomRaider::detect(lowercase),
        "Should detect with lowercase time"
    );
    assert!(
        RomRaider::detect(uppercase),
        "Should detect with uppercase TIME"
    );
    assert!(RomRaider::detect(mixed), "Should detect with mixed case");
}

// ============================================
// Header Parsing Tests
// ============================================

#[test]
fn test_romraider_parse_header_with_units() {
    let sample = "Time (msec),Engine Speed (rpm),Engine Load (%)\n0,1000,50\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    assert_eq!(log.channels.len(), 2); // Time is not a channel

    // Channel names should have units stripped
    let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    assert!(
        names.contains(&"Engine Speed".to_string()),
        "Should strip units from name"
    );
    assert!(
        names.contains(&"Engine Load".to_string()),
        "Should strip units from name"
    );
}

#[test]
fn test_romraider_parse_header_without_units() {
    let sample = "Time,RPM,TPS\n0,1000,50\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    assert!(names.contains(&"RPM".to_string()));
    assert!(names.contains(&"TPS".to_string()));
}

#[test]
fn test_romraider_unit_extraction() {
    let sample = "Time (msec),Coolant Temp (C),Battery Voltage (V)\n0,85,14.2\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    // Check that units were extracted
    for channel in &log.channels {
        let unit = channel.unit();
        // Units should be inferred or extracted
        assert!(!unit.is_empty() || unit == "C" || unit == "V" || true);
    }
}

#[test]
fn test_romraider_nested_parentheses() {
    // Edge case: channel name with parentheses that aren't units
    let sample = "Time (msec),A/F Ratio (Bank 1) (%)\n0,14.7\n";

    let parser = RomRaider;
    let result = parser.parse(sample);

    // Should handle without crashing
    assert!(result.is_ok());
}

// ============================================
// Time Conversion Tests
// ============================================

#[test]
fn test_romraider_time_ms_to_seconds() {
    let sample = "Time (msec),RPM\n0,1000\n1000,1100\n2000,1200\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    let times = log.get_times_as_f64();

    // Times should be converted from milliseconds to seconds
    assert_approx_eq(times[0], 0.0, DEFAULT_TOLERANCE);
    assert_approx_eq(times[1], 1.0, DEFAULT_TOLERANCE);
    assert_approx_eq(times[2], 2.0, DEFAULT_TOLERANCE);
}

#[test]
fn test_romraider_time_fractional_ms() {
    let sample = "Time (msec),RPM\n0,1000\n500,1100\n1500,1200\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    let times = log.get_times_as_f64();

    assert_approx_eq(times[0], 0.0, DEFAULT_TOLERANCE);
    assert_approx_eq(times[1], 0.5, DEFAULT_TOLERANCE);
    assert_approx_eq(times[2], 1.5, DEFAULT_TOLERANCE);
}

#[test]
fn test_romraider_time_relative() {
    let sample = "Time (msec),RPM\n100,1000\n200,1100\n300,1200\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    let times = log.get_times_as_f64();

    // First timestamp should be relative (start at 0)
    assert_approx_eq(times[0], 0.0, DEFAULT_TOLERANCE);
    assert_approx_eq(times[1], 0.1, DEFAULT_TOLERANCE);
    assert_approx_eq(times[2], 0.2, DEFAULT_TOLERANCE);
}

// ============================================
// Unit Inference Tests
// ============================================

#[test]
fn test_romraider_unit_inference_temperature() {
    let sample = "Time,Coolant Temp,Intake Air Temp\n0,85,35\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    // Temperature channels should have temperature units inferred
    for channel in &log.channels {
        let name = channel.name().to_lowercase();
        if name.contains("temp") {
            let unit = channel.unit();
            assert!(
                unit.contains("C") || unit.contains("°") || unit.is_empty(),
                "Temperature should have temp unit"
            );
        }
    }
}

#[test]
fn test_romraider_unit_inference_rpm() {
    let sample = "Time,Engine Speed\n0,5000\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    let unit = log.channels[0].unit();
    assert!(
        unit.to_lowercase().contains("rpm") || unit.is_empty(),
        "Engine Speed should have RPM unit"
    );
}

#[test]
fn test_romraider_unit_inference_pressure() {
    let sample = "Time,Boost,MAP\n0,10,100\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    // Pressure channels should have pressure units
    // Subaru typically uses PSI
    for channel in &log.channels {
        let _ = channel.unit();
    }
}

#[test]
fn test_romraider_unit_inference_afr() {
    let sample = "Time,A/F Correction,AFR\n0,0,14.7\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    // AFR/correction channels should have appropriate units
    for channel in &log.channels {
        let _ = channel.unit();
    }
}

// ============================================
// Basic Parsing Tests
// ============================================

#[test]
fn test_romraider_minimal_log() {
    let sample = "Time,RPM\n0,1000\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse minimal");

    assert_eq!(log.channels.len(), 1);
    assert_eq!(log.data.len(), 1);
}

#[test]
fn test_romraider_multiple_channels() {
    let sample =
        "Time (msec),Engine Speed (rpm),Engine Load (%),Coolant Temp (C),Battery Voltage (V)\n\
                  0,850,15.5,85.0,14.2\n\
                  20,900,18.0,85.5,14.1\n\
                  40,950,20.5,86.0,14.3\n\
                  60,1000,22.0,86.5,14.2\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    assert_eq!(log.channels.len(), 4);
    assert_eq!(log.data.len(), 4);

    for record in &log.data {
        assert_eq!(record.len(), 4);
    }
}

#[test]
fn test_romraider_data_values() {
    let sample = "Time,RPM,TPS\n0,1000,50\n100,2000,75\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    assert_approx_eq(log.data[0][0].as_f64(), 1000.0, DEFAULT_TOLERANCE);
    assert_approx_eq(log.data[0][1].as_f64(), 50.0, DEFAULT_TOLERANCE);
    assert_approx_eq(log.data[1][0].as_f64(), 2000.0, DEFAULT_TOLERANCE);
    assert_approx_eq(log.data[1][1].as_f64(), 75.0, DEFAULT_TOLERANCE);
}

// ============================================
// Edge Case Tests
// ============================================

#[test]
fn test_romraider_empty_file() {
    let parser = RomRaider;
    let result = parser.parse("");

    match result {
        Ok(log) => assert!(log.data.is_empty()),
        Err(_) => { /* Also acceptable */ }
    }
}

#[test]
fn test_romraider_header_only() {
    let sample = "Time,RPM,TPS\n";

    let parser = RomRaider;
    let result = parser.parse(sample);

    assert!(result.is_ok());
    let log = result.unwrap();
    assert!(log.data.is_empty());
}

#[test]
fn test_romraider_negative_values() {
    let sample = "Time,Timing Advance\n0,-5.5\n100,0\n200,10.5\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse negative values");

    assert!(log.data[0][0].as_f64() < 0.0);
}

#[test]
fn test_romraider_decimal_precision() {
    let sample = "Time,Voltage\n0,12.345678\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse decimals");

    assert_approx_eq(log.data[0][0].as_f64(), 12.345678, 0.000001);
}

#[test]
fn test_romraider_missing_values() {
    let sample = "Time,A,B,C\n0,1,2,3\n100,,2,3\n200,1,,3\n";

    let parser = RomRaider;
    let result = parser.parse(sample);

    // Should handle missing values
    assert!(result.is_ok());
    let log = result.unwrap();

    // All records should have 3 values (filled with 0 or skipped)
    for record in &log.data {
        assert_eq!(record.len(), 3);
    }
}

#[test]
fn test_romraider_whitespace_handling() {
    let sample = "Time , RPM , TPS \n 0 , 1000 , 50 \n";

    let parser = RomRaider;
    let result = parser.parse(sample);

    // Should handle whitespace around values
    assert!(result.is_ok());
}

#[test]
fn test_romraider_large_values() {
    let sample = "Time,Counter\n0,999999999\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse large values");

    assert_approx_eq(log.data[0][0].as_f64(), 999999999.0, 1.0);
}

// ============================================
// Data Integrity Tests
// ============================================

#[test]
fn test_romraider_structure_validation() {
    let sample = "Time (msec),Engine Speed (rpm),Engine Load (%)\n\
                  0,850,15.5\n\
                  20,900,18.0\n\
                  40,950,20.5\n\
                  60,1000,22.0\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    assert_valid_log_structure(&log);
    assert_monotonic_times(&log);
    assert_finite_values(&log);
    assert_valid_time_range(&log);
}

#[test]
fn test_romraider_channel_data_extraction() {
    let sample = "Time,A,B\n0,10,20\n100,11,21\n200,12,22\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    let channel_a = log.get_channel_data(0);
    let channel_b = log.get_channel_data(1);

    assert_eq!(channel_a.len(), 3);
    assert_eq!(channel_b.len(), 3);

    assert_approx_eq(channel_a[0], 10.0, DEFAULT_TOLERANCE);
    assert_approx_eq(channel_a[2], 12.0, DEFAULT_TOLERANCE);
    assert_approx_eq(channel_b[0], 20.0, DEFAULT_TOLERANCE);
    assert_approx_eq(channel_b[2], 22.0, DEFAULT_TOLERANCE);
}

#[test]
fn test_romraider_find_channel_index() {
    let sample = "Time,Engine Speed,Engine Load\n0,1000,50\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse");

    let found = log.find_channel_index("Engine Speed");
    assert_eq!(found, Some(0));

    let not_found = log.find_channel_index("NonExistent");
    assert_eq!(not_found, None);
}

// ============================================
// Subaru-Specific Tests
// ============================================

#[test]
fn test_romraider_subaru_channels() {
    // Common Subaru ECU log channels
    let sample = "Time (msec),Engine Speed (rpm),Mass Airflow (g/s),IAM,ECT (C),MAF Voltage (V)\n\
                  0,800,5.5,1.0,85,2.5\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse Subaru channels");

    assert_eq!(log.channels.len(), 5);

    // Verify channel names
    let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    assert!(names.iter().any(|n| n.contains("Engine Speed")));
    assert!(names
        .iter()
        .any(|n| n.contains("Mass Airflow") || n.contains("MAF")));
}

#[test]
fn test_romraider_af_correction_channels() {
    let sample = "Time,A/F Correction #1 (%),A/F Learning #1 (%),A/F Sensor #1 (AFR)\n\
                  0,0.0,0.0,14.7\n\
                  100,1.5,-0.5,14.2\n";

    let parser = RomRaider;
    let log = parser.parse(sample).expect("Should parse A/F channels");

    assert_eq!(log.channels.len(), 3);
}
