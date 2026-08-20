//! Unit preference types and conversion utilities.
//!
//! This module provides user-configurable unit preferences for displaying
//! ECU log data in various measurement systems (metric, imperial, etc.).

use serde::{Deserialize, Serialize};
/// Temperature unit preference
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum TemperatureUnit {
    Kelvin,
    #[default]
    Celsius,
    Fahrenheit,
}

impl TemperatureUnit {
    pub fn symbol(&self) -> &'static str {
        match self {
            TemperatureUnit::Kelvin => "K",
            TemperatureUnit::Celsius => "°C",
            TemperatureUnit::Fahrenheit => "°F",
        }
    }

    /// Convert from Kelvin to the selected unit
    pub fn convert_from_kelvin(&self, kelvin: f64) -> f64 {
        match self {
            TemperatureUnit::Kelvin => kelvin,
            TemperatureUnit::Celsius => kelvin - 273.15,
            TemperatureUnit::Fahrenheit => (kelvin - 273.15) * 9.0 / 5.0 + 32.0,
        }
    }

    /// Convert from Celsius to the selected unit
    pub fn convert_from_celsius(&self, celsius: f64) -> f64 {
        match self {
            TemperatureUnit::Kelvin => celsius + 273.15,
            TemperatureUnit::Celsius => celsius,
            TemperatureUnit::Fahrenheit => celsius * 9.0 / 5.0 + 32.0,
        }
    }

    /// Convert from Fahrenheit to the selected unit
    pub fn convert_from_fahrenheit(&self, fahrenheit: f64) -> f64 {
        match self {
            TemperatureUnit::Kelvin => (fahrenheit - 32.0) * 5.0 / 9.0 + 273.15,
            TemperatureUnit::Celsius => (fahrenheit - 32.0) * 5.0 / 9.0,
            TemperatureUnit::Fahrenheit => fahrenheit,
        }
    }
}

/// Pressure unit preference
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum PressureUnit {
    #[default]
    KPa,
    PSI,
    Bar,
}

impl PressureUnit {
    pub fn symbol(&self) -> &'static str {
        match self {
            PressureUnit::KPa => "kPa",
            PressureUnit::PSI => "PSI",
            PressureUnit::Bar => "bar",
        }
    }

    /// Convert from kPa to the selected unit
    pub fn convert_from_kpa(&self, kpa: f64) -> f64 {
        match self {
            PressureUnit::KPa => kpa,
            PressureUnit::PSI => kpa * 0.145038,
            PressureUnit::Bar => kpa * 0.01,
        }
    }
}

/// Speed unit preference
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum SpeedUnit {
    #[default]
    KmH,
    Mph,
}

impl SpeedUnit {
    pub fn symbol(&self) -> &'static str {
        match self {
            SpeedUnit::KmH => "km/h",
            SpeedUnit::Mph => "mph",
        }
    }

    /// Convert from km/h to the selected unit
    pub fn convert_from_kmh(&self, kmh: f64) -> f64 {
        match self {
            SpeedUnit::KmH => kmh,
            SpeedUnit::Mph => kmh * 0.621371,
        }
    }

    /// Convert from m/s (GPS loggers such as RaceChrono) to the selected unit
    pub fn convert_from_mps(&self, mps: f64) -> f64 {
        self.convert_from_kmh(mps * 3.6)
    }
}

/// Distance unit preference
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum DistanceUnit {
    #[default]
    Kilometers,
    Miles,
}

impl DistanceUnit {
    pub fn symbol(&self) -> &'static str {
        match self {
            DistanceUnit::Kilometers => "km",
            DistanceUnit::Miles => "mi",
        }
    }

    /// Convert from km to the selected unit
    pub fn convert_from_km(&self, km: f64) -> f64 {
        match self {
            DistanceUnit::Kilometers => km,
            DistanceUnit::Miles => km * 0.621371,
        }
    }

    /// Convert a short distance in meters for display. The imperial
    /// counterpart of the meter is the foot — channels logged in meters
    /// (GPS altitude, accuracy, distance traveled) would read as 0.00x mi
    /// if forced through the km -> mi conversion.
    pub fn convert_from_meters(&self, meters: f64) -> f64 {
        match self {
            DistanceUnit::Kilometers => meters,
            DistanceUnit::Miles => meters * 3.28084,
        }
    }

    /// Display symbol for short distances (the meter-sourced counterpart
    /// of `symbol()`)
    pub fn short_symbol(&self) -> &'static str {
        match self {
            DistanceUnit::Kilometers => "m",
            DistanceUnit::Miles => "ft",
        }
    }
}

