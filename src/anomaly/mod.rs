//! Anomaly detection module for ECU log data.
//!
//! This module provides local, fast anomaly detection algorithms that run
//! without requiring external LLM calls. Detected anomalies can be displayed
//! to users and/or included in LLM prompts for deeper analysis.
//!
//! ## Algorithms
//!
//! - **Z-Score**: Statistical outliers based on standard deviation
//! - **Rate of Change**: Sudden spikes or drops in values
//! - **Flatline**: Sensor stuck at constant value (potential failure)
//! - **Range Violation**: Values outside expected ECU parameter ranges
//! - **Correlation Break**: When typically correlated channels diverge

// ============================================================================
// Types
// ============================================================================

/// Severity level of a detected anomaly
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnomalySeverity {
    /// Informational - might be interesting but not concerning
    Info,
    /// Warning - potentially problematic, worth investigating
    Warning,
    /// Critical - likely indicates a real issue
    Critical,
}

impl AnomalySeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Critical => "critical",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Info => "ℹ️",
            Self::Warning => "⚠️",
            Self::Critical => "🚨",
        }
    }
}

/// Type of anomaly detected
#[derive(Debug, Clone, PartialEq)]
pub enum AnomalyType {
    /// Value is statistically unusual (z-score based)
    StatisticalOutlier { z_score: f64 },
    /// Sudden spike or drop in value
    RateOfChange { delta: f64, delta_per_second: f64 },
    /// Value stuck at constant (potential sensor failure)
    Flatline { duration_seconds: f64, stuck_value: f64 },
    /// Value outside expected range for this parameter
    RangeViolation { value: f64, expected_min: f64, expected_max: f64 },
    /// Correlation between channels broke down
    CorrelationBreak { other_channel: String, expected_correlation: f64, actual: f64 },
    /// Oscillation detected (value bouncing rapidly)
    Oscillation { frequency_hz: f64, amplitude: f64 },
}

impl AnomalyType {
    pub fn description(&self) -> String {
        match self {
            Self::StatisticalOutlier { z_score } => {
                format!("Statistical outlier (z-score: {:.1}σ)", z_score)
            }
            Self::RateOfChange { delta, delta_per_second } => {
                format!("Rapid change: {:.2}/s (Δ{:.2})", delta_per_second, delta)
            }
            Self::Flatline { duration_seconds, stuck_value } => {
                format!("Flatline for {:.1}s at {:.2}", duration_seconds, stuck_value)
            }
            Self::RangeViolation { value, expected_min, expected_max } => {
                format!("Out of range: {:.2} (expected {:.1}-{:.1})", value, expected_min, expected_max)
            }
            Self::CorrelationBreak { other_channel, expected_correlation, actual } => {
                format!("Correlation break with {} (expected {:.2}, got {:.2})",
                    other_channel, expected_correlation, actual)
            }
            Self::Oscillation { frequency_hz, amplitude } => {
                format!("Oscillation detected: {:.1}Hz, amplitude {:.2}", frequency_hz, amplitude)
            }
        }
    }
}

/// A detected anomaly in the ECU log data
#[derive(Debug, Clone)]
pub struct Anomaly {
    /// Channel where anomaly was detected
    pub channel_name: String,
    /// Index into the channel's data array
    pub data_index: usize,
    /// Timestamp in seconds
    pub time: f64,
    /// The value at this point
    pub value: f64,
    /// Type of anomaly
    pub anomaly_type: AnomalyType,
    /// Severity level
    pub severity: AnomalySeverity,
}

impl Anomaly {
    /// Format anomaly for display in UI
    pub fn display_string(&self) -> String {
        format!(
            "{} {} @ {:.2}s: {}",
            self.severity.emoji(),
            self.channel_name,
            self.time,
            self.anomaly_type.description()
        )
    }

    /// Format anomaly for LLM prompt inclusion
    pub fn prompt_string(&self) -> String {
        format!(
            "[{} {} @ {:.2}s] {}: value={:.3}, {}",
            self.severity.as_str().to_uppercase(),
            self.channel_name,
            self.time,
            self.anomaly_type.description(),
            self.value,
            match &self.anomaly_type {
                AnomalyType::StatisticalOutlier { z_score } =>
                    format!("This is {:.1} standard deviations from the mean", z_score),
                AnomalyType::RateOfChange { delta_per_second, .. } =>
                    format!("Rate of change: {:.2} units/second", delta_per_second),
                AnomalyType::Flatline { duration_seconds, .. } =>
                    format!("No change for {:.1} seconds", duration_seconds),
                AnomalyType::RangeViolation { expected_min, expected_max, .. } =>
                    format!("Normal range is {:.1} to {:.1}", expected_min, expected_max),
                AnomalyType::CorrelationBreak { other_channel, .. } =>
                    format!("Usually correlates with {}", other_channel),
                AnomalyType::Oscillation { frequency_hz, .. } =>
                    format!("Oscillating at {:.1} Hz", frequency_hz),
            }
        )
    }
}

