//! Statistical analysis algorithms.
//!
//! Provides statistical measures, correlation analysis, and data characterization.

use super::*;
use std::collections::HashMap;

/// Descriptive statistics analyzer
#[derive(Clone)]
pub struct DescriptiveStatsAnalyzer {
    /// Channel to analyze
    pub channel: String,
}

impl Default for DescriptiveStatsAnalyzer {
    fn default() -> Self {
        Self {
            channel: "RPM".to_string(),
        }
    }
}

impl Analyzer for DescriptiveStatsAnalyzer {
    fn id(&self) -> &str {
        "descriptive_stats"
    }

    fn name(&self) -> &str {
        "Descriptive Statistics"
    }

    fn description(&self) -> &str {
        "Computes basic statistics: mean, median, standard deviation, min, max, \
         range, and coefficient of variation for a channel."
    }

    fn category(&self) -> &str {
        "Statistics"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let data = require_channel(log, &self.channel)?;
        require_min_length(&data, 2)?;

        let (stats, computation_time) = timed_analyze(|| compute_descriptive_stats(&data));

        let mut warnings = vec![];

        // Warn about high coefficient of variation
        if stats.cv > 50.0 {
            warnings.push(format!(
                "High variability detected (CV={:.1}%) - signal may be noisy",
                stats.cv
            ));
        }

        // Create a "normalized" version of the data for visualization (z-scores)
        let z_scores: Vec<f64> = data
            .iter()
            .map(|&x| (x - stats.mean) / stats.stdev.max(0.001))
            .collect();

        Ok(AnalysisResult {
            name: format!("{} Z-Score", self.channel),
            unit: "σ".to_string(),
            values: z_scores,
            metadata: AnalysisMetadata {
                algorithm: "Descriptive Statistics".to_string(),
                parameters: vec![
                    ("mean".to_string(), format!("{:.4}", stats.mean)),
                    ("median".to_string(), format!("{:.4}", stats.median)),
                    ("stdev".to_string(), format!("{:.4}", stats.stdev)),
                    ("min".to_string(), format!("{:.4}", stats.min)),
                    ("max".to_string(), format!("{:.4}", stats.max)),
                    ("range".to_string(), format!("{:.4}", stats.range)),
                    ("cv".to_string(), format!("{:.2}%", stats.cv)),
                    ("n".to_string(), stats.count.to_string()),
                ],
                warnings,
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("channel".to_string(), self.channel.clone());

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
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

/// Correlation analyzer between two channels
#[derive(Clone)]
pub struct CorrelationAnalyzer {
    /// First channel (X)
    pub channel_x: String,
    /// Second channel (Y)
    pub channel_y: String,
}

impl Default for CorrelationAnalyzer {
    fn default() -> Self {
        Self {
            channel_x: "RPM".to_string(),
            channel_y: "MAP".to_string(),
        }
    }
}

impl Analyzer for CorrelationAnalyzer {
    fn id(&self) -> &str {
        "correlation"
    }

    fn name(&self) -> &str {
        "Channel Correlation"
    }

    fn description(&self) -> &str {
        "Computes Pearson correlation coefficient between two channels. \
         Values near ±1 indicate strong linear relationship."
    }

    fn category(&self) -> &str {
        "Statistics"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.channel_x, &self.channel_y]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let x = require_channel(log, &self.channel_x)?;
        let y = require_channel(log, &self.channel_y)?;

        if x.len() != y.len() {
            return Err(AnalysisError::ComputationError(
                "Channels have different lengths".to_string(),
            ));
        }

        require_min_length(&x, 3)?;

        let (correlation, computation_time) = timed_analyze(|| pearson_correlation(&x, &y));

        let mut warnings = vec![];
        let r = correlation.r;

        // Interpret correlation strength
        let strength = if r.abs() > 0.9 {
            "very strong"
        } else if r.abs() > 0.7 {
            "strong"
        } else if r.abs() > 0.5 {
            "moderate"
        } else if r.abs() > 0.3 {
            "weak"
        } else {
            "very weak/none"
        };

        let direction = if r > 0.0 { "positive" } else { "negative" };

        warnings.push(format!(
            "Correlation is {} {} (r={:.3})",
            strength, direction, r
        ));

        // Create residuals for visualization
        let residuals = compute_residuals(&x, &y);

        Ok(AnalysisResult {
            name: format!("{} vs {} Residuals", self.channel_x, self.channel_y),
            unit: String::new(),
            values: residuals,
            metadata: AnalysisMetadata {
                algorithm: "Pearson Correlation".to_string(),
                parameters: vec![
                    ("r".to_string(), format!("{:.4}", r)),
                    ("r²".to_string(), format!("{:.4}", r * r)),
                    ("channel_x".to_string(), self.channel_x.clone()),
                    ("channel_y".to_string(), self.channel_y.clone()),
                ],
                warnings,
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("channel_x".to_string(), self.channel_x.clone());
        params.insert("channel_y".to_string(), self.channel_y.clone());

        AnalyzerConfig {
            id: self.id().to_string(),
            name: self.name().to_string(),
            parameters: params,
        }
    }

    fn set_config(&mut self, config: &AnalyzerConfig) {
        if let Some(ch) = config.parameters.get("channel_x") {
            self.channel_x = ch.clone();
        }
        if let Some(ch) = config.parameters.get("channel_y") {
            self.channel_y = ch.clone();
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

/// Rate of change analyzer
#[derive(Clone)]
pub struct RateOfChangeAnalyzer {
    /// Channel to analyze
    pub channel: String,
    /// Use time-based derivative (true) or sample-based (false)
    pub time_based: bool,
}

impl Default for RateOfChangeAnalyzer {
    fn default() -> Self {
        Self {
            channel: "RPM".to_string(),
            time_based: true,
        }
    }
}

impl Analyzer for RateOfChangeAnalyzer {
    fn id(&self) -> &str {
        "rate_of_change"
    }

    fn name(&self) -> &str {
        "Rate of Change"
    }

    fn description(&self) -> &str {
        "Computes the derivative (rate of change) of a channel. Time-based mode \
         gives units per second; sample-based gives units per sample."
    }

    fn category(&self) -> &str {
        "Statistics"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let data = require_channel(log, &self.channel)?;
        require_min_length(&data, 2)?;

        let times = log.times();
        if times.len() != data.len() {
            return Err(AnalysisError::ComputationError(
                "Data and time vectors have different lengths".to_string(),
            ));
        }

        let (derivative, computation_time) = timed_analyze(|| {
            if self.time_based {
                time_derivative(&data, times)
            } else {
                sample_derivative(&data)
            }
        });

        // Compute statistics on the derivative
        let stats = compute_descriptive_stats(&derivative);

        let mut warnings = vec![];

        // Warn about high rate of change
        let max_abs_rate = stats.max.abs().max(stats.min.abs());
        if max_abs_rate > stats.stdev * 5.0 {
            warnings.push(format!(
                "Extreme rate of change detected: max |dv/dt| = {:.2}",
                max_abs_rate
            ));
        }

        let unit = if self.time_based { "/s" } else { "/sample" };

        Ok(AnalysisResult {
            name: format!("d({})/dt", self.channel),
            unit: unit.to_string(),
            values: derivative,
            metadata: AnalysisMetadata {
                algorithm: if self.time_based {
                    "Time-based Derivative"
                } else {
                    "Sample-based Derivative"
                }
                .to_string(),
                parameters: vec![
                    ("channel".to_string(), self.channel.clone()),
                    ("mean_rate".to_string(), format!("{:.4}", stats.mean)),
                    ("max_rate".to_string(), format!("{:.4}", stats.max)),
                    ("min_rate".to_string(), format!("{:.4}", stats.min)),
                ],
                warnings,
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("channel".to_string(), self.channel.clone());
        params.insert("time_based".to_string(), self.time_based.to_string());

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
        if let Some(tb) = config.parameters.get("time_based") {
            self.time_based = tb.parse().unwrap_or(true);
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

// ============================================================================
// Core statistics implementations
// ============================================================================

/// Container for descriptive statistics
#[derive(Clone, Debug, Default)]
pub struct DescriptiveStats {
    pub count: usize,
    pub mean: f64,
    pub median: f64,
    pub stdev: f64,
    pub min: f64,
    pub max: f64,
    pub range: f64,
    pub cv: f64, // Coefficient of variation (%)
}

/// Compute descriptive statistics for a dataset
pub fn compute_descriptive_stats(data: &[f64]) -> DescriptiveStats {
    if data.is_empty() {
        return DescriptiveStats::default();
    }

    let n = data.len();

    // Mean (Welford's algorithm for numerical stability)
    let mean = data.iter().sum::<f64>() / n as f64;

    // Variance (two-pass for stability)
    let variance = data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1).max(1) as f64;
    let stdev = variance.sqrt();

    // Min/Max
    let min = data.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    // Median (requires sorting)
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    #[allow(clippy::manual_is_multiple_of)]
    let median = if n % 2 == 0 {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    };

    // Coefficient of variation
    let cv = if mean.abs() > f64::EPSILON {
        (stdev / mean.abs()) * 100.0
    } else {
        0.0
    };

    DescriptiveStats {
        count: n,
        mean,
        median,
        stdev,
        min,
        max,
        range: max - min,
        cv,
    }
}

/// Container for correlation results
#[derive(Clone, Debug, Default)]
pub struct CorrelationResult {
    pub r: f64,         // Pearson correlation coefficient
    pub r_squared: f64, // Coefficient of determination
}

/// Compute Pearson correlation coefficient
pub fn pearson_correlation(x: &[f64], y: &[f64]) -> CorrelationResult {
    if x.len() != y.len() || x.len() < 2 {
        return CorrelationResult::default();
    }

    let n = x.len() as f64;
    let mean_x = x.iter().sum::<f64>() / n;
    let mean_y = y.iter().sum::<f64>() / n;

    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;

    for i in 0..x.len() {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    let denom = (var_x * var_y).sqrt();
    let r = if denom > f64::EPSILON {
        cov / denom
    } else {
        0.0
    };

    CorrelationResult {
        r,
        r_squared: r * r,
    }
}

/// Compute residuals from linear regression
pub fn compute_residuals(x: &[f64], y: &[f64]) -> Vec<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return vec![];
    }

    let n = x.len() as f64;
    let mean_x = x.iter().sum::<f64>() / n;
    let mean_y = y.iter().sum::<f64>() / n;

    // Compute slope and intercept
    let mut num = 0.0;
    let mut den = 0.0;
    for i in 0..x.len() {
        let dx = x[i] - mean_x;
        num += dx * (y[i] - mean_y);
        den += dx * dx;
    }

    let slope = if den.abs() > f64::EPSILON {
        num / den
    } else {
        0.0
    };
    let intercept = mean_y - slope * mean_x;

    // Compute residuals
    x.iter()
        .zip(y.iter())
        .map(|(&xi, &yi)| yi - (slope * xi + intercept))
        .collect()
}

/// Compute time-based derivative using central differences
pub fn time_derivative(data: &[f64], times: &[f64]) -> Vec<f64> {
    if data.len() < 2 || times.len() != data.len() {
        return vec![0.0; data.len()];
    }

    let mut result = Vec::with_capacity(data.len());

    // Forward difference for first point
    let dt = times[1] - times[0];
    if dt.abs() > f64::EPSILON {
        result.push((data[1] - data[0]) / dt);
    } else {
        result.push(0.0);
    }

    // Central differences for interior points
    for i in 1..data.len() - 1 {
        let dt = times[i + 1] - times[i - 1];
        if dt.abs() > f64::EPSILON {
            result.push((data[i + 1] - data[i - 1]) / dt);
        } else {
            result.push(0.0);
        }
    }

    // Backward difference for last point
    let dt = times[data.len() - 1] - times[data.len() - 2];
    if dt.abs() > f64::EPSILON {
        result.push((data[data.len() - 1] - data[data.len() - 2]) / dt);
    } else {
        result.push(0.0);
    }

    result
}

/// Compute sample-based derivative (simple differences)
pub fn sample_derivative(data: &[f64]) -> Vec<f64> {
    if data.len() < 2 {
        return vec![0.0; data.len()];
    }

    let mut result = Vec::with_capacity(data.len());

    // Forward difference for first point
    result.push(data[1] - data[0]);

    // Central differences for interior points
    for i in 1..data.len() - 1 {
        result.push((data[i + 1] - data[i - 1]) / 2.0);
    }

    // Backward difference for last point
    result.push(data[data.len() - 1] - data[data.len() - 2]);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_descriptive_stats() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = compute_descriptive_stats(&data);

        assert_eq!(stats.count, 5);
        assert!((stats.mean - 3.0).abs() < 0.001);
        assert!((stats.median - 3.0).abs() < 0.001);
        assert!((stats.min - 1.0).abs() < 0.001);
        assert!((stats.max - 5.0).abs() < 0.001);
        assert!((stats.range - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_pearson_correlation_perfect() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0]; // Perfect positive correlation
        let result = pearson_correlation(&x, &y);

        assert!((result.r - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_pearson_correlation_negative() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![10.0, 8.0, 6.0, 4.0, 2.0]; // Perfect negative correlation
        let result = pearson_correlation(&x, &y);

        assert!((result.r + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_time_derivative() {
        let data = vec![0.0, 1.0, 4.0, 9.0, 16.0]; // y = x^2 at x=0,1,2,3,4
        let times = vec![0.0, 1.0, 2.0, 3.0, 4.0];
        let derivative = time_derivative(&data, &times);

        // dy/dx = 2x, so at x=2, derivative should be ~4
        assert!((derivative[2] - 4.0).abs() < 0.001);
    }
}
