//! Comprehensive tests for unit conversion system
//!
//! Tests cover:
//! - All unit conversion functions
//! - Roundtrip accuracy
//! - Edge cases (very large/small values, negative values)
//! - Symbol generation

use ultralog::units::{
    AccelerationUnit, DistanceUnit, FlowUnit, FuelEconomyUnit, PressureUnit, SpeedUnit,
    TemperatureUnit, UnitPreferences, VolumeUnit,
};

// ============================================
// Temperature Conversion Tests
// ============================================

#[test]
fn test_temperature_celsius_identity() {
    let celsius = TemperatureUnit::Celsius.convert_from_kelvin(300.0);
    // 300K = 26.85°C
    assert!((celsius - 26.85).abs() < 0.01);
}

#[test]
fn test_temperature_kelvin_identity() {
    let kelvin = TemperatureUnit::Kelvin.convert_from_kelvin(300.0);
    assert_eq!(kelvin, 300.0);
}

#[test]
fn test_temperature_fahrenheit() {
    let fahrenheit = TemperatureUnit::Fahrenheit.convert_from_kelvin(300.0);
    // 300K = 80.33°F
    assert!((fahrenheit - 80.33).abs() < 0.01);
}

#[test]
fn test_temperature_freezing_point() {
    // 273.15K = 0°C = 32°F
    let celsius = TemperatureUnit::Celsius.convert_from_kelvin(273.15);
    let fahrenheit = TemperatureUnit::Fahrenheit.convert_from_kelvin(273.15);

    assert!(celsius.abs() < 0.01);
    assert!((fahrenheit - 32.0).abs() < 0.01);
}

#[test]
fn test_temperature_boiling_point() {
    // 373.15K = 100°C = 212°F
    let celsius = TemperatureUnit::Celsius.convert_from_kelvin(373.15);
    let fahrenheit = TemperatureUnit::Fahrenheit.convert_from_kelvin(373.15);

    assert!((celsius - 100.0).abs() < 0.01);
    assert!((fahrenheit - 212.0).abs() < 0.01);
}

#[test]
fn test_temperature_absolute_zero() {
    // 0K = -273.15°C = -459.67°F
    let celsius = TemperatureUnit::Celsius.convert_from_kelvin(0.0);
    let fahrenheit = TemperatureUnit::Fahrenheit.convert_from_kelvin(0.0);

    assert!((celsius - (-273.15)).abs() < 0.01);
    assert!((fahrenheit - (-459.67)).abs() < 0.01);
}

#[test]
fn test_temperature_symbols() {
    assert_eq!(TemperatureUnit::Celsius.symbol(), "°C");
    assert_eq!(TemperatureUnit::Fahrenheit.symbol(), "°F");
    assert_eq!(TemperatureUnit::Kelvin.symbol(), "K");
}

// ============================================
// Pressure Conversion Tests
// ============================================

#[test]
fn test_pressure_kpa_identity() {
    let kpa = PressureUnit::KPa.convert_from_kpa(101.325);
    assert_eq!(kpa, 101.325);
}

#[test]
fn test_pressure_psi() {
    let psi = PressureUnit::PSI.convert_from_kpa(101.325);
    // 101.325 kPa ≈ 14.7 PSI (1 atm)
    assert!((psi - 14.696).abs() < 0.01);
}

#[test]
fn test_pressure_bar() {
    let bar = PressureUnit::Bar.convert_from_kpa(100.0);
    // 100 kPa = 1 bar
    assert!((bar - 1.0).abs() < 0.001);
}

#[test]
fn test_pressure_zero() {
    assert_eq!(PressureUnit::KPa.convert_from_kpa(0.0), 0.0);
    assert_eq!(PressureUnit::PSI.convert_from_kpa(0.0), 0.0);
    assert_eq!(PressureUnit::Bar.convert_from_kpa(0.0), 0.0);
}