/// Fuel economy unit preference
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum FuelEconomyUnit {
    #[default]
    LPer100Km,
    Mpg,
    KmPerL,
}

impl FuelEconomyUnit {
    pub fn symbol(&self) -> &'static str {
        match self {
            FuelEconomyUnit::LPer100Km => "L/100km",
            FuelEconomyUnit::Mpg => "mpg",
            FuelEconomyUnit::KmPerL => "km/L",
        }
    }

    /// Convert from L/100km to the selected unit
    pub fn convert_from_l_per_100km(&self, l_per_100km: f64) -> f64 {
        match self {
            FuelEconomyUnit::LPer100Km => l_per_100km,
            FuelEconomyUnit::Mpg => {
                if l_per_100km > 0.0 {
                    235.215 / l_per_100km
                } else {
                    0.0
                }
            }
            FuelEconomyUnit::KmPerL => {
                if l_per_100km > 0.0 {
                    100.0 / l_per_100km
                } else {
                    0.0
                }
            }
        }
    }
}

/// Volume unit preference
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum VolumeUnit {
    #[default]
    Liters,
    Gallons,
}

impl VolumeUnit {
    pub fn symbol(&self) -> &'static str {
        match self {
            VolumeUnit::Liters => "L",
            VolumeUnit::Gallons => "gal",
        }
    }

    /// Convert from liters to the selected unit
    pub fn convert_from_liters(&self, liters: f64) -> f64 {
        match self {
            VolumeUnit::Liters => liters,
            VolumeUnit::Gallons => liters * 0.264172,
        }
    }
}

/// Flow rate unit preference
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum FlowUnit {
    #[default]
    CcPerMin,
    LbPerHr,
}

impl FlowUnit {
    pub fn symbol(&self) -> &'static str {
        match self {
            FlowUnit::CcPerMin => "cc/min",
            FlowUnit::LbPerHr => "lb/hr",
        }
    }

    /// Convert from cc/min to the selected unit (assuming gasoline density ~0.75 g/cc)
    pub fn convert_from_cc_per_min(&self, cc_per_min: f64) -> f64 {
        match self {
            FlowUnit::CcPerMin => cc_per_min,
            // cc/min * 0.75 g/cc * 60 min/hr / 453.592 g/lb = lb/hr
            FlowUnit::LbPerHr => cc_per_min * 0.75 * 60.0 / 453.592,
        }
    }
}

/// Acceleration unit preference
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum AccelerationUnit {
    #[default]
    MPerS2,
    G,
}

impl AccelerationUnit {
    pub fn symbol(&self) -> &'static str {
        match self {
            AccelerationUnit::MPerS2 => "m/s²",
            AccelerationUnit::G => "g",
        }
    }

    /// Convert from m/s² to the selected unit
    pub fn convert_from_m_per_s2(&self, m_per_s2: f64) -> f64 {
        match self {
            AccelerationUnit::MPerS2 => m_per_s2,
            AccelerationUnit::G => m_per_s2 / 9.80665,
        }
    }
}

/// AFR/Lambda unit preference
///
/// ECU logs may output mixture data as either AFR (Air Fuel Ratio, e.g. 14.7 for stoich gasoline)
/// or Lambda (normalized ratio, e.g. 1.0 for stoich). This preference controls which format
/// is displayed regardless of what the source ECU outputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum AfrLambdaUnit {
    #[default]
    AFR,
    Lambda,
}

