//! Air-Fuel Ratio analysis algorithms.
//!
//! Provides AFR-related analysis including fuel trim drift detection (CUSUM),
//! rich/lean zone detection, and AFR deviation analysis.

use super::*;
use std::collections::HashMap;

/// Fuel Trim Drift Analyzer using CUSUM algorithm
///
/// Detects gradual drift in long-term fuel trim that may indicate
/// injector degradation, air leaks, or sensor aging.
#[derive(Clone)]
pub struct FuelTrimDriftAnalyzer {
    /// Channel to analyze (typically LTFT or STFT)
    pub channel: String,
    /// Allowable slack parameter k (typically 0.5σ)
    /// Controls sensitivity to small shifts
    pub k: f64,
    /// Decision threshold h (typically 4-5σ)
    /// Higher values = fewer false alarms, slower detection
    pub h: f64,
    /// Baseline calculation window (percentage of data from start)
    pub baseline_pct: f64,
}

impl Default for FuelTrimDriftAnalyzer {
    fn default() -> Self {
        Self {
            channel: "LTFT".to_string(),
            k: 2.5,             // 0.5 * typical 5% σ
            h: 20.0,            // 4 * typical 5% σ
            baseline_pct: 10.0, // Use first 10% for baseline
        }
    }
}

impl Analyzer for FuelTrimDriftAnalyzer {
    fn id(&self) -> &str {
        "fuel_trim_drift"
    }

    fn name(&self) -> &str {
        "Fuel Trim Drift Detection"
    }

    fn description(&self) -> &str {
        "CUSUM algorithm detecting gradual long-term fuel trim drift indicating \
         injector degradation, air leaks, or sensor aging. Returns drift indicator: \
         +1 = rich drift, -1 = lean drift, 0 = normal."
    }