#[test]
fn test_pressure_symbols() {
    assert_eq!(PressureUnit::KPa.symbol(), "kPa");
    assert_eq!(PressureUnit::PSI.symbol(), "PSI");
    assert_eq!(PressureUnit::Bar.symbol(), "bar");
}

// ============================================
// Speed Conversion Tests
// ============================================

#[test]
fn test_speed_kmh_identity() {
    let kmh = SpeedUnit::KmH.convert_from_kmh(100.0);
    assert_eq!(kmh, 100.0);
}

#[test]
fn test_speed_mph() {
    let mph = SpeedUnit::Mph.convert_from_kmh(100.0);
    // 100 km/h ≈ 62.14 mph
    assert!((mph - 62.137).abs() < 0.01);
}

#[test]
fn test_speed_zero() {
    assert_eq!(SpeedUnit::KmH.convert_from_kmh(0.0), 0.0);
    assert_eq!(SpeedUnit::Mph.convert_from_kmh(0.0), 0.0);
}

#[test]
fn test_speed_symbols() {
    assert_eq!(SpeedUnit::KmH.symbol(), "km/h");
    assert_eq!(SpeedUnit::Mph.symbol(), "mph");
}

// ============================================
// Distance Conversion Tests
// ============================================

#[test]
fn test_distance_km_identity() {
    let km = DistanceUnit::Kilometers.convert_from_km(100.0);
    assert_eq!(km, 100.0);
}

#[test]
fn test_distance_miles() {
    let miles = DistanceUnit::Miles.convert_from_km(100.0);
    // 100 km ≈ 62.14 miles
    assert!((miles - 62.137).abs() < 0.01);
}

#[test]
fn test_distance_zero() {
    assert_eq!(DistanceUnit::Kilometers.convert_from_km(0.0), 0.0);
    assert_eq!(DistanceUnit::Miles.convert_from_km(0.0), 0.0);
}

#[test]
fn test_distance_symbols() {
    assert_eq!(DistanceUnit::Kilometers.symbol(), "km");
    assert_eq!(DistanceUnit::Miles.symbol(), "mi");
}

// ============================================
// Fuel Economy Conversion Tests
// ============================================

#[test]
fn test_fuel_economy_l100km_identity() {
    let l100km = FuelEconomyUnit::LPer100Km.convert_from_l_per_100km(8.0);
    assert_eq!(l100km, 8.0);
}

#[test]
fn test_fuel_economy_mpg() {
    let mpg = FuelEconomyUnit::Mpg.convert_from_l_per_100km(8.0);
    // 8 L/100km ≈ 29.4 mpg (US)
    assert!((mpg - 29.4).abs() < 0.1);
}

#[test]
fn test_fuel_economy_km_per_l() {
    let km_l = FuelEconomyUnit::KmPerL.convert_from_l_per_100km(8.0);
    // 8 L/100km = 12.5 km/L
    assert!((km_l - 12.5).abs() < 0.01);
}

#[test]
fn test_fuel_economy_zero_handling() {
    // Zero L/100km should return 0 for mpg (handled specially to avoid infinity)
    let mpg = FuelEconomyUnit::Mpg.convert_from_l_per_100km(0.0);
    assert_eq!(mpg, 0.0);
}

#[test]
fn test_fuel_economy_symbols() {
    assert_eq!(FuelEconomyUnit::LPer100Km.symbol(), "L/100km");
    assert_eq!(FuelEconomyUnit::Mpg.symbol(), "mpg");
    assert_eq!(FuelEconomyUnit::KmPerL.symbol(), "km/L");
}

// ============================================
// Volume Conversion Tests
// ============================================

#[test]
fn test_volume_liters_identity() {
    let liters = VolumeUnit::Liters.convert_from_liters(10.0);
    assert_eq!(liters, 10.0);
}

#[test]
fn test_volume_gallons() {
    let gallons = VolumeUnit::Gallons.convert_from_liters(10.0);
    // 10 L ≈ 2.64 US gallons
    assert!((gallons - 2.6417).abs() < 0.01);
}

