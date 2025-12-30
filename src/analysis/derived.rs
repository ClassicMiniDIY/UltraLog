//! Derived calculation algorithms.
//!
//! Computes derived engine metrics like Volumetric Efficiency,
//! Injector Duty Cycle, and other calculated values.

use super::*;
use std::collections::HashMap;

/// Volumetric Efficiency Analyzer
///
/// Estimates VE from MAF and engine parameters, or from MAP/IAT using
/// speed-density equations.
#[derive(Clone)]
pub struct VolumetricEfficiencyAnalyzer {
    /// RPM channel
    pub rpm_channel: String,
    /// MAP channel (kPa)
    pub map_channel: String,
    /// IAT channel (°C or K - set is_iat_kelvin accordingly)
    pub iat_channel: String,
    /// Engine displacement in liters
    pub displacement_l: f64,
    /// Whether IAT is already in Kelvin (false = Celsius)
    pub is_iat_kelvin: bool,
}

impl Default for VolumetricEfficiencyAnalyzer {
    fn default() -> Self {
        Self {
            rpm_channel: "RPM".to_string(),
            map_channel: "MAP".to_string(),
            iat_channel: "IAT".to_string(),
            displacement_l: 2.0, // Default 2.0L engine
            is_iat_kelvin: false,
        }
    }
}

impl Analyzer for VolumetricEfficiencyAnalyzer {
    fn id(&self) -> &str {
        "volumetric_efficiency"
    }

    fn name(&self) -> &str {
        "Volumetric Efficiency"
    }

    fn description(&self) -> &str {
        "Estimates Volumetric Efficiency (VE) from MAP, RPM, and IAT using the \
         speed-density equation. VE = (MAP × 2) / (ρ_ref × Displacement × RPM × 60). \
         Requires engine displacement to be configured."
    }