impl AfrLambdaUnit {
    pub fn symbol(&self) -> &'static str {
        match self {
            AfrLambdaUnit::AFR => "AFR",
            AfrLambdaUnit::Lambda => "λ",
        }
    }

    /// Stoichiometric AFR for gasoline
    const GASOLINE_STOICH: f64 = 14.7;

    /// Convert from AFR to the selected unit
    pub fn convert_from_afr(&self, afr: f64) -> f64 {
        match self {
            AfrLambdaUnit::AFR => afr,
            AfrLambdaUnit::Lambda => afr / Self::GASOLINE_STOICH,
        }
    }

    /// Convert from Lambda to the selected unit
    pub fn convert_from_lambda(&self, lambda: f64) -> f64 {
        match self {
            AfrLambdaUnit::AFR => lambda * Self::GASOLINE_STOICH,
            AfrLambdaUnit::Lambda => lambda,
        }
    }
}

/// User preferences for display units
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UnitPreferences {
    pub temperature: TemperatureUnit,
    pub pressure: PressureUnit,
    pub speed: SpeedUnit,
    pub distance: DistanceUnit,
    pub fuel_economy: FuelEconomyUnit,
    pub volume: VolumeUnit,
    pub flow: FlowUnit,
    pub acceleration: AccelerationUnit,
    pub afr_lambda: AfrLambdaUnit,
}