/// Configuration for anomaly detection
#[derive(Debug, Clone)]
pub struct AnomalyConfig {
    /// Enable/disable anomaly detection
    pub enabled: bool,
    /// Z-score threshold for statistical outliers (default: 3.0)
    pub z_score_threshold: f64,
    /// Minimum rate of change to flag (units per second, relative to range)
    pub rate_of_change_threshold: f64,
    /// Minimum flatline duration to flag (seconds)
    pub flatline_min_duration: f64,
    /// Enable range violation detection (uses known ECU parameter ranges)
    pub check_ranges: bool,
    /// Enable correlation break detection
    pub check_correlations: bool,
    /// Maximum number of anomalies to detect per channel
    pub max_anomalies_per_channel: usize,
}

impl Default for AnomalyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            z_score_threshold: 3.0,
            rate_of_change_threshold: 0.5, // 50% of range per second
            flatline_min_duration: 2.0,
            check_ranges: true,
            check_correlations: true,
            max_anomalies_per_channel: 50,
        }
    }
}

/// Results from anomaly detection run
#[derive(Debug, Clone, Default)]
pub struct AnomalyResults {
    /// All detected anomalies, sorted by time
    pub anomalies: Vec<Anomaly>,
    /// Summary counts by severity
    pub info_count: usize,
    pub warning_count: usize,
    pub critical_count: usize,
    /// Channels analyzed
    pub channels_analyzed: usize,
    /// Data points analyzed
    pub points_analyzed: usize,
}

impl AnomalyResults {
    /// Get anomalies for a specific channel
    pub fn for_channel(&self, channel_name: &str) -> Vec<&Anomaly> {
        self.anomalies
            .iter()
            .filter(|a| a.channel_name == channel_name)
            .collect()
    }

    /// Get anomalies within a time range
    pub fn in_time_range(&self, start: f64, end: f64) -> Vec<&Anomaly> {
        self.anomalies
            .iter()
            .filter(|a| a.time >= start && a.time <= end)
            .collect()
    }

    /// Get the most severe anomalies (up to n)
    pub fn top_anomalies(&self, n: usize) -> Vec<&Anomaly> {
        let mut sorted: Vec<_> = self.anomalies.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted.into_iter().take(n).collect()
    }

    /// Generate a summary for LLM prompt
    pub fn to_prompt_summary(&self) -> String {
        if self.anomalies.is_empty() {
            return "No anomalies detected in the analyzed data.".to_string();
        }

        let mut summary = format!(
            "ANOMALY DETECTION RESULTS:\n\
             Analyzed {} channels, {} data points\n\
             Found {} anomalies: {} critical, {} warnings, {} info\n\n\
             TOP ANOMALIES:\n",
            self.channels_analyzed,
            self.points_analyzed,
            self.anomalies.len(),
            self.critical_count,
            self.warning_count,
            self.info_count
        );

        // Include top 10 most severe anomalies
        for anomaly in self.top_anomalies(10) {
            summary.push_str(&anomaly.prompt_string());
            summary.push('\n');
        }

        summary
    }
}

// ============================================================================
// Detection Algorithms
// ============================================================================

/// Main anomaly detector
pub struct AnomalyDetector {
    config: AnomalyConfig,
}

impl AnomalyDetector {
    pub fn new(config: AnomalyConfig) -> Self {
        Self { config }
    }

    /// Run all enabled detection algorithms on channel data
    pub fn analyze_channel(
        &self,
        channel_name: &str,
        times: &[f64],
        values: &[f64],
    ) -> Vec<Anomaly> {
        if !self.config.enabled || times.is_empty() || values.is_empty() {
            return Vec::new();
        }

        let mut anomalies = Vec::new();

        // Calculate basic statistics
        let stats = ChannelStats::calculate(values);

        // Run detection algorithms
        anomalies.extend(self.detect_statistical_outliers(channel_name, times, values, &stats));
        anomalies.extend(self.detect_rate_of_change(channel_name, times, values, &stats));
        anomalies.extend(self.detect_flatlines(channel_name, times, values));

        if self.config.check_ranges {
            anomalies.extend(self.detect_range_violations(channel_name, times, values));
        }

        // Sort by time and limit
        anomalies.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        anomalies.truncate(self.config.max_anomalies_per_channel);

        anomalies
    }