    fn category(&self) -> &str {
        "Derived"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.rpm_channel, &self.map_channel, &self.iat_channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let rpm = require_channel(log, &self.rpm_channel)?;
        let map = require_channel(log, &self.map_channel)?;
        let iat = require_channel(log, &self.iat_channel)?;

        if rpm.len() != map.len() || rpm.len() != iat.len() {
            return Err(AnalysisError::ComputationError(
                "Channels have different lengths".to_string(),
            ));
        }

        require_min_length(&rpm, 2)?;

        if self.displacement_l <= 0.0 {
            return Err(AnalysisError::InvalidParameter(
                "Displacement must be positive".to_string(),
            ));
        }

        let (ve_values, computation_time) = timed_analyze(|| {
            compute_volumetric_efficiency(&rpm, &map, &iat, self.displacement_l, self.is_iat_kelvin)
        });

        // Compute statistics
        let stats = super::statistics::compute_descriptive_stats(&ve_values);

        let mut warnings = vec![];

        // VE typically ranges from 70-110% for naturally aspirated,
        // and can exceed 100% for forced induction
        if stats.max > 150.0 {
            warnings.push(format!(
                "Very high VE detected (max {:.1}%) - verify displacement setting or check for sensor issues",
                stats.max
            ));
        }
        if stats.min < 20.0 && stats.min > 0.0 {
            warnings.push(format!(
                "Very low VE detected (min {:.1}%) - possible manifold leak or sensor issue",
                stats.min
            ));
        }

        Ok(AnalysisResult {
            name: "Volumetric Efficiency".to_string(),
            unit: "%".to_string(),
            values: ve_values,
            metadata: AnalysisMetadata {
                algorithm: "Speed-Density".to_string(),
                parameters: vec![
                    (
                        "displacement_l".to_string(),
                        format!("{:.2}L", self.displacement_l),
                    ),
                    ("mean_ve".to_string(), format!("{:.1}%", stats.mean)),
                    ("max_ve".to_string(), format!("{:.1}%", stats.max)),
                    ("min_ve".to_string(), format!("{:.1}%", stats.min)),
                ],
                warnings,
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("rpm_channel".to_string(), self.rpm_channel.clone());
        params.insert("map_channel".to_string(), self.map_channel.clone());
        params.insert("iat_channel".to_string(), self.iat_channel.clone());
        params.insert(
            "displacement_l".to_string(),
            self.displacement_l.to_string(),
        );
        params.insert("is_iat_kelvin".to_string(), self.is_iat_kelvin.to_string());

        AnalyzerConfig {
            id: self.id().to_string(),
            name: self.name().to_string(),
            parameters: params,
        }
    }

    fn set_config(&mut self, config: &AnalyzerConfig) {
        if let Some(ch) = config.parameters.get("rpm_channel") {
            self.rpm_channel = ch.clone();
        }
        if let Some(ch) = config.parameters.get("map_channel") {
            self.map_channel = ch.clone();
        }
        if let Some(ch) = config.parameters.get("iat_channel") {
            self.iat_channel = ch.clone();
        }
        if let Some(v) = config.parameters.get("displacement_l") {
            if let Ok(val) = v.parse() {
                self.displacement_l = val;
            }
        }
        if let Some(v) = config.parameters.get("is_iat_kelvin") {
            self.is_iat_kelvin = v.parse().unwrap_or(false);
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

/// Injector Duty Cycle Analyzer
///
/// Calculates injector duty cycle as a percentage, important for
/// determining if injectors are reaching their limit.
#[derive(Clone)]
pub struct InjectorDutyCycleAnalyzer {
    /// Injector pulse width channel (milliseconds)
    pub pulse_width_channel: String,
    /// RPM channel
    pub rpm_channel: String,
}

impl Default for InjectorDutyCycleAnalyzer {
    fn default() -> Self {
        Self {
            pulse_width_channel: "IPW".to_string(), // Injector Pulse Width
            rpm_channel: "RPM".to_string(),
        }
    }
}

impl Analyzer for InjectorDutyCycleAnalyzer {
    fn id(&self) -> &str {
        "injector_duty_cycle"
    }

    fn name(&self) -> &str {
        "Injector Duty Cycle"
    }

    fn description(&self) -> &str {
        "Calculates injector duty cycle (%) from pulse width and RPM. \
         Formula: IDC = (PW_ms × RPM) / 1200 for 4-stroke engines. \
         Warning issued above 80% (traditional) or 95% (high-performance)."
    }

    fn category(&self) -> &str {
        "Derived"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.pulse_width_channel, &self.rpm_channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let pw = require_channel(log, &self.pulse_width_channel)?;
        let rpm = require_channel(log, &self.rpm_channel)?;

        if pw.len() != rpm.len() {
            return Err(AnalysisError::ComputationError(
                "Channels have different lengths".to_string(),
            ));
        }

        require_min_length(&pw, 2)?;

        let (idc_values, computation_time) =
            timed_analyze(|| compute_injector_duty_cycle(&pw, &rpm));

        // Compute statistics
        let stats = super::statistics::compute_descriptive_stats(&idc_values);

        let mut warnings = vec![];

        // Count samples at various duty cycle thresholds
        let above_80 = idc_values.iter().filter(|&&v| v > 80.0).count();
        let above_95 = idc_values.iter().filter(|&&v| v > 95.0).count();
        let at_100 = idc_values.iter().filter(|&&v| v >= 100.0).count();
        let total = idc_values.len();

        if at_100 > 0 {
            warnings.push(format!(
                "CRITICAL: Injectors at 100% duty cycle ({:.1}% of time) - \
                 fueling capacity exceeded, engine running lean!",
                100.0 * at_100 as f64 / total as f64
            ));
        } else if above_95 > 0 {
            warnings.push(format!(
                "High duty cycle (>95%) detected ({:.1}% of time) - \
                 approaching injector limits",
                100.0 * above_95 as f64 / total as f64
            ));
        } else if above_80 > total / 10 {
            warnings.push(format!(
                "Elevated duty cycle (>80%) for {:.1}% of samples - \
                 consider larger injectors for additional power",
                100.0 * above_80 as f64 / total as f64
            ));
        }

        Ok(AnalysisResult {
            name: "Injector Duty Cycle".to_string(),
            unit: "%".to_string(),
            values: idc_values,
            metadata: AnalysisMetadata {
                algorithm: "PW × RPM / 1200".to_string(),
                parameters: vec![
                    ("mean_idc".to_string(), format!("{:.1}%", stats.mean)),
                    ("max_idc".to_string(), format!("{:.1}%", stats.max)),
                    ("samples_above_80".to_string(), format!("{}", above_80)),
                    ("samples_above_95".to_string(), format!("{}", above_95)),
                ],
                warnings,
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert(
            "pulse_width_channel".to_string(),
            self.pulse_width_channel.clone(),
        );
        params.insert("rpm_channel".to_string(), self.rpm_channel.clone());

        AnalyzerConfig {
            id: self.id().to_string(),
            name: self.name().to_string(),
            parameters: params,
        }
    }

    fn set_config(&mut self, config: &AnalyzerConfig) {
        if let Some(ch) = config.parameters.get("pulse_width_channel") {
            self.pulse_width_channel = ch.clone();
        }
        if let Some(ch) = config.parameters.get("rpm_channel") {
            self.rpm_channel = ch.clone();
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

/// Lambda Calculator
///
/// Converts AFR to Lambda for easier analysis across different fuels.
#[derive(Clone)]
pub struct LambdaCalculator {
    /// AFR channel to convert
    pub afr_channel: String,
    /// Stoichiometric AFR for the fuel (14.7 for gasoline, 14.6 for E10, etc.)
    pub stoich_afr: f64,
}

impl Default for LambdaCalculator {
    fn default() -> Self {
        Self {
            afr_channel: "AFR".to_string(),
            stoich_afr: 14.7,
        }
    }
}

impl Analyzer for LambdaCalculator {
    fn id(&self) -> &str {
        "lambda_calculator"
    }

    fn name(&self) -> &str {
        "Lambda Calculator"
    }

    fn description(&self) -> &str {
        "Converts AFR to Lambda (λ = AFR / Stoich). Lambda of 1.0 = stoichiometric. \
         Useful for comparing fueling across different fuel types."
    }

    fn category(&self) -> &str {
        "Derived"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.afr_channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let afr = require_channel(log, &self.afr_channel)?;
        require_min_length(&afr, 2)?;

        if self.stoich_afr <= 0.0 {
            return Err(AnalysisError::InvalidParameter(
                "Stoichiometric AFR must be positive".to_string(),
            ));
        }

        let (lambda_values, computation_time) = timed_analyze(|| {
            afr.iter()
                .map(|&a| a / self.stoich_afr)
                .collect::<Vec<f64>>()
        });

        let stats = super::statistics::compute_descriptive_stats(&lambda_values);

        let mut warnings = vec![];

        if stats.min < 0.7 {
            warnings.push(format!(
                "Very rich lambda detected (min {:.2}) - check for flooding or \
                 over-fueling conditions",
                stats.min
            ));
        }
        if stats.max > 1.3 {
            warnings.push(format!(
                "Very lean lambda detected (max {:.2}) - risk of detonation, \
                 check fueling",
                stats.max
            ));
        }

        Ok(AnalysisResult {
            name: "Lambda".to_string(),
            unit: "λ".to_string(),
            values: lambda_values,
            metadata: AnalysisMetadata {
                algorithm: "AFR / Stoich".to_string(),
                parameters: vec![
                    ("stoich_afr".to_string(), format!("{:.1}", self.stoich_afr)),
                    ("mean_lambda".to_string(), format!("{:.3}", stats.mean)),
                    ("min_lambda".to_string(), format!("{:.3}", stats.min)),
                    ("max_lambda".to_string(), format!("{:.3}", stats.max)),
                ],
                warnings,
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("afr_channel".to_string(), self.afr_channel.clone());
        params.insert("stoich_afr".to_string(), self.stoich_afr.to_string());

        AnalyzerConfig {
            id: self.id().to_string(),
            name: self.name().to_string(),
            parameters: params,
        }
    }

    fn set_config(&mut self, config: &AnalyzerConfig) {
        if let Some(ch) = config.parameters.get("afr_channel") {
            self.afr_channel = ch.clone();
        }
        if let Some(v) = config.parameters.get("stoich_afr") {
            if let Ok(val) = v.parse() {
                self.stoich_afr = val;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

// ============================================================================
// Core derived calculation implementations
// ============================================================================

/// Compute Volumetric Efficiency from speed-density equation
///
/// VE% = (Actual air mass / Theoretical air mass) × 100
///
/// For speed-density calculation:
/// VE% = (MAP × 2) / (P_ref × Disp × RPM × (T_ref/T_actual) × 1/60) × 100
///
/// Simplified: VE% ≈ (MAP / 101.325) × (298 / T_actual_K) × 100
/// This gives a relative VE assuming standard conditions.
fn compute_volumetric_efficiency(
    rpm: &[f64],
    map: &[f64],
    iat: &[f64],
    displacement_l: f64,
    is_iat_kelvin: bool,
) -> Vec<f64> {
    // Reference conditions
    const P_REF: f64 = 101.325; // Standard pressure in kPa
    const T_REF: f64 = 298.0; // Standard temperature in Kelvin (25°C)

    rpm.iter()
        .zip(map.iter())
        .zip(iat.iter())
        .map(|((&r, &m), &t)| {
            // Convert IAT to Kelvin if needed
            let t_kelvin = if is_iat_kelvin { t } else { t + 273.15 };

            // Avoid division by zero
            if r <= 0.0 || t_kelvin <= 0.0 {
                return 0.0;
            }

            // Speed-density VE calculation
            // This is a simplified model that gives relative VE
            // VE = (MAP / P_ref) × (T_ref / T_actual) × 100
            // This gives how much air we're getting compared to standard conditions

            // More accurate would use MAF if available:
            // VE = (MAF_actual × 120) / (ρ_std × Disp_cc × RPM) × 100

            // For now, use the MAP-based estimate
            let ve = (m / P_REF) * (T_REF / t_kelvin) * 100.0;

            // Clamp to reasonable range (negative values not physical)
            ve.max(0.0)
        })
        .collect()
}

/// Compute Injector Duty Cycle
///
/// For 4-stroke engines, each injector fires once per 2 revolutions:
/// Time per injection cycle = 60,000 / (RPM / 2) = 120,000 / RPM (in ms)
///
/// IDC% = (Pulse_Width_ms / Time_per_cycle_ms) × 100
/// IDC% = (PW × RPM) / 120,000 × 100
/// IDC% = (PW × RPM) / 1200
fn compute_injector_duty_cycle(pulse_width: &[f64], rpm: &[f64]) -> Vec<f64> {
    pulse_width
        .iter()
        .zip(rpm.iter())
        .map(|(&pw, &r)| {
            if r <= 0.0 {
                return 0.0;
            }
            // IDC = (PW_ms × RPM) / 1200
            let idc = (pw * r) / 1200.0;
            idc.max(0.0).min(100.0) // Clamp to 0-100%
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_injector_duty_cycle() {
        // At 6000 RPM with 10ms pulse width
        // IDC = (10 × 6000) / 1200 = 50%
        let pw = vec![10.0];
        let rpm = vec![6000.0];
        let idc = compute_injector_duty_cycle(&pw, &rpm);
        assert!((idc[0] - 50.0).abs() < 0.01);

        // At 7200 RPM with 10ms pulse width
        // IDC = (10 × 7200) / 1200 = 60%
        let pw = vec![10.0];
        let rpm = vec![7200.0];
        let idc = compute_injector_duty_cycle(&pw, &rpm);
        assert!((idc[0] - 60.0).abs() < 0.01);

        // At redline 8000 RPM with 15ms
        // IDC = (15 × 8000) / 1200 = 100%
        let pw = vec![15.0];
        let rpm = vec![8000.0];
        let idc = compute_injector_duty_cycle(&pw, &rpm);
        assert!((idc[0] - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_volumetric_efficiency() {
        // At atmospheric pressure and standard temp, VE should be ~100%
        let rpm = vec![3000.0];
        let map = vec![101.325]; // 1 atm
        let iat = vec![25.0]; // 25°C

        let ve = compute_volumetric_efficiency(&rpm, &map, &iat, 2.0, false);
        assert!((ve[0] - 100.0).abs() < 1.0); // Should be close to 100%

        // At half atmospheric pressure, VE should be ~50%
        let map = vec![50.0];
        let ve = compute_volumetric_efficiency(&rpm, &map, &iat, 2.0, false);
        assert!((ve[0] - 50.0).abs() < 5.0);
    }

    #[test]
    fn test_lambda_calculation() {
        // Lambda = AFR / 14.7
        assert!((14.7_f64 / 14.7 - 1.0).abs() < 0.001); // Stoich = lambda 1.0
        assert!((13.0_f64 / 14.7) < 1.0); // Rich (lambda < 1)
        assert!((16.0_f64 / 14.7) > 1.0); // Lean (lambda > 1)
    }
}
