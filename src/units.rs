//! Unit preference types and conversion utilities.
//!
//! This module provides user-configurable unit preferences for displaying
//! ECU log data in various measurement systems (metric, imperial, etc.).

/// Temperature unit preference
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
}

/// Pressure unit preference
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
}

/// Distance unit preference
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
}

/// Fuel economy unit preference
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
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

/// User preferences for display units
#[derive(Clone, Debug, Default)]
pub struct UnitPreferences {
    pub temperature: TemperatureUnit,
    pub pressure: PressureUnit,
    pub speed: SpeedUnit,
    pub distance: DistanceUnit,
    pub fuel_economy: FuelEconomyUnit,
    pub volume: VolumeUnit,
    pub flow: FlowUnit,
    pub acceleration: AccelerationUnit,
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
            // Pressure (source is kPa)
            "kPa" => (
                self.pressure.convert_from_kpa(value),
                self.pressure.symbol(),
            ),
            // Speed (source is km/h)
            "km/h" => (self.speed.convert_from_kmh(value), self.speed.symbol()),
            // Distance (source is km)
            "km" => (self.distance.convert_from_km(value), self.distance.symbol()),
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
    fn test_distance_symbols() {
        assert_eq!(DistanceUnit::Kilometers.symbol(), "km");
        assert_eq!(DistanceUnit::Miles.symbol(), "mi");
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
    fn test_unit_preferences_unknown_unit_passthrough() {
        let prefs = UnitPreferences::default();
        // Unknown units should pass through unchanged
        let (value, unit) = prefs.convert_value(42.0, "RPM");
        assert_eq!(value, 42.0);
        assert_eq!(unit, "RPM");

        let (value, unit) = prefs.convert_value(14.7, "AFR");
        assert_eq!(value, 14.7);
        assert_eq!(unit, "AFR");
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
    }
}