impl UnitPreferences {
    /// Convert a value and get the display unit based on the source unit string
    /// Returns (converted_value, display_unit)
    pub fn convert_value<'a>(&self, value: f64, source_unit: &'a str) -> (f64, &'a str) {
        match source_unit {
            // Temperature (source is Kelvin)
            "K" => (
                self.temperature.convert_from_kelvin(value),
                self.temperature.symbol(),
            ),
            // Temperature (source is Celsius — most CSV parsers emit °C)
            "°C" => (
                self.temperature.convert_from_celsius(value),
                self.temperature.symbol(),
            ),
            // Temperature (source is Fahrenheit)
            "°F" => (
                self.temperature.convert_from_fahrenheit(value),
                self.temperature.symbol(),
            ),
            // Pressure (source is kPa)
            "kPa" => (
                self.pressure.convert_from_kpa(value),
                self.pressure.symbol(),
            ),
            // Speed (source is km/h)
            "km/h" => (self.speed.convert_from_kmh(value), self.speed.symbol()),
            // Speed (source is m/s — GPS loggers such as RaceChrono)
            "m/s" => (self.speed.convert_from_mps(value), self.speed.symbol()),
            // Distance (source is km)
            "km" => (self.distance.convert_from_km(value), self.distance.symbol()),
            // Short distance (source is meters). Metric keeps meters,
            // imperial shows feet — altitude or GPS accuracy in miles
            // would display as 0.00x mi.
            "m" => (
                self.distance.convert_from_meters(value),
                self.distance.short_symbol(),
            ),
            // Fuel economy (source is L/100km)
            "L/100km" => (
                self.fuel_economy.convert_from_l_per_100km(value),
                self.fuel_economy.symbol(),
            ),
            // Volume (source is L)
            "L" => (self.volume.convert_from_liters(value), self.volume.symbol()),
            // Flow (source is cc/min)
            "cc/min" => (self.flow.convert_from_cc_per_min(value), self.flow.symbol()),
            // Acceleration (source is m/s²)
            "m/s²" => (
                self.acceleration.convert_from_m_per_s2(value),
                self.acceleration.symbol(),
            ),
            // AFR (source is AFR, e.g. 14.7)
            "AFR" => (
                self.afr_lambda.convert_from_afr(value),
                self.afr_lambda.symbol(),
            ),
            // Lambda (source is λ, e.g. 1.0)
            "λ" => (
                self.afr_lambda.convert_from_lambda(value),
                self.afr_lambda.symbol(),
            ),
            // No conversion needed for other units
            _ => (value, source_unit),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // Temperature Unit Tests
    // ============================================

    #[test]
    fn test_temperature_kelvin_identity() {
        let unit = TemperatureUnit::Kelvin;
        assert_eq!(unit.convert_from_kelvin(273.15), 273.15);
        assert_eq!(unit.convert_from_kelvin(0.0), 0.0);
        assert_eq!(unit.convert_from_kelvin(373.15), 373.15);
    }

    #[test]
    fn test_temperature_kelvin_to_celsius() {
        let unit = TemperatureUnit::Celsius;
        // 0°C = 273.15K
        assert!((unit.convert_from_kelvin(273.15) - 0.0).abs() < 0.001);
        // 100°C = 373.15K
        assert!((unit.convert_from_kelvin(373.15) - 100.0).abs() < 0.001);
        // -40°C = 233.15K
        assert!((unit.convert_from_kelvin(233.15) - (-40.0)).abs() < 0.001);
        // Absolute zero
        assert!((unit.convert_from_kelvin(0.0) - (-273.15)).abs() < 0.001);
    }

    #[test]
    fn test_temperature_kelvin_to_fahrenheit() {
        let unit = TemperatureUnit::Fahrenheit;
        // 32°F = 273.15K (0°C)
        assert!((unit.convert_from_kelvin(273.15) - 32.0).abs() < 0.001);
        // 212°F = 373.15K (100°C)
        assert!((unit.convert_from_kelvin(373.15) - 212.0).abs() < 0.001);
        // -40°F = -40°C = 233.15K
        assert!((unit.convert_from_kelvin(233.15) - (-40.0)).abs() < 0.001);
    }

    #[test]
    fn test_temperature_from_celsius() {
        // Identity
        assert_eq!(TemperatureUnit::Celsius.convert_from_celsius(90.0), 90.0);
        // 100°C = 212°F
        assert!((TemperatureUnit::Fahrenheit.convert_from_celsius(100.0) - 212.0).abs() < 0.001);
        // -40°C = -40°F
        assert!((TemperatureUnit::Fahrenheit.convert_from_celsius(-40.0) - (-40.0)).abs() < 0.001);
        // 0°C = 273.15K
        assert!((TemperatureUnit::Kelvin.convert_from_celsius(0.0) - 273.15).abs() < 0.001);
    }

    #[test]
    fn test_temperature_from_fahrenheit() {
        // Identity
        assert_eq!(
            TemperatureUnit::Fahrenheit.convert_from_fahrenheit(212.0),
            212.0
        );
        // 212°F = 100°C
        assert!((TemperatureUnit::Celsius.convert_from_fahrenheit(212.0) - 100.0).abs() < 0.001);
        // 32°F = 273.15K
        assert!((TemperatureUnit::Kelvin.convert_from_fahrenheit(32.0) - 273.15).abs() < 0.001);
    }

    #[test]
    fn test_temperature_symbols() {
        assert_eq!(TemperatureUnit::Kelvin.symbol(), "K");
        assert_eq!(TemperatureUnit::Celsius.symbol(), "°C");
        assert_eq!(TemperatureUnit::Fahrenheit.symbol(), "°F");
    }

    // ============================================
    // Pressure Unit Tests
    // ============================================

    #[test]
    fn test_pressure_kpa_identity() {
        let unit = PressureUnit::KPa;
        assert_eq!(unit.convert_from_kpa(101.325), 101.325);
        assert_eq!(unit.convert_from_kpa(0.0), 0.0);
        assert_eq!(unit.convert_from_kpa(200.0), 200.0);
    }

    #[test]
    fn test_pressure_kpa_to_psi() {
        let unit = PressureUnit::PSI;
        // 1 kPa ≈ 0.145038 PSI
        assert!((unit.convert_from_kpa(1.0) - 0.145038).abs() < 0.0001);
        // Atmospheric pressure: 101.325 kPa ≈ 14.696 PSI
        assert!((unit.convert_from_kpa(101.325) - 14.696).abs() < 0.01);
        // 100 kPa ≈ 14.5 PSI
        assert!((unit.convert_from_kpa(100.0) - 14.5038).abs() < 0.01);
    }

    #[test]
    fn test_pressure_kpa_to_bar() {
        let unit = PressureUnit::Bar;
        // 100 kPa = 1 bar
        assert!((unit.convert_from_kpa(100.0) - 1.0).abs() < 0.001);
        // 101.325 kPa = 1.01325 bar (atmospheric)
        assert!((unit.convert_from_kpa(101.325) - 1.01325).abs() < 0.001);
        // 200 kPa = 2 bar
        assert!((unit.convert_from_kpa(200.0) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_pressure_symbols() {
        assert_eq!(PressureUnit::KPa.symbol(), "kPa");
        assert_eq!(PressureUnit::PSI.symbol(), "PSI");
        assert_eq!(PressureUnit::Bar.symbol(), "bar");
    }

    // ============================================
    // Speed Unit Tests
    // ============================================

    #[test]
    fn test_speed_kmh_identity() {
        let unit = SpeedUnit::KmH;
        assert_eq!(unit.convert_from_kmh(100.0), 100.0);
        assert_eq!(unit.convert_from_kmh(0.0), 0.0);
    }

    #[test]
    fn test_speed_kmh_to_mph() {
        let unit = SpeedUnit::Mph;
        // 100 km/h ≈ 62.14 mph
        assert!((unit.convert_from_kmh(100.0) - 62.1371).abs() < 0.001);
        // 160 km/h ≈ 99.42 mph
        assert!((unit.convert_from_kmh(160.0) - 99.4194).abs() < 0.01);
    }

    #[test]
    fn test_speed_from_mps() {
        // 10 m/s = 36 km/h
        assert!((SpeedUnit::KmH.convert_from_mps(10.0) - 36.0).abs() < 0.001);
        // 10 m/s ≈ 22.37 mph
        assert!((SpeedUnit::Mph.convert_from_mps(10.0) - 22.3694).abs() < 0.001);
        assert_eq!(SpeedUnit::KmH.convert_from_mps(0.0), 0.0);
    }

    #[test]
    fn test_speed_symbols() {
        assert_eq!(SpeedUnit::KmH.symbol(), "km/h");
        assert_eq!(SpeedUnit::Mph.symbol(), "mph");
    }

    // ============================================
    // Distance Unit Tests
    // ============================================

    #[test]
    fn test_distance_km_identity() {
        let unit = DistanceUnit::Kilometers;
        assert_eq!(unit.convert_from_km(100.0), 100.0);
        assert_eq!(unit.convert_from_km(0.0), 0.0);
    }

    #[test]
    fn test_distance_km_to_miles() {
        let unit = DistanceUnit::Miles;
        // 100 km ≈ 62.14 miles
        assert!((unit.convert_from_km(100.0) - 62.1371).abs() < 0.001);
        // 1.60934 km ≈ 1 mile
        assert!((unit.convert_from_km(1.60934) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_distance_from_meters() {
        // Metric keeps meters untouched
        assert_eq!(DistanceUnit::Kilometers.convert_from_meters(100.0), 100.0);
        // Imperial shows feet: 100 m ≈ 328.084 ft
        assert!((DistanceUnit::Miles.convert_from_meters(100.0) - 328.084).abs() < 0.001);
    }

    #[test]
    fn test_distance_symbols() {
        assert_eq!(DistanceUnit::Kilometers.symbol(), "km");
        assert_eq!(DistanceUnit::Miles.symbol(), "mi");
        assert_eq!(DistanceUnit::Kilometers.short_symbol(), "m");
        assert_eq!(DistanceUnit::Miles.short_symbol(), "ft");
    }

    // ============================================
    // Fuel Economy Unit Tests
    // ============================================

    #[test]
    fn test_fuel_economy_l_per_100km_identity() {
        let unit = FuelEconomyUnit::LPer100Km;
        assert_eq!(unit.convert_from_l_per_100km(10.0), 10.0);
        assert_eq!(unit.convert_from_l_per_100km(5.0), 5.0);
    }

    #[test]
    fn test_fuel_economy_l_per_100km_to_mpg() {
        let unit = FuelEconomyUnit::Mpg;
        // 10 L/100km ≈ 23.52 mpg
        assert!((unit.convert_from_l_per_100km(10.0) - 23.5215).abs() < 0.01);
        // 5 L/100km ≈ 47.04 mpg
        assert!((unit.convert_from_l_per_100km(5.0) - 47.043).abs() < 0.01);
        // Edge case: 0 L/100km should return 0 (not divide by zero)
        assert_eq!(unit.convert_from_l_per_100km(0.0), 0.0);
    }

    #[test]
    fn test_fuel_economy_l_per_100km_to_km_per_l() {
        let unit = FuelEconomyUnit::KmPerL;
        // 10 L/100km = 10 km/L
        assert!((unit.convert_from_l_per_100km(10.0) - 10.0).abs() < 0.001);
        // 5 L/100km = 20 km/L
        assert!((unit.convert_from_l_per_100km(5.0) - 20.0).abs() < 0.001);
        // Edge case: 0 L/100km should return 0
        assert_eq!(unit.convert_from_l_per_100km(0.0), 0.0);
    }

    #[test]
    fn test_fuel_economy_symbols() {
        assert_eq!(FuelEconomyUnit::LPer100Km.symbol(), "L/100km");
        assert_eq!(FuelEconomyUnit::Mpg.symbol(), "mpg");
        assert_eq!(FuelEconomyUnit::KmPerL.symbol(), "km/L");
    }

    // ============================================
    // Volume Unit Tests
    // ============================================

    #[test]
    fn test_volume_liters_identity() {
        let unit = VolumeUnit::Liters;
        assert_eq!(unit.convert_from_liters(100.0), 100.0);
        assert_eq!(unit.convert_from_liters(0.0), 0.0);
    }

    #[test]
    fn test_volume_liters_to_gallons() {
        let unit = VolumeUnit::Gallons;
        // 1 L ≈ 0.264172 gallons
        assert!((unit.convert_from_liters(1.0) - 0.264172).abs() < 0.0001);
        // 3.78541 L ≈ 1 gallon
        assert!((unit.convert_from_liters(3.78541) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_volume_symbols() {
        assert_eq!(VolumeUnit::Liters.symbol(), "L");
        assert_eq!(VolumeUnit::Gallons.symbol(), "gal");
    }

    // ============================================
    // Flow Rate Unit Tests
    // ============================================

    #[test]
    fn test_flow_cc_per_min_identity() {
        let unit = FlowUnit::CcPerMin;
        assert_eq!(unit.convert_from_cc_per_min(100.0), 100.0);
        assert_eq!(unit.convert_from_cc_per_min(0.0), 0.0);
    }

    #[test]
    fn test_flow_cc_per_min_to_lb_per_hr() {
        let unit = FlowUnit::LbPerHr;
        // Formula: cc/min * 0.75 g/cc * 60 min/hr / 453.592 g/lb
        // 100 cc/min ≈ 9.92 lb/hr
        let result = unit.convert_from_cc_per_min(100.0);
        let expected = 100.0 * 0.75 * 60.0 / 453.592;
        assert!((result - expected).abs() < 0.01);
    }

    #[test]
    fn test_flow_symbols() {
        assert_eq!(FlowUnit::CcPerMin.symbol(), "cc/min");
        assert_eq!(FlowUnit::LbPerHr.symbol(), "lb/hr");
    }

    // ============================================
    // Acceleration Unit Tests
    // ============================================

    #[test]
    fn test_acceleration_m_per_s2_identity() {
        let unit = AccelerationUnit::MPerS2;
        assert_eq!(unit.convert_from_m_per_s2(9.80665), 9.80665);
        assert_eq!(unit.convert_from_m_per_s2(0.0), 0.0);
    }

    #[test]
    fn test_acceleration_m_per_s2_to_g() {
        let unit = AccelerationUnit::G;
        // 1g = 9.80665 m/s²
        assert!((unit.convert_from_m_per_s2(9.80665) - 1.0).abs() < 0.0001);
        // 19.6133 m/s² = 2g
        assert!((unit.convert_from_m_per_s2(19.6133) - 2.0).abs() < 0.001);
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
        assert_eq!(prefs.temperature, TemperatureUnit::Celsius);
        assert_eq!(prefs.pressure, PressureUnit::KPa);
        assert_eq!(prefs.speed, SpeedUnit::KmH);
        assert_eq!(prefs.distance, DistanceUnit::Kilometers);
        assert_eq!(prefs.fuel_economy, FuelEconomyUnit::LPer100Km);
        assert_eq!(prefs.volume, VolumeUnit::Liters);
        assert_eq!(prefs.flow, FlowUnit::CcPerMin);
        assert_eq!(prefs.acceleration, AccelerationUnit::MPerS2);
        assert_eq!(prefs.afr_lambda, AfrLambdaUnit::AFR);
    }

    #[test]
    fn test_unit_preferences_convert_temperature() {
        let mut prefs = UnitPreferences::default();

        // Default: Celsius
        let (value, unit) = prefs.convert_value(293.15, "K");
        assert!((value - 20.0).abs() < 0.01);
        assert_eq!(unit, "°C");

        // Switch to Fahrenheit
        prefs.temperature = TemperatureUnit::Fahrenheit;
        let (value, unit) = prefs.convert_value(293.15, "K");
        assert!((value - 68.0).abs() < 0.1);
        assert_eq!(unit, "°F");
    }

    #[test]
    fn test_unit_preferences_convert_pressure() {
        let mut prefs = UnitPreferences::default();

        // Default: kPa
        let (value, unit) = prefs.convert_value(101.325, "kPa");
        assert!((value - 101.325).abs() < 0.001);
        assert_eq!(unit, "kPa");

        // Switch to PSI
        prefs.pressure = PressureUnit::PSI;
        let (value, unit) = prefs.convert_value(101.325, "kPa");
        assert!((value - 14.696).abs() < 0.01);
        assert_eq!(unit, "PSI");
    }

    #[test]
    fn test_unit_preferences_convert_celsius_and_fahrenheit_sources() {
        let mut prefs = UnitPreferences::default();

        // °C source with the default Celsius preference is untouched
        let (value, unit) = prefs.convert_value(90.0, "°C");
        assert_eq!(value, 90.0);
        assert_eq!(unit, "°C");

        // Switch to Fahrenheit: parsers that emit °C now honor it
        prefs.temperature = TemperatureUnit::Fahrenheit;
        let (value, unit) = prefs.convert_value(100.0, "°C");
        assert!((value - 212.0).abs() < 0.001);
        assert_eq!(unit, "°F");

        // °F source with a Celsius preference converts down
        prefs.temperature = TemperatureUnit::Celsius;
        let (value, unit) = prefs.convert_value(212.0, "°F");
        assert!((value - 100.0).abs() < 0.001);
        assert_eq!(unit, "°C");
    }

    #[test]
    fn test_unit_preferences_convert_mps_source() {
        let mut prefs = UnitPreferences::default();

        // Default km/h preference: 10 m/s displays as 36 km/h
        let (value, unit) = prefs.convert_value(10.0, "m/s");
        assert!((value - 36.0).abs() < 0.001);
        assert_eq!(unit, "km/h");

        // mph preference
        prefs.speed = SpeedUnit::Mph;
        let (value, unit) = prefs.convert_value(10.0, "m/s");
        assert!((value - 22.3694).abs() < 0.001);
        assert_eq!(unit, "mph");
    }

    #[test]
    fn test_unit_preferences_convert_meters_source() {
        let mut prefs = UnitPreferences::default();

        // Metric preference keeps meters as meters (altitude, GPS accuracy)
        let (value, unit) = prefs.convert_value(100.0, "m");
        assert_eq!(value, 100.0);
        assert_eq!(unit, "m");

        // Imperial preference shows feet, never 0.00x mi
        prefs.distance = DistanceUnit::Miles;
        let (value, unit) = prefs.convert_value(100.0, "m");
        assert!((value - 328.084).abs() < 0.001);
        assert_eq!(unit, "ft");
    }

    #[test]
    fn test_unit_preferences_unknown_unit_passthrough() {
        let prefs = UnitPreferences::default();
        // Unknown units should pass through unchanged
        let (value, unit) = prefs.convert_value(42.0, "RPM");
        assert_eq!(value, 42.0);
        assert_eq!(unit, "RPM");

        let (value, unit) = prefs.convert_value(100.0, "%");
        assert_eq!(value, 100.0);
        assert_eq!(unit, "%");
    }

    #[test]
    fn test_unit_preferences_all_conversions() {
        let prefs = UnitPreferences {
            temperature: TemperatureUnit::Fahrenheit,
            pressure: PressureUnit::PSI,
            speed: SpeedUnit::Mph,
            distance: DistanceUnit::Miles,
            fuel_economy: FuelEconomyUnit::Mpg,
            volume: VolumeUnit::Gallons,
            flow: FlowUnit::LbPerHr,
            acceleration: AccelerationUnit::G,
            afr_lambda: AfrLambdaUnit::Lambda,
        };

        // Test each conversion
        let (_, unit) = prefs.convert_value(300.0, "K");
        assert_eq!(unit, "°F");

        let (_, unit) = prefs.convert_value(100.0, "kPa");
        assert_eq!(unit, "PSI");

        let (_, unit) = prefs.convert_value(100.0, "km/h");
        assert_eq!(unit, "mph");

        let (_, unit) = prefs.convert_value(100.0, "km");
        assert_eq!(unit, "mi");

        let (_, unit) = prefs.convert_value(10.0, "L/100km");
        assert_eq!(unit, "mpg");

        let (_, unit) = prefs.convert_value(50.0, "L");
        assert_eq!(unit, "gal");

        let (_, unit) = prefs.convert_value(100.0, "cc/min");
        assert_eq!(unit, "lb/hr");

        let (_, unit) = prefs.convert_value(9.8, "m/s²");
        assert_eq!(unit, "g");

        let (_, unit) = prefs.convert_value(14.7, "AFR");
        assert_eq!(unit, "λ");

        let (_, unit) = prefs.convert_value(1.0, "λ");
        assert_eq!(unit, "λ");
    }

    // ============================================
    // AFR/Lambda Unit Tests
    // ============================================

    #[test]
    fn test_afr_lambda_default_is_afr() {
        let unit = AfrLambdaUnit::default();
        assert_eq!(unit, AfrLambdaUnit::AFR);
    }

    #[test]
    fn test_afr_lambda_symbols() {
        assert_eq!(AfrLambdaUnit::AFR.symbol(), "AFR");
        assert_eq!(AfrLambdaUnit::Lambda.symbol(), "λ");
    }

    #[test]
    fn test_afr_to_afr_identity() {
        let unit = AfrLambdaUnit::AFR;
        assert_eq!(unit.convert_from_afr(14.7), 14.7);
        assert_eq!(unit.convert_from_afr(12.0), 12.0);
    }

    #[test]
    fn test_afr_to_lambda() {
        let unit = AfrLambdaUnit::Lambda;
        // Stoich: 14.7 AFR = 1.0 λ
        assert!((unit.convert_from_afr(14.7) - 1.0).abs() < 0.001);
        // Rich: 12.0 AFR ≈ 0.816 λ
        assert!((unit.convert_from_afr(12.0) - 0.8163).abs() < 0.001);
        // Lean: 16.0 AFR ≈ 1.088 λ
        assert!((unit.convert_from_afr(16.0) - 1.0884).abs() < 0.001);
    }

    #[test]
    fn test_lambda_to_lambda_identity() {
        let unit = AfrLambdaUnit::Lambda;
        assert_eq!(unit.convert_from_lambda(1.0), 1.0);
        assert_eq!(unit.convert_from_lambda(0.85), 0.85);
    }

    #[test]
    fn test_lambda_to_afr() {
        let unit = AfrLambdaUnit::AFR;
        // Stoich: 1.0 λ = 14.7 AFR
        assert!((unit.convert_from_lambda(1.0) - 14.7).abs() < 0.001);
        // Rich: 0.85 λ = 12.495 AFR
        assert!((unit.convert_from_lambda(0.85) - 12.495).abs() < 0.001);
        // Lean: 1.1 λ = 16.17 AFR
        assert!((unit.convert_from_lambda(1.1) - 16.17).abs() < 0.001);
    }

    #[test]
    fn test_afr_lambda_convert_value_integration() {
        let mut prefs = UnitPreferences::default();

        // Default is AFR — AFR source passes through
        let (value, unit) = prefs.convert_value(14.7, "AFR");
        assert!((value - 14.7).abs() < 0.001);
        assert_eq!(unit, "AFR");

        // Default is AFR — Lambda source converts to AFR
        let (value, unit) = prefs.convert_value(1.0, "λ");
        assert!((value - 14.7).abs() < 0.001);
        assert_eq!(unit, "AFR");

        // Switch to Lambda
        prefs.afr_lambda = AfrLambdaUnit::Lambda;

        // Lambda pref — AFR source converts to Lambda
        let (value, unit) = prefs.convert_value(14.7, "AFR");
        assert!((value - 1.0).abs() < 0.001);
        assert_eq!(unit, "λ");

        // Lambda pref — Lambda source passes through
        let (value, unit) = prefs.convert_value(1.0, "λ");
        assert!((value - 1.0).abs() < 0.001);
        assert_eq!(unit, "λ");
    }
}