#[test]
fn test_volume_zero() {
    assert_eq!(VolumeUnit::Liters.convert_from_liters(0.0), 0.0);
    assert_eq!(VolumeUnit::Gallons.convert_from_liters(0.0), 0.0);
}

#[test]
fn test_volume_symbols() {
    assert_eq!(VolumeUnit::Liters.symbol(), "L");
    assert_eq!(VolumeUnit::Gallons.symbol(), "gal");
}

// ============================================
// Flow Rate Conversion Tests
// ============================================

#[test]
fn test_flow_cc_min_identity() {
    let cc_min = FlowUnit::CcPerMin.convert_from_cc_per_min(500.0);
    assert_eq!(cc_min, 500.0);
}

#[test]
fn test_flow_lb_hr() {
    let lb_hr = FlowUnit::LbPerHr.convert_from_cc_per_min(500.0);
    // Conversion depends on fuel density assumption
    // Should be non-zero positive value
    assert!(lb_hr > 0.0);
}

#[test]
fn test_flow_zero() {
    assert_eq!(FlowUnit::CcPerMin.convert_from_cc_per_min(0.0), 0.0);
    assert_eq!(FlowUnit::LbPerHr.convert_from_cc_per_min(0.0), 0.0);
}

#[test]
fn test_flow_symbols() {
    assert_eq!(FlowUnit::CcPerMin.symbol(), "cc/min");
    assert_eq!(FlowUnit::LbPerHr.symbol(), "lb/hr");
}

// ============================================
// Acceleration Conversion Tests
// ============================================

#[test]
fn test_acceleration_m_s2_identity() {
    let m_s2 = AccelerationUnit::MPerS2.convert_from_m_per_s2(9.81);
    assert_eq!(m_s2, 9.81);
}

#[test]
fn test_acceleration_g() {
    let g = AccelerationUnit::G.convert_from_m_per_s2(9.80665);
    // 9.80665 m/s² = 1 g (exactly)
    assert!((g - 1.0).abs() < 0.0001);
}

#[test]
fn test_acceleration_zero() {
    assert_eq!(AccelerationUnit::MPerS2.convert_from_m_per_s2(0.0), 0.0);
    assert_eq!(AccelerationUnit::G.convert_from_m_per_s2(0.0), 0.0);
}

#[test]
fn test_acceleration_negative() {
    let g = AccelerationUnit::G.convert_from_m_per_s2(-9.80665);
    assert!((g - (-1.0)).abs() < 0.0001);
}

#[test]
fn test_acceleration_symbols() {
    assert_eq!(AccelerationUnit::MPerS2.symbol(), "m/s²");
    assert_eq!(AccelerationUnit::G.symbol(), "g");
}

// ============================================
// UnitPreferences Tests
// ============================================

#[test]
fn test_unit_preferences_default() {
    let prefs = UnitPreferences::default();

    // Default should be metric
    assert!(matches!(prefs.temperature, TemperatureUnit::Celsius));
    assert!(matches!(prefs.pressure, PressureUnit::KPa));
    assert!(matches!(prefs.speed, SpeedUnit::KmH));
}

#[test]
fn test_unit_preferences_convert_kelvin() {
    let mut prefs = UnitPreferences::default();
    prefs.temperature = TemperatureUnit::Fahrenheit;

    // Convert 300K to display unit
    let (value, symbol) = prefs.convert_value(300.0, "K");

    // Should convert from Kelvin to Fahrenheit
    assert!((value - 80.33).abs() < 0.1);
    assert_eq!(symbol, "°F");
}

#[test]
fn test_unit_preferences_convert_kpa() {
    let mut prefs = UnitPreferences::default();
    prefs.pressure = PressureUnit::PSI;

    let (value, symbol) = prefs.convert_value(101.325, "kPa");

    assert!((value - 14.696).abs() < 0.01);
    assert_eq!(symbol, "PSI");
}

#[test]
fn test_unit_preferences_convert_unknown() {
    let prefs = UnitPreferences::default();

    // Unknown units should pass through unchanged
    let (value, symbol) = prefs.convert_value(42.0, "unknown_unit");

    assert_eq!(value, 42.0);
    assert_eq!(symbol, "unknown_unit");
}