    /// Detect values that are statistically unusual (z-score method)
    fn detect_statistical_outliers(
        &self,
        channel_name: &str,
        times: &[f64],
        values: &[f64],
        stats: &ChannelStats,
    ) -> Vec<Anomaly> {
        if stats.std_dev < 0.0001 {
            return Vec::new(); // Avoid division by near-zero
        }

        let mut anomalies = Vec::new();
        let threshold = self.config.z_score_threshold;

        for (i, (&time, &value)) in times.iter().zip(values.iter()).enumerate() {
            let z_score = (value - stats.mean).abs() / stats.std_dev;

            if z_score >= threshold {
                let severity = if z_score >= threshold * 2.0 {
                    AnomalySeverity::Critical
                } else if z_score >= threshold * 1.5 {
                    AnomalySeverity::Warning
                } else {
                    AnomalySeverity::Info
                };

                anomalies.push(Anomaly {
                    channel_name: channel_name.to_string(),
                    data_index: i,
                    time,
                    value,
                    anomaly_type: AnomalyType::StatisticalOutlier { z_score },
                    severity,
                });
            }
        }

        anomalies
    }

    /// Detect sudden spikes or drops
    fn detect_rate_of_change(
        &self,
        channel_name: &str,
        times: &[f64],
        values: &[f64],
        stats: &ChannelStats,
    ) -> Vec<Anomaly> {
        if values.len() < 2 || stats.range < 0.0001 {
            return Vec::new();
        }

        let mut anomalies = Vec::new();
        let threshold_per_second = stats.range * self.config.rate_of_change_threshold;

        for i in 1..values.len() {
            let dt = times[i] - times[i - 1];
            if dt < 0.0001 {
                continue; // Avoid division by near-zero
            }

            let delta = values[i] - values[i - 1];
            let rate = delta.abs() / dt;

            if rate >= threshold_per_second {
                let normalized_rate = rate / stats.range;
                let severity = if normalized_rate >= 2.0 {
                    AnomalySeverity::Critical
                } else if normalized_rate >= 1.0 {
                    AnomalySeverity::Warning
                } else {
                    AnomalySeverity::Info
                };

                anomalies.push(Anomaly {
                    channel_name: channel_name.to_string(),
                    data_index: i,
                    time: times[i],
                    value: values[i],
                    anomaly_type: AnomalyType::RateOfChange {
                        delta,
                        delta_per_second: rate,
                    },
                    severity,
                });
            }
        }

        anomalies
    }

    /// Detect sensor flatlines (stuck values)
    fn detect_flatlines(
        &self,
        channel_name: &str,
        times: &[f64],
        values: &[f64],
    ) -> Vec<Anomaly> {
        if values.len() < 2 {
            return Vec::new();
        }

        let mut anomalies = Vec::new();
        let min_duration = self.config.flatline_min_duration;
        let epsilon = 0.0001; // Tolerance for "same value"

        let mut flatline_start: Option<usize> = None;
        let mut flatline_value = 0.0;

        for i in 1..values.len() {
            let same_value = (values[i] - values[i - 1]).abs() < epsilon;

            if same_value {
                if flatline_start.is_none() {
                    flatline_start = Some(i - 1);
                    flatline_value = values[i - 1];
                }
            } else if let Some(start_idx) = flatline_start {
                // Flatline ended, check duration
                let duration = times[i - 1] - times[start_idx];
                if duration >= min_duration {
                    let severity = if duration >= min_duration * 3.0 {
                        AnomalySeverity::Critical
                    } else if duration >= min_duration * 1.5 {
                        AnomalySeverity::Warning
                    } else {
                        AnomalySeverity::Info
                    };

                    anomalies.push(Anomaly {
                        channel_name: channel_name.to_string(),
                        data_index: start_idx,
                        time: times[start_idx],
                        value: flatline_value,
                        anomaly_type: AnomalyType::Flatline {
                            duration_seconds: duration,
                            stuck_value: flatline_value,
                        },
                        severity,
                    });
                }
                flatline_start = None;
            }
        }

        // Check if flatline continues to end
        if let Some(start_idx) = flatline_start {
            let duration = times[times.len() - 1] - times[start_idx];
            if duration >= min_duration {
                let severity = if duration >= min_duration * 3.0 {
                    AnomalySeverity::Critical
                } else {
                    AnomalySeverity::Warning
                };

                anomalies.push(Anomaly {
                    channel_name: channel_name.to_string(),
                    data_index: start_idx,
                    time: times[start_idx],
                    value: flatline_value,
                    anomaly_type: AnomalyType::Flatline {
                        duration_seconds: duration,
                        stuck_value: flatline_value,
                    },
                    severity,
                });
            }
        }

        anomalies
    }