    fn category(&self) -> &str {
        "AFR"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let data = require_channel(log, &self.channel)?;
        require_min_length(&data, 100)?;

        let (result_data, computation_time) =
            timed_analyze(|| cusum_drift_detection(&data, self.k, self.h, self.baseline_pct));

        // Count drift events for warnings
        let high_drift_samples = result_data.drift_flags.iter().filter(|&&x| x > 0.5).count();
        let low_drift_samples = result_data
            .drift_flags
            .iter()
            .filter(|&&x| x < -0.5)
            .count();
        let total = data.len();

        let mut warnings = vec![];

        if high_drift_samples > total / 20 {
            warnings.push(format!(
                "Sustained positive drift detected ({:.1}% of samples) - running rich, check for over-fueling",
                100.0 * high_drift_samples as f64 / total as f64
            ));
        }
        if low_drift_samples > total / 20 {
            warnings.push(format!(
                "Sustained negative drift detected ({:.1}% of samples) - running lean, check for air leaks",
                100.0 * low_drift_samples as f64 / total as f64
            ));
        }

        Ok(AnalysisResult {
            name: format!("{} Drift", self.channel),
            unit: "drift".to_string(),
            values: result_data.drift_flags,
            metadata: AnalysisMetadata {
                algorithm: "CUSUM".to_string(),
                parameters: vec![
                    ("k".to_string(), format!("{:.2}", self.k)),
                    ("h".to_string(), format!("{:.2}", self.h)),
                    (
                        "baseline_μ".to_string(),
                        format!("{:.2}%", result_data.baseline_mean),
                    ),
                    (
                        "baseline_σ".to_string(),
                        format!("{:.2}%", result_data.baseline_stdev),
                    ),
                ],
                warnings,
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("channel".to_string(), self.channel.clone());
        params.insert("k".to_string(), self.k.to_string());
        params.insert("h".to_string(), self.h.to_string());
        params.insert("baseline_pct".to_string(), self.baseline_pct.to_string());

        AnalyzerConfig {
            id: self.id().to_string(),
            name: self.name().to_string(),
            parameters: params,
        }
    }

    fn set_config(&mut self, config: &AnalyzerConfig) {
        if let Some(ch) = config.parameters.get("channel") {
            self.channel = ch.clone();
        }
        if let Some(v) = config.parameters.get("k") {
            if let Ok(val) = v.parse() {
                self.k = val;
            }
        }
        if let Some(v) = config.parameters.get("h") {
            if let Ok(val) = v.parse() {
                self.h = val;
            }
        }
        if let Some(v) = config.parameters.get("baseline_pct") {
            if let Ok(val) = v.parse() {
                self.baseline_pct = val;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

/// Rich/Lean Zone Analyzer
///
/// Detects periods where AFR deviates significantly from target,
/// classifying into rich, lean, and stoichiometric zones.
#[derive(Clone)]
pub struct RichLeanZoneAnalyzer {
    /// AFR channel to analyze
    pub channel: String,
    /// Target AFR (default stoichiometric 14.7)
    pub target_afr: f64,
    /// Rich threshold (below target - this value)
    pub rich_threshold: f64,
    /// Lean threshold (above target + this value)
    pub lean_threshold: f64,
}

impl Default for RichLeanZoneAnalyzer {
    fn default() -> Self {
        Self {
            channel: "AFR".to_string(),
            target_afr: 14.7,    // Stoichiometric for gasoline
            rich_threshold: 0.5, // Rich if AFR < 14.2
            lean_threshold: 0.5, // Lean if AFR > 15.2
        }
    }
}

impl Analyzer for RichLeanZoneAnalyzer {
    fn id(&self) -> &str {
        "rich_lean_zone"
    }

    fn name(&self) -> &str {
        "Rich/Lean Zone Detection"
    }

    fn description(&self) -> &str {
        "Classifies AFR readings into rich (-1), stoichiometric (0), and lean (+1) zones \
         based on deviation from target AFR. Also computes time spent in each zone."
    }

    fn category(&self) -> &str {
        "AFR"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let data = require_channel(log, &self.channel)?;
        require_min_length(&data, 10)?;

        let rich_limit = self.target_afr - self.rich_threshold;
        let lean_limit = self.target_afr + self.lean_threshold;

        let (zones, computation_time) = timed_analyze(|| {
            data.iter()
                .map(|&afr| {
                    if afr < rich_limit {
                        -1.0 // Rich
                    } else if afr > lean_limit {
                        1.0 // Lean
                    } else {
                        0.0 // Stoichiometric
                    }
                })
                .collect::<Vec<f64>>()
        });

        // Count time in each zone
        let rich_count = zones.iter().filter(|&&z| z < -0.5).count();
        let lean_count = zones.iter().filter(|&&z| z > 0.5).count();
        let stoich_count = zones.iter().filter(|&&z| z.abs() < 0.5).count();
        let total = zones.len() as f64;

        let rich_pct = 100.0 * rich_count as f64 / total;
        let lean_pct = 100.0 * lean_count as f64 / total;
        let stoich_pct = 100.0 * stoich_count as f64 / total;

        let mut warnings = vec![];

        if rich_pct > 30.0 {
            warnings.push(format!(
                "Excessive rich operation ({:.1}%) - may indicate over-fueling or cold conditions",
                rich_pct
            ));
        }
        if lean_pct > 30.0 {
            warnings.push(format!(
                "Excessive lean operation ({:.1}%) - check for air leaks or fuel delivery issues",
                lean_pct
            ));
        }

        Ok(AnalysisResult {
            name: format!("{} Zone", self.channel),
            unit: "zone".to_string(),
            values: zones,
            metadata: AnalysisMetadata {
                algorithm: "Threshold Classification".to_string(),
                parameters: vec![
                    ("target_afr".to_string(), format!("{:.1}", self.target_afr)),
                    ("rich_limit".to_string(), format!("{:.1}", rich_limit)),
                    ("lean_limit".to_string(), format!("{:.1}", lean_limit)),
                    ("rich_pct".to_string(), format!("{:.1}%", rich_pct)),
                    ("stoich_pct".to_string(), format!("{:.1}%", stoich_pct)),
                    ("lean_pct".to_string(), format!("{:.1}%", lean_pct)),
                ],
                warnings,
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("channel".to_string(), self.channel.clone());
        params.insert("target_afr".to_string(), self.target_afr.to_string());
        params.insert(
            "rich_threshold".to_string(),
            self.rich_threshold.to_string(),
        );
        params.insert(
            "lean_threshold".to_string(),
            self.lean_threshold.to_string(),
        );

        AnalyzerConfig {
            id: self.id().to_string(),
            name: self.name().to_string(),
            parameters: params,
        }
    }

    fn set_config(&mut self, config: &AnalyzerConfig) {
        if let Some(ch) = config.parameters.get("channel") {
            self.channel = ch.clone();
        }
        if let Some(v) = config.parameters.get("target_afr") {
            if let Ok(val) = v.parse() {
                self.target_afr = val;
            }
        }
        if let Some(v) = config.parameters.get("rich_threshold") {
            if let Ok(val) = v.parse() {
                self.rich_threshold = val;
            }
        }
        if let Some(v) = config.parameters.get("lean_threshold") {
            if let Ok(val) = v.parse() {
                self.lean_threshold = val;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

/// AFR Deviation Analyzer
///
/// Computes percentage deviation from target AFR, useful for
/// fuel table correction calculations.
#[derive(Clone)]
pub struct AfrDeviationAnalyzer {
    /// AFR channel to analyze
    pub channel: String,
    /// Target AFR (default stoichiometric 14.7)
    pub target_afr: f64,
}

impl Default for AfrDeviationAnalyzer {
    fn default() -> Self {
        Self {
            channel: "AFR".to_string(),
            target_afr: 14.7,
        }
    }
}

impl Analyzer for AfrDeviationAnalyzer {
    fn id(&self) -> &str {
        "afr_deviation"
    }

    fn name(&self) -> &str {
        "AFR Deviation %"
    }

    fn description(&self) -> &str {
        "Computes percentage deviation from target AFR. Positive = lean, negative = rich. \
         Useful for determining fuel table corrections."
    }

    fn category(&self) -> &str {
        "AFR"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let data = require_channel(log, &self.channel)?;
        require_min_length(&data, 2)?;

        if self.target_afr <= 0.0 {
            return Err(AnalysisError::InvalidParameter(
                "Target AFR must be positive".to_string(),
            ));
        }

        let (deviations, computation_time) = timed_analyze(|| {
            data.iter()
                .map(|&afr| ((afr - self.target_afr) / self.target_afr) * 100.0)
                .collect::<Vec<f64>>()
        });

        // Compute statistics on deviations
        let stats = super::statistics::compute_descriptive_stats(&deviations);

        let mut warnings = vec![];

        if stats.mean.abs() > 5.0 {
            let direction = if stats.mean > 0.0 { "lean" } else { "rich" };
            warnings.push(format!(
                "Significant average {} bias ({:.1}%) - consider fuel table adjustment",
                direction, stats.mean
            ));
        }

        if stats.stdev > 10.0 {
            warnings.push(format!(
                "High AFR variability (σ={:.1}%) - check sensor or tune stability",
                stats.stdev
            ));
        }

        Ok(AnalysisResult {
            name: format!("{} Deviation", self.channel),
            unit: "%".to_string(),
            values: deviations,
            metadata: AnalysisMetadata {
                algorithm: "Percentage Deviation".to_string(),
                parameters: vec![
                    ("target_afr".to_string(), format!("{:.1}", self.target_afr)),
                    ("mean_deviation".to_string(), format!("{:.2}%", stats.mean)),
                    ("stdev".to_string(), format!("{:.2}%", stats.stdev)),
                    ("max_deviation".to_string(), format!("{:.2}%", stats.max)),
                    ("min_deviation".to_string(), format!("{:.2}%", stats.min)),
                ],
                warnings,
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("channel".to_string(), self.channel.clone());
        params.insert("target_afr".to_string(), self.target_afr.to_string());

        AnalyzerConfig {
            id: self.id().to_string(),
            name: self.name().to_string(),
            parameters: params,
        }
    }

    fn set_config(&mut self, config: &AnalyzerConfig) {
        if let Some(ch) = config.parameters.get("channel") {
            self.channel = ch.clone();
        }
        if let Some(v) = config.parameters.get("target_afr") {
            if let Ok(val) = v.parse() {
                self.target_afr = val;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

// ============================================================================
// Core AFR algorithm implementations
// ============================================================================

/// Result of CUSUM drift detection
struct CusumResult {
    drift_flags: Vec<f64>,
    baseline_mean: f64,
    baseline_stdev: f64,
}

/// CUSUM (Cumulative Sum) drift detection algorithm
///
/// Detects gradual shifts from baseline mean.
/// - k: allowable slack (sensitivity), typically 0.5σ
/// - h: decision threshold, typically 4-5σ
fn cusum_drift_detection(data: &[f64], k: f64, h: f64, baseline_pct: f64) -> CusumResult {
    if data.is_empty() {
        return CusumResult {
            drift_flags: vec![],
            baseline_mean: 0.0,
            baseline_stdev: 1.0,
        };
    }

    // Calculate baseline statistics from initial portion of data
    let baseline_len = ((data.len() as f64 * baseline_pct / 100.0) as usize).max(10);
    let baseline_data = &data[..baseline_len.min(data.len())];

    let baseline_mean: f64 = baseline_data.iter().sum::<f64>() / baseline_data.len() as f64;
    let baseline_variance: f64 = baseline_data
        .iter()
        .map(|x| (x - baseline_mean).powi(2))
        .sum::<f64>()
        / (baseline_data.len() - 1).max(1) as f64;
    let baseline_stdev = baseline_variance.sqrt().max(0.001);

    // CUSUM calculation
    let mut s_high = 0.0;
    let mut s_low = 0.0;
    let mut drift_flags = Vec::with_capacity(data.len());

    for &x in data {
        // Update CUSUM statistics
        s_high = (s_high + (x - baseline_mean) - k).max(0.0);
        s_low = (s_low + (-x + baseline_mean) - k).max(0.0);

        // Determine drift direction
        let flag = if s_high > h {
            1.0 // Positive drift (running rich if this is fuel trim)
        } else if s_low > h {
            -1.0 // Negative drift (running lean if this is fuel trim)
        } else {
            0.0 // Normal
        };
        drift_flags.push(flag);

        // Reset after detection (optional - prevents persistent flagging)
        if s_high > h {
            s_high = 0.0;
        }
        if s_low > h {
            s_low = 0.0;
        }
    }

    CusumResult {
        drift_flags,
        baseline_mean,
        baseline_stdev,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cusum_stable() {
        // Stable data around 0 should not trigger drift
        let data: Vec<f64> = (0..200).map(|_| 0.5).collect();
        let result = cusum_drift_detection(&data, 2.5, 20.0, 10.0);

        let drift_count = result.drift_flags.iter().filter(|&&x| x != 0.0).count();
        assert_eq!(drift_count, 0, "Stable data should have no drift");
    }

    #[test]
    fn test_cusum_drift_up() {
        // Data that drifts upward
        let mut data: Vec<f64> = vec![0.0; 100];
        data.extend(vec![10.0; 100]); // Step change

        let result = cusum_drift_detection(&data, 2.5, 20.0, 10.0);

        let drift_count = result.drift_flags.iter().filter(|&&x| x > 0.0).count();
        assert!(drift_count > 0, "Upward drift should be detected");
    }

    #[test]
    fn test_rich_lean_zones() {
        let afr_data = vec![14.7, 14.0, 15.5, 14.7, 13.5, 16.0];

        let analyzer = RichLeanZoneAnalyzer::default();
        let rich_limit = analyzer.target_afr - analyzer.rich_threshold;
        let lean_limit = analyzer.target_afr + analyzer.lean_threshold;

        // Count expected zones
        let rich = afr_data.iter().filter(|&&a| a < rich_limit).count();
        let lean = afr_data.iter().filter(|&&a| a > lean_limit).count();

        assert!(rich > 0, "Should detect rich conditions");
        assert!(lean > 0, "Should detect lean conditions");
    }

    #[test]
    fn test_afr_deviation() {
        let afr_data = vec![14.7, 15.435, 13.965]; // 0%, +5%, -5%
        let target = 14.7;

        let deviations: Vec<f64> = afr_data
            .iter()
            .map(|&afr| ((afr - target) / target) * 100.0)
            .collect();

        assert!((deviations[0] - 0.0).abs() < 0.1);
        assert!((deviations[1] - 5.0).abs() < 0.1);
        assert!((deviations[2] + 5.0).abs() < 0.1);
    }
}