#[test]
fn test_unit_preferences_convert_rpm() {
    let prefs = UnitPreferences::default();

    // RPM should pass through unchanged
    let (value, symbol) = prefs.convert_value(5000.0, "RPM");

    assert_eq!(value, 5000.0);
    assert_eq!(symbol, "RPM");
}

// ============================================
// Edge Cases
// ============================================

#[test]
fn test_very_large_values() {
    let large = 1e10;

    // Should handle large values without overflow
    let celsius = TemperatureUnit::Celsius.convert_from_kelvin(large);
    assert!(celsius.is_finite());

    let psi = PressureUnit::PSI.convert_from_kpa(large);
    assert!(psi.is_finite());

    let mph = SpeedUnit::Mph.convert_from_kmh(large);
    assert!(mph.is_finite());
}

#[test]
fn test_very_small_values() {
    let small = 1e-10;

    // Should handle small values
    let celsius = TemperatureUnit::Celsius.convert_from_kelvin(small);
    assert!(celsius.is_finite());

    let psi = PressureUnit::PSI.convert_from_kpa(small);
    assert!(psi.is_finite());
}

#[test]
fn test_negative_values() {
    // Negative pressure (vacuum)
    let psi = PressureUnit::PSI.convert_from_kpa(-50.0);
    assert!(psi < 0.0);

    // Negative speed (reverse)
    let mph = SpeedUnit::Mph.convert_from_kmh(-100.0);
    assert!(mph < 0.0);

    // Negative acceleration (deceleration)
    let g = AccelerationUnit::G.convert_from_m_per_s2(-19.6133);
    assert!((g - (-2.0)).abs() < 0.01);
}

// ============================================
// Roundtrip Tests
// ============================================

#[test]
fn test_temperature_roundtrip() {
    // Converting to Fahrenheit and back should preserve value
    let original_kelvin = 300.0;
    let fahrenheit = TemperatureUnit::Fahrenheit.convert_from_kelvin(original_kelvin);

    // Reverse conversion: F to K = (F - 32) * 5/9 + 273.15
    let back_to_kelvin = (fahrenheit - 32.0) * 5.0 / 9.0 + 273.15;
    assert!((back_to_kelvin - original_kelvin).abs() < 0.01);
}

#[test]
fn test_pressure_roundtrip() {
    let original_kpa = 200.0;
    let psi = PressureUnit::PSI.convert_from_kpa(original_kpa);

    // Reverse: PSI to kPa = PSI / 0.145038
    let back_to_kpa = psi / 0.145038;
    assert!((back_to_kpa - original_kpa).abs() < 0.01);
}

#[test]
fn test_speed_roundtrip() {
    let original_kmh = 120.0;
    let mph = SpeedUnit::Mph.convert_from_kmh(original_kmh);

    // Reverse: mph to km/h = mph / 0.621371
    let back_to_kmh = mph / 0.621371;
    assert!((back_to_kmh - original_kmh).abs() < 0.01);
}

// ============================================
// All Units Have Symbols
// ============================================

#[test]
fn test_all_temperature_units_have_symbols() {
    let units = [
        TemperatureUnit::Celsius,
        TemperatureUnit::Fahrenheit,
        TemperatureUnit::Kelvin,
    ];

    for unit in &units {
        assert!(!unit.symbol().is_empty(), "Unit should have symbol");
    }
}

#[test]
fn test_all_pressure_units_have_symbols() {
    let units = [PressureUnit::KPa, PressureUnit::PSI, PressureUnit::Bar];

    for unit in &units {
        assert!(!unit.symbol().is_empty(), "Unit should have symbol");
    }
}

#[test]
fn test_all_speed_units_have_symbols() {
    let units = [SpeedUnit::KmH, SpeedUnit::Mph];

    for unit in &units {
        assert!(!unit.symbol().is_empty(), "Unit should have symbol");
    }
}