    /// Detect values outside known ECU parameter ranges
    fn detect_range_violations(
        &self,
        channel_name: &str,
        times: &[f64],
        values: &[f64],
    ) -> Vec<Anomaly> {
        let Some((expected_min, expected_max)) = get_known_range(channel_name) else {
            return Vec::new();
        };

        let mut anomalies = Vec::new();

        for (i, (&time, &value)) in times.iter().zip(values.iter()).enumerate() {
            if value < expected_min || value > expected_max {
                let deviation = if value < expected_min {
                    expected_min - value
                } else {
                    value - expected_max
                };
                let range = expected_max - expected_min;
                let severity = if deviation > range * 0.5 {
                    AnomalySeverity::Critical
                } else if deviation > range * 0.2 {
                    AnomalySeverity::Warning
                } else {
                    AnomalySeverity::Info
                };

                anomalies.push(Anomaly {
                    channel_name: channel_name.to_string(),
                    data_index: i,
                    time,
                    value,
                    anomaly_type: AnomalyType::RangeViolation {
                        value,
                        expected_min,
                        expected_max,
                    },
                    severity,
                });
            }
        }

        anomalies
    }
}

/// Analyze multiple channels at once
pub fn analyze_all_channels(
    config: &AnomalyConfig,
    channels: &[(String, Vec<f64>, Vec<f64>)], // (name, times, values)
) -> AnomalyResults {
    let detector = AnomalyDetector::new(config.clone());
    let mut results = AnomalyResults::default();

    for (name, times, values) in channels {
        results.channels_analyzed += 1;
        results.points_analyzed += values.len();

        let channel_anomalies = detector.analyze_channel(name, times, values);

        for anomaly in &channel_anomalies {
            match anomaly.severity {
                AnomalySeverity::Info => results.info_count += 1,
                AnomalySeverity::Warning => results.warning_count += 1,
                AnomalySeverity::Critical => results.critical_count += 1,
            }
        }

        results.anomalies.extend(channel_anomalies);
    }

    // Sort all anomalies by time
    results.anomalies.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

    results
}

// ============================================================================
// Helper Types
// ============================================================================

/// Basic statistics for a channel
struct ChannelStats {
    mean: f64,
    std_dev: f64,
    min: f64,
    max: f64,
    range: f64,
}

impl ChannelStats {
    fn calculate(values: &[f64]) -> Self {
        if values.is_empty() {
            return Self {
                mean: 0.0,
                std_dev: 0.0,
                min: 0.0,
                max: 0.0,
                range: 0.0,
            };
        }

        let n = values.len() as f64;
        let sum: f64 = values.iter().sum();
        let mean = sum / n;

        let variance: f64 = values.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();

        let min = values.iter().cloned().fold(f64::MAX, f64::min);
        let max = values.iter().cloned().fold(f64::MIN, f64::max);

        Self {
            mean,
            std_dev,
            min,
            max,
            range: max - min,
        }
    }
}

// ============================================================================
// Known ECU Parameter Ranges
// ============================================================================

/// Get known valid range for common ECU parameters
/// Returns (min, max) if the channel is recognized
fn get_known_range(channel_name: &str) -> Option<(f64, f64)> {
    let name_lower = channel_name.to_lowercase();

    // Match against common patterns
    if name_lower.contains("afr") || name_lower.contains("air fuel") || name_lower.contains("lambda") {
        if name_lower.contains("lambda") {
            Some((0.7, 1.3)) // Lambda
        } else {
            Some((10.0, 20.0)) // AFR (gasoline)
        }
    } else if name_lower.contains("coolant") || name_lower.contains("ect") || name_lower.contains("water temp") {
        Some((-40.0, 130.0)) // Celsius
    } else if name_lower.contains("oil temp") {
        Some((-40.0, 150.0)) // Celsius
    } else if name_lower.contains("oil press") {
        Some((0.0, 150.0)) // PSI
    } else if name_lower.contains("fuel press") {
        Some((0.0, 100.0)) // PSI for returnless
    } else if name_lower.contains("intake") && name_lower.contains("temp") || name_lower.contains("iat") {
        Some((-40.0, 80.0)) // Celsius
    } else if name_lower.contains("boost") || name_lower.contains("map") {
        if name_lower.contains("kpa") {
            Some((0.0, 350.0)) // kPa
        } else {
            Some((-15.0, 50.0)) // PSI
        }
    } else if name_lower.contains("rpm") || name_lower.contains("engine speed") {
        Some((0.0, 12000.0)) // RPM
    } else if name_lower.contains("throttle") || name_lower.contains("tps") {
        Some((0.0, 100.0)) // Percentage
    } else if name_lower.contains("battery") || name_lower.contains("voltage") {
        Some((8.0, 16.0)) // Volts
    } else if name_lower.contains("duty") && name_lower.contains("cycle") {
        Some((0.0, 100.0)) // Percentage
    } else if name_lower.contains("ignition") || name_lower.contains("timing") || name_lower.contains("advance") {
        Some((-10.0, 50.0)) // Degrees
    } else if name_lower.contains("knock") {
        Some((0.0, 20.0)) // Degrees of retard typically
    } else if name_lower.contains("ve") || name_lower.contains("volumetric") {
        Some((0.0, 150.0)) // Percentage (can exceed 100%)
    } else if name_lower.contains("speed") && !name_lower.contains("engine") {
        Some((0.0, 300.0)) // km/h or mph
    } else if name_lower.contains("gear") {
        Some((0.0, 8.0)) // Gear number
    } else {
        None // Unknown channel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_statistical_outlier_detection() {
        let config = AnomalyConfig::default();
        let detector = AnomalyDetector::new(config);

        // Normal data with one outlier
        let times: Vec<f64> = (0..100).map(|i| i as f64 * 0.1).collect();
        let mut values: Vec<f64> = vec![14.7; 100];
        values[50] = 8.0; // Major outlier (lean spike)

        let anomalies = detector.analyze_channel("AFR", &times, &values);
        assert!(!anomalies.is_empty());
        assert!(anomalies.iter().any(|a| a.data_index == 50));
    }

    #[test]
    fn test_flatline_detection() {
        let config = AnomalyConfig {
            flatline_min_duration: 1.0,
            ..Default::default()
        };
        let detector = AnomalyDetector::new(config);

        // Data with flatline
        let times: Vec<f64> = (0..100).map(|i| i as f64 * 0.1).collect();
        let mut values: Vec<f64> = (0..100).map(|i| (i as f64).sin()).collect();
        // Create flatline from index 20-50
        for i in 20..50 {
            values[i] = 0.5;
        }

        let anomalies = detector.analyze_channel("Sensor", &times, &values);
        let flatlines: Vec<_> = anomalies
            .iter()
            .filter(|a| matches!(a.anomaly_type, AnomalyType::Flatline { .. }))
            .collect();

        assert!(!flatlines.is_empty());
    }

    #[test]
    fn test_range_violation() {
        let config = AnomalyConfig::default();
        let detector = AnomalyDetector::new(config);

        let times: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let values = vec![14.7, 14.5, 14.8, 5.0, 14.7, 14.6, 25.0, 14.7, 14.8, 14.7];

        let anomalies = detector.analyze_channel("AFR", &times, &values);
        let violations: Vec<_> = anomalies
            .iter()
            .filter(|a| matches!(a.anomaly_type, AnomalyType::RangeViolation { .. }))
            .collect();

        // Should detect 5.0 and 25.0 as out of range (10.0-20.0 for AFR)
        assert!(violations.len() >= 2);
    }

    #[test]
    fn test_rate_of_change_detection() {
        let config = AnomalyConfig {
            rate_of_change_threshold: 0.3, // 30% of range per second
            ..Default::default()
        };
        let detector = AnomalyDetector::new(config);

        // Data with sudden spike
        let times: Vec<f64> = (0..20).map(|i| i as f64 * 0.1).collect();
        let mut values: Vec<f64> = vec![100.0; 20];
        values[10] = 200.0; // Sudden spike

        let anomalies = detector.analyze_channel("Boost", &times, &values);
        let rate_changes: Vec<_> = anomalies
            .iter()
            .filter(|a| matches!(a.anomaly_type, AnomalyType::RateOfChange { .. }))
            .collect();

        assert!(!rate_changes.is_empty());
    }

    #[test]
    fn test_analyze_all_channels() {
        let config = AnomalyConfig::default();

        let channels = vec![
            (
                "AFR".to_string(),
                vec![0.0, 1.0, 2.0, 3.0, 4.0],
                vec![14.7, 14.5, 8.0, 14.6, 14.7], // Outlier at index 2
            ),
            (
                "RPM".to_string(),
                vec![0.0, 1.0, 2.0, 3.0, 4.0],
                vec![3000.0, 3100.0, 3050.0, 3000.0, 3100.0], // Normal
            ),
        ];

        let results = super::analyze_all_channels(&config, &channels);

        assert_eq!(results.channels_analyzed, 2);
        assert_eq!(results.points_analyzed, 10);
        assert!(!results.anomalies.is_empty());
    }

    #[test]
    fn test_anomaly_results_methods() {
        let mut results = AnomalyResults::default();
        results.anomalies = vec![
            Anomaly {
                channel_name: "AFR".to_string(),
                data_index: 0,
                time: 1.0,
                value: 8.0,
                anomaly_type: AnomalyType::RangeViolation {
                    value: 8.0,
                    expected_min: 10.0,
                    expected_max: 20.0,
                },
                severity: AnomalySeverity::Critical,
            },
            Anomaly {
                channel_name: "Boost".to_string(),
                data_index: 5,
                time: 5.0,
                value: 30.0,
                anomaly_type: AnomalyType::StatisticalOutlier { z_score: 4.0 },
                severity: AnomalySeverity::Warning,
            },
            Anomaly {
                channel_name: "AFR".to_string(),
                data_index: 10,
                time: 10.0,
                value: 22.0,
                anomaly_type: AnomalyType::RangeViolation {
                    value: 22.0,
                    expected_min: 10.0,
                    expected_max: 20.0,
                },
                severity: AnomalySeverity::Info,
            },
        ];
        results.critical_count = 1;
        results.warning_count = 1;
        results.info_count = 1;

        // Test for_channel
        let afr_anomalies = results.for_channel("AFR");
        assert_eq!(afr_anomalies.len(), 2);

        // Test in_time_range
        let range_anomalies = results.in_time_range(0.0, 6.0);
        assert_eq!(range_anomalies.len(), 2);

        // Test top_anomalies (sorted by severity)
        let top = results.top_anomalies(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].severity, AnomalySeverity::Critical);
    }

    #[test]
    fn test_known_ranges() {
        // Test that known ranges return expected values
        assert!(get_known_range("AFR").is_some());
        assert!(get_known_range("Lambda").is_some());
        assert!(get_known_range("Coolant Temp").is_some());
        assert!(get_known_range("RPM").is_some());
        assert!(get_known_range("Throttle Position").is_some());
        assert!(get_known_range("Battery Voltage").is_some());
        assert!(get_known_range("Unknown Channel XYZ").is_none());
    }

    #[test]
    fn test_anomaly_display_and_prompt_strings() {
        let anomaly = Anomaly {
            channel_name: "AFR".to_string(),
            data_index: 5,
            time: 2.5,
            value: 8.0,
            anomaly_type: AnomalyType::RangeViolation {
                value: 8.0,
                expected_min: 10.0,
                expected_max: 20.0,
            },
            severity: AnomalySeverity::Critical,
        };

        let display = anomaly.display_string();
        assert!(display.contains("AFR"));
        assert!(display.contains("2.50s"));

        let prompt = anomaly.prompt_string();
        assert!(prompt.contains("CRITICAL"));
        assert!(prompt.contains("AFR"));
    }

    #[test]
    fn test_anomaly_results_prompt_summary() {
        let mut results = AnomalyResults::default();
        results.channels_analyzed = 3;
        results.points_analyzed = 1000;
        results.critical_count = 1;
        results.warning_count = 2;
        results.info_count = 3;
        results.anomalies = vec![Anomaly {
            channel_name: "Test".to_string(),
            data_index: 0,
            time: 1.0,
            value: 100.0,
            anomaly_type: AnomalyType::StatisticalOutlier { z_score: 5.0 },
            severity: AnomalySeverity::Critical,
        }];

        let summary = results.to_prompt_summary();
        assert!(summary.contains("3 channels"));
        assert!(summary.contains("1000 data points"));
        assert!(summary.contains("1 critical"));
    }
}
