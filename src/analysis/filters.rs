//! Filter-based analysis algorithms.
//!
//! Provides signal processing filters for smoothing, noise reduction,
//! and data conditioning.

use super::*;
use std::collections::{HashMap, VecDeque};

/// Moving Average filter analyzer
#[derive(Clone)]
pub struct MovingAverageAnalyzer {
    /// Channel to filter
    pub channel: String,
    /// Window size for averaging
    pub window_size: usize,
}

impl Default for MovingAverageAnalyzer {
    fn default() -> Self {
        Self {
            channel: "RPM".to_string(),
            window_size: 5,
        }
    }
}

impl Analyzer for MovingAverageAnalyzer {
    fn id(&self) -> &str {
        "moving_average"
    }

    fn name(&self) -> &str {
        "Moving Average"
    }

    fn description(&self) -> &str {
        "Simple moving average filter for smoothing noisy signals. \
         Averages the last N samples to reduce high-frequency noise."
    }

    fn category(&self) -> &str {
        "Filters"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let data = require_channel(log, &self.channel)?;
        require_min_length(&data, self.window_size)?;

        let (values, computation_time) = timed_analyze(|| moving_average(&data, self.window_size));

        Ok(AnalysisResult {
            name: format!("{} (MA{})", self.channel, self.window_size),
            unit: String::new(), // Same unit as input
            values,
            metadata: AnalysisMetadata {
                algorithm: "Simple Moving Average".to_string(),
                parameters: vec![
                    ("window_size".to_string(), self.window_size.to_string()),
                    ("channel".to_string(), self.channel.clone()),
                ],
                warnings: vec![],
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("channel".to_string(), self.channel.clone());
        params.insert("window_size".to_string(), self.window_size.to_string());

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
        if let Some(ws) = config.parameters.get("window_size") {
            if let Ok(size) = ws.parse() {
                self.window_size = size;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

/// Exponential Moving Average filter analyzer
#[derive(Clone)]
pub struct ExponentialMovingAverageAnalyzer {
    /// Channel to filter
    pub channel: String,
    /// Smoothing factor alpha (0 < alpha <= 1)
    /// Higher alpha = more weight to recent values
    pub alpha: f64,
}

impl Default for ExponentialMovingAverageAnalyzer {
    fn default() -> Self {
        Self {
            channel: "RPM".to_string(),
            alpha: 0.2, // Equivalent to ~9 period SMA
        }
    }
}

impl Analyzer for ExponentialMovingAverageAnalyzer {
    fn id(&self) -> &str {
        "exponential_moving_average"
    }

    fn name(&self) -> &str {
        "Exponential Moving Average"
    }

    fn description(&self) -> &str {
        "Exponentially weighted moving average filter. More recent samples have \
         higher weight. Alpha parameter controls smoothing (0.1=heavy, 0.5=light)."
    }

    fn category(&self) -> &str {
        "Filters"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let data = require_channel(log, &self.channel)?;
        require_min_length(&data, 2)?;

        if self.alpha <= 0.0 || self.alpha > 1.0 {
            return Err(AnalysisError::InvalidParameter(
                "Alpha must be between 0 and 1".to_string(),
            ));
        }

        let (values, computation_time) =
            timed_analyze(|| exponential_moving_average(&data, self.alpha));

        Ok(AnalysisResult {
            name: format!("{} (EMA α={:.2})", self.channel, self.alpha),
            unit: String::new(),
            values,
            metadata: AnalysisMetadata {
                algorithm: "Exponential Moving Average".to_string(),
                parameters: vec![
                    ("alpha".to_string(), format!("{:.3}", self.alpha)),
                    ("channel".to_string(), self.channel.clone()),
                ],
                warnings: vec![],
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("channel".to_string(), self.channel.clone());
        params.insert("alpha".to_string(), self.alpha.to_string());

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
        if let Some(a) = config.parameters.get("alpha") {
            if let Ok(alpha) = a.parse() {
                self.alpha = alpha;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

/// Median filter analyzer
#[derive(Clone)]
pub struct MedianFilterAnalyzer {
    /// Channel to filter
    pub channel: String,
    /// Window size (must be odd for symmetric window)
    pub window_size: usize,
}

impl Default for MedianFilterAnalyzer {
    fn default() -> Self {
        Self {
            channel: "RPM".to_string(),
            window_size: 5,
        }
    }
}

impl Analyzer for MedianFilterAnalyzer {
    fn id(&self) -> &str {
        "median_filter"
    }

    fn name(&self) -> &str {
        "Median Filter"
    }

    fn description(&self) -> &str {
        "Median filter for removing impulse noise (spikes). Replaces each value \
         with the median of neighboring samples. Preserves edges better than averaging."
    }

    fn category(&self) -> &str {
        "Filters"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let data = require_channel(log, &self.channel)?;
        require_min_length(&data, self.window_size)?;

        // Ensure odd window size
        #[allow(clippy::manual_is_multiple_of)]
        let window = if self.window_size % 2 == 0 {
            self.window_size + 1
        } else {
            self.window_size
        };

        let (values, computation_time) = timed_analyze(|| median_filter(&data, window));

        let mut warnings = vec![];
        #[allow(clippy::manual_is_multiple_of)]
        if self.window_size % 2 == 0 {
            warnings.push(format!(
                "Window size adjusted from {} to {} (must be odd)",
                self.window_size, window
            ));
        }

        Ok(AnalysisResult {
            name: format!("{} (Median{})", self.channel, window),
            unit: String::new(),
            values,
            metadata: AnalysisMetadata {
                algorithm: "Median Filter".to_string(),
                parameters: vec![
                    ("window_size".to_string(), window.to_string()),
                    ("channel".to_string(), self.channel.clone()),
                ],
                warnings,
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("channel".to_string(), self.channel.clone());
        params.insert("window_size".to_string(), self.window_size.to_string());

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
        if let Some(ws) = config.parameters.get("window_size") {
            if let Ok(size) = ws.parse() {
                self.window_size = size;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

// ============================================================================
// Core filter implementations
// ============================================================================

/// Simple moving average filter
pub fn moving_average(data: &[f64], window_size: usize) -> Vec<f64> {
    if data.is_empty() || window_size == 0 {
        return data.to_vec();
    }

    let mut result = Vec::with_capacity(data.len());
    let mut sum: f64 = 0.0;
    let mut window: VecDeque<f64> = VecDeque::with_capacity(window_size);

    for &value in data {
        window.push_back(value);
        sum += value;

        if window.len() > window_size {
            sum -= window.pop_front().unwrap();
        }

        result.push(sum / window.len() as f64);
    }

    result
}

/// Exponential moving average filter
pub fn exponential_moving_average(data: &[f64], alpha: f64) -> Vec<f64> {
    if data.is_empty() {
        return vec![];
    }

    let mut result = Vec::with_capacity(data.len());
    let mut ema = data[0];

    for &value in data {
        ema = alpha * value + (1.0 - alpha) * ema;
        result.push(ema);
    }

    result
}

/// Median filter
pub fn median_filter(data: &[f64], window_size: usize) -> Vec<f64> {
    if data.is_empty() || window_size == 0 {
        return data.to_vec();
    }

    let half_window = window_size / 2;
    let mut result = Vec::with_capacity(data.len());

    for i in 0..data.len() {
        let start = i.saturating_sub(half_window);
        let end = (i + half_window + 1).min(data.len());

        let mut window: Vec<f64> = data[start..end].to_vec();
        window.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        #[allow(clippy::manual_is_multiple_of)]
        let median = if window.len() % 2 == 0 {
            (window[window.len() / 2 - 1] + window[window.len() / 2]) / 2.0
        } else {
            window[window.len() / 2]
        };

        result.push(median);
    }

    result
}

/// Butterworth lowpass filter analyzer
///
/// Implements a digital Butterworth lowpass filter with configurable
/// cutoff frequency and filter order. Uses forward-backward filtering
/// for zero phase distortion.
#[derive(Clone)]
pub struct ButterworthLowpassAnalyzer {
    /// Channel to filter
    pub channel: String,
    /// Cutoff frequency as fraction of sample rate (0.0 to 0.5)
    /// e.g., 0.1 = cutoff at 10% of Nyquist frequency
    pub cutoff_normalized: f64,
    /// Filter order (1-4 recommended)
    pub order: usize,
}

impl Default for ButterworthLowpassAnalyzer {
    fn default() -> Self {
        Self {
            channel: "RPM".to_string(),
            cutoff_normalized: 0.1, // 10% of Nyquist
            order: 2,
        }
    }
}

impl Analyzer for ButterworthLowpassAnalyzer {
    fn id(&self) -> &str {
        "butterworth_lowpass"
    }

    fn name(&self) -> &str {
        "Butterworth Lowpass"
    }

    fn description(&self) -> &str {
        "Butterworth lowpass filter with maximally flat passband response. \
         Uses zero-phase filtering (filtfilt) to eliminate phase distortion. \
         Cutoff is normalized (0-0.5, where 0.5 = Nyquist frequency)."
    }

    fn category(&self) -> &str {
        "Filters"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let data = require_channel(log, &self.channel)?;
        require_min_length(&data, 10)?;

        if self.cutoff_normalized <= 0.0 || self.cutoff_normalized >= 0.5 {
            return Err(AnalysisError::InvalidParameter(
                "Cutoff must be between 0 and 0.5 (Nyquist)".to_string(),
            ));
        }

        if self.order < 1 || self.order > 8 {
            return Err(AnalysisError::InvalidParameter(
                "Order must be between 1 and 8".to_string(),
            ));
        }

        let (values, computation_time) = timed_analyze(|| {
            butterworth_lowpass_filtfilt(&data, self.cutoff_normalized, self.order)
        });

        let mut warnings = vec![];
        if self.order > 4 {
            warnings.push("High filter orders (>4) may cause numerical instability".to_string());
        }

        // Estimate actual cutoff frequency if we know sample rate
        let times = log.times();
        let sample_rate_hint = if times.len() >= 2 {
            1.0 / (times[1] - times[0]).max(0.001)
        } else {
            0.0
        };

        let mut params = vec![
            (
                "cutoff_normalized".to_string(),
                format!("{:.3}", self.cutoff_normalized),
            ),
            ("order".to_string(), self.order.to_string()),
            ("channel".to_string(), self.channel.clone()),
        ];

        if sample_rate_hint > 0.0 {
            let cutoff_hz = self.cutoff_normalized * sample_rate_hint;
            params.push((
                "cutoff_hz_approx".to_string(),
                format!("{:.1} Hz", cutoff_hz),
            ));
        }

        Ok(AnalysisResult {
            name: format!("{} (Butter{})", self.channel, self.order),
            unit: String::new(),
            values,
            metadata: AnalysisMetadata {
                algorithm: "Butterworth Lowpass (filtfilt)".to_string(),
                parameters: params,
                warnings,
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("channel".to_string(), self.channel.clone());
        params.insert(
            "cutoff_normalized".to_string(),
            self.cutoff_normalized.to_string(),
        );
        params.insert("order".to_string(), self.order.to_string());

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
        if let Some(c) = config.parameters.get("cutoff_normalized") {
            if let Ok(cutoff) = c.parse() {
                self.cutoff_normalized = cutoff;
            }
        }
        if let Some(o) = config.parameters.get("order") {
            if let Ok(order) = o.parse() {
                self.order = order;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

/// Butterworth highpass filter analyzer
#[derive(Clone)]
pub struct ButterworthHighpassAnalyzer {
    /// Channel to filter
    pub channel: String,
    /// Cutoff frequency as fraction of sample rate (0.0 to 0.5)
    pub cutoff_normalized: f64,
    /// Filter order (1-4 recommended)
    pub order: usize,
}

impl Default for ButterworthHighpassAnalyzer {
    fn default() -> Self {
        Self {
            channel: "RPM".to_string(),
            cutoff_normalized: 0.05, // 5% of Nyquist
            order: 2,
        }
    }
}

impl Analyzer for ButterworthHighpassAnalyzer {
    fn id(&self) -> &str {
        "butterworth_highpass"
    }

    fn name(&self) -> &str {
        "Butterworth Highpass"
    }

    fn description(&self) -> &str {
        "Butterworth highpass filter for removing low-frequency drift and DC offset. \
         Uses zero-phase filtering (filtfilt) to eliminate phase distortion."
    }

    fn category(&self) -> &str {
        "Filters"
    }

    fn required_channels(&self) -> Vec<&str> {
        vec![&self.channel]
    }

    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError> {
        let data = require_channel(log, &self.channel)?;
        require_min_length(&data, 10)?;

        if self.cutoff_normalized <= 0.0 || self.cutoff_normalized >= 0.5 {
            return Err(AnalysisError::InvalidParameter(
                "Cutoff must be between 0 and 0.5 (Nyquist)".to_string(),
            ));
        }

        if self.order < 1 || self.order > 8 {
            return Err(AnalysisError::InvalidParameter(
                "Order must be between 1 and 8".to_string(),
            ));
        }

        let (values, computation_time) = timed_analyze(|| {
            butterworth_highpass_filtfilt(&data, self.cutoff_normalized, self.order)
        });

        Ok(AnalysisResult {
            name: format!("{} (HP{})", self.channel, self.order),
            unit: String::new(),
            values,
            metadata: AnalysisMetadata {
                algorithm: "Butterworth Highpass (filtfilt)".to_string(),
                parameters: vec![
                    (
                        "cutoff_normalized".to_string(),
                        format!("{:.3}", self.cutoff_normalized),
                    ),
                    ("order".to_string(), self.order.to_string()),
                    ("channel".to_string(), self.channel.clone()),
                ],
                warnings: vec![],
                computation_time_ms: computation_time,
            },
        })
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut params = HashMap::new();
        params.insert("channel".to_string(), self.channel.clone());
        params.insert(
            "cutoff_normalized".to_string(),
            self.cutoff_normalized.to_string(),
        );
        params.insert("order".to_string(), self.order.to_string());

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
        if let Some(c) = config.parameters.get("cutoff_normalized") {
            if let Ok(cutoff) = c.parse() {
                self.cutoff_normalized = cutoff;
            }
        }
        if let Some(o) = config.parameters.get("order") {
            if let Ok(order) = o.parse() {
                self.order = order;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn Analyzer> {
        Box::new(self.clone())
    }
}

// ============================================================================
// Butterworth filter implementation (Professional-grade)
//
// This implementation uses:
// - Second-Order Sections (SOS) for numerical stability
// - Proper bilinear transform with frequency pre-warping
// - Correct pole placement from Butterworth prototype
// - Edge padding with reflection to reduce transient artifacts
// - Steady-state initial conditions (lfilter_zi equivalent)
// - Support for orders 1-8
// ============================================================================

use std::f64::consts::PI;

/// A second-order section (biquad) filter
/// Transfer function: H(z) = (b0 + b1*z^-1 + b2*z^-2) / (1 + a1*z^-1 + a2*z^-2)
#[derive(Clone, Debug)]
struct Sos {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Sos {
    /// Apply this biquad section to data using Direct Form II Transposed
    /// This form has better numerical properties than Direct Form I
    fn filter(&self, data: &[f64], zi: Option<[f64; 2]>) -> (Vec<f64>, [f64; 2]) {
        let n = data.len();
        let mut output = Vec::with_capacity(n);

        // State variables (delay elements)
        let mut z1 = zi.map(|z| z[0]).unwrap_or(0.0);
        let mut z2 = zi.map(|z| z[1]).unwrap_or(0.0);

        for &x in data {
            // Direct Form II Transposed
            let y = self.b0 * x + z1;
            z1 = self.b1 * x - self.a1 * y + z2;
            z2 = self.b2 * x - self.a2 * y;
            output.push(y);
        }

        (output, [z1, z2])
    }

    /// Compute steady-state initial conditions for step response
    /// This is equivalent to scipy's lfilter_zi
    fn compute_zi(&self, x0: f64) -> [f64; 2] {
        // For a step input of value x0, compute the initial state
        // that would make the output also be x0 (steady state)
        //
        // From: y[n] = b0*x[n] + z1[n-1]
        //       z1[n] = b1*x[n] - a1*y[n] + z2[n-1]
        //       z2[n] = b2*x[n] - a2*y[n]
        //
        // At steady state with x[n] = y[n] = x0:
        //   z2 = (b2 - a2) * x0
        //   z1 = (b1 - a1) * x0 + z2 = (b1 - a1 + b2 - a2) * x0

        let z2 = (self.b2 - self.a2) * x0;
        let z1 = (self.b1 - self.a1) * x0 + z2;
        [z1, z2]
    }
}

/// Generate Butterworth lowpass second-order sections
///
/// Uses the standard approach:
/// 1. Compute analog prototype poles on unit circle
/// 2. Apply bilinear transform with frequency pre-warping
/// 3. Return cascaded biquad sections
fn butterworth_lowpass_sos(cutoff: f64, order: usize) -> Vec<Sos> {
    if order == 0 {
        return vec![];
    }

    // Pre-warp the cutoff frequency for bilinear transform
    // wc = tan(pi * cutoff) where cutoff is normalized frequency (0 to 0.5)
    let wc = (PI * cutoff).tan();

    let mut sections = Vec::new();

    // For odd orders, we have one first-order section
    if order % 2 == 1 {
        // First-order section from real pole at s = -1
        // Bilinear transform: s = (1 - z^-1) / (1 + z^-1) * (1/wc)
        // H(s) = wc / (s + wc) -> H(z)

        let k = wc / (1.0 + wc);
        sections.push(Sos {
            b0: k,
            b1: k,
            b2: 0.0,
            a1: (wc - 1.0) / (wc + 1.0),
            a2: 0.0,
        });
    }

    // Second-order sections from complex conjugate pole pairs
    let num_biquads = order / 2;
    for i in 0..num_biquads {
        // Butterworth poles are evenly spaced on the left half of the unit circle
        // For order n, pole angles are: theta_k = pi * (2k + n + 1) / (2n)
        // For the k-th biquad (0-indexed), we use poles k and (n-1-k)

        // Pole angle for this biquad section
        // Using: theta = pi * (2*i + 1 + (order % 2)) / (2 * order) + pi/2
        let k_index = i as f64;
        let theta = PI * (2.0 * k_index + 1.0 + (order % 2) as f64) / (2.0 * order as f64);

        // Analog prototype pole: p = -sin(theta) + j*cos(theta) (on unit circle)
        // For Butterworth, the damping factor relates to theta
        // Q = 1 / (2 * cos(theta)) but we use the direct pole representation

        // The second-order analog section is:
        // H(s) = wc^2 / (s^2 + 2*zeta*wc*s + wc^2)
        // where zeta = sin(theta) = cos(pi/2 - theta)

        let zeta = theta.sin(); // damping ratio for this section

        // Bilinear transform of second-order lowpass section
        // Using s = (2/T) * (1 - z^-1) / (1 + z^-1), with T = 2 (normalized)
        // and pre-warped wc

        let wc2 = wc * wc;
        let two_zeta_wc = 2.0 * zeta * wc;

        // Denominator: s^2 + 2*zeta*wc*s + wc^2
        // After bilinear: (1 + a1*z^-1 + a2*z^-2) * norm
        let denom = 1.0 + two_zeta_wc + wc2;

        let a1 = 2.0 * (wc2 - 1.0) / denom;
        let a2 = (1.0 - two_zeta_wc + wc2) / denom;

        // Numerator: wc^2
        // After bilinear: (b0 + b1*z^-1 + b2*z^-2)
        let b0 = wc2 / denom;
        let b1 = 2.0 * wc2 / denom;
        let b2 = wc2 / denom;

        sections.push(Sos { b0, b1, b2, a1, a2 });
    }

    sections
}

/// Generate Butterworth highpass second-order sections
///
/// Uses the lowpass-to-highpass transformation in the analog domain:
/// s -> wc/s, then applies bilinear transform
fn butterworth_highpass_sos(cutoff: f64, order: usize) -> Vec<Sos> {
    if order == 0 {
        return vec![];
    }

    // Pre-warp the cutoff frequency
    let wc = (PI * cutoff).tan();

    let mut sections = Vec::new();

    // For odd orders, we have one first-order section
    if order % 2 == 1 {
        // First-order highpass: H(s) = s / (s + wc)
        // After bilinear transform:
        let k = 1.0 / (1.0 + wc);
        sections.push(Sos {
            b0: k,
            b1: -k,
            b2: 0.0,
            a1: (wc - 1.0) / (wc + 1.0),
            a2: 0.0,
        });
    }

    // Second-order sections
    let num_biquads = order / 2;
    for i in 0..num_biquads {
        let k_index = i as f64;
        let theta = PI * (2.0 * k_index + 1.0 + (order % 2) as f64) / (2.0 * order as f64);
        let zeta = theta.sin();

        // Highpass second-order section: H(s) = s^2 / (s^2 + 2*zeta*wc*s + wc^2)
        // After bilinear transform:

        let wc2 = wc * wc;
        let two_zeta_wc = 2.0 * zeta * wc;
        let denom = 1.0 + two_zeta_wc + wc2;

        let a1 = 2.0 * (wc2 - 1.0) / denom;
        let a2 = (1.0 - two_zeta_wc + wc2) / denom;

        // Numerator for highpass: s^2 -> (1 - z^-1)^2 / (1 + z^-1)^2 after bilinear
        // Normalized: (1 - 2*z^-1 + z^-2) / denom
        let norm = 1.0 / denom;
        let b0 = norm;
        let b1 = -2.0 * norm;
        let b2 = norm;

        sections.push(Sos { b0, b1, b2, a1, a2 });
    }

    sections
}

/// Apply a cascade of SOS sections to data
#[allow(dead_code)]
fn sos_filter(data: &[f64], sos: &[Sos], zi: Option<&[[f64; 2]]>) -> Vec<f64> {
    if data.is_empty() || sos.is_empty() {
        return data.to_vec();
    }

    let mut result = data.to_vec();

    for (i, section) in sos.iter().enumerate() {
        let initial = zi.and_then(|z| z.get(i).copied());
        let (filtered, _) = section.filter(&result, initial);
        result = filtered;
    }

    result
}

/// Compute initial conditions for steady-state filtering
fn sos_compute_zi(sos: &[Sos], x0: f64) -> Vec<[f64; 2]> {
    sos.iter().map(|s| s.compute_zi(x0)).collect()
}

/// Reflect-pad the signal to reduce edge transients
/// Pads with reflected values at both ends
fn reflect_pad(data: &[f64], pad_len: usize) -> Vec<f64> {
    if data.len() < 2 {
        return data.to_vec();
    }

    let n = data.len();
    let pad_len = pad_len.min(n - 1); // Can't pad more than data length - 1

    let mut padded = Vec::with_capacity(n + 2 * pad_len);

    // Left padding: reflect about first element
    // data[0] - (data[pad_len] - data[0]), data[0] - (data[pad_len-1] - data[0]), ...
    for i in (1..=pad_len).rev() {
        padded.push(2.0 * data[0] - data[i]);
    }

    // Original data
    padded.extend_from_slice(data);

    // Right padding: reflect about last element
    for i in 1..=pad_len {
        let idx = n - 1 - i;
        padded.push(2.0 * data[n - 1] - data[idx]);
    }

    padded
}

/// Zero-phase filtering using forward-backward filtering with edge padding
/// This is equivalent to scipy's filtfilt
fn sosfiltfilt(data: &[f64], sos: &[Sos]) -> Vec<f64> {
    if data.is_empty() || sos.is_empty() {
        return data.to_vec();
    }

    // Check for NaN/Inf in input
    if data.iter().any(|&x| !x.is_finite()) {
        // Replace non-finite values with interpolated values or zeros
        let cleaned: Vec<f64> = data
            .iter()
            .map(|&x| if x.is_finite() { x } else { 0.0 })
            .collect();
        return sosfiltfilt(&cleaned, sos);
    }

    let n = data.len();

    // Padding length: 3 * max(len(a), len(b)) per scipy, which is 3*3 = 9 per section
    // For cascaded sections, use 3 * order
    let pad_len = (3 * sos.len() * 2).min(n - 1).max(1);

    // Pad the signal
    let padded = reflect_pad(data, pad_len);

    // Compute initial conditions based on the padded edge value
    let zi_forward = sos_compute_zi(sos, padded[0]);

    // Forward pass with initial conditions
    let mut forward = padded.clone();
    for (i, section) in sos.iter().enumerate() {
        let (filtered, _) = section.filter(&forward, Some(zi_forward[i]));
        forward = filtered;
    }

    // Reverse
    forward.reverse();

    // Compute initial conditions for backward pass
    let zi_backward = sos_compute_zi(sos, forward[0]);

    // Backward pass with initial conditions
    let mut backward = forward;
    for (i, section) in sos.iter().enumerate() {
        let (filtered, _) = section.filter(&backward, Some(zi_backward[i]));
        backward = filtered;
    }

    // Reverse back
    backward.reverse();

    // Remove padding
    backward[pad_len..pad_len + n].to_vec()
}

/// Butterworth lowpass filter with zero-phase filtering
///
/// Uses second-order sections for numerical stability and
/// forward-backward filtering to eliminate phase distortion.
///
/// # Arguments
/// * `data` - Input signal
/// * `cutoff` - Normalized cutoff frequency (0 < cutoff < 0.5, where 0.5 = Nyquist)
/// * `order` - Filter order (1-8)
///
/// # Returns
/// Filtered signal with same length as input
pub fn butterworth_lowpass_filtfilt(data: &[f64], cutoff: f64, order: usize) -> Vec<f64> {
    if data.is_empty() {
        return vec![];
    }

    // Clamp order to valid range
    let order = order.clamp(1, 8);

    // Clamp cutoff to valid range (with small margin from boundaries)
    let cutoff = cutoff.clamp(0.001, 0.499);

    let sos = butterworth_lowpass_sos(cutoff, order);
    sosfiltfilt(data, &sos)
}

/// Butterworth highpass filter with zero-phase filtering
///
/// Uses second-order sections for numerical stability and
/// forward-backward filtering to eliminate phase distortion.
///
/// # Arguments
/// * `data` - Input signal
/// * `cutoff` - Normalized cutoff frequency (0 < cutoff < 0.5, where 0.5 = Nyquist)
/// * `order` - Filter order (1-8)
///
/// # Returns
/// Filtered signal with same length as input
pub fn butterworth_highpass_filtfilt(data: &[f64], cutoff: f64, order: usize) -> Vec<f64> {
    if data.is_empty() {
        return vec![];
    }

    // Clamp order to valid range
    let order = order.clamp(1, 8);

    // Clamp cutoff to valid range
    let cutoff = cutoff.clamp(0.001, 0.499);

    let sos = butterworth_highpass_sos(cutoff, order);
    sosfiltfilt(data, &sos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moving_average() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = moving_average(&data, 3);

        assert_eq!(result.len(), 5);
        assert!((result[0] - 1.0).abs() < 0.001); // First value is just itself
        assert!((result[1] - 1.5).abs() < 0.001); // (1+2)/2
        assert!((result[2] - 2.0).abs() < 0.001); // (1+2+3)/3
        assert!((result[3] - 3.0).abs() < 0.001); // (2+3+4)/3
        assert!((result[4] - 4.0).abs() < 0.001); // (3+4+5)/3
    }

    #[test]
    fn test_exponential_moving_average() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = exponential_moving_average(&data, 0.5);

        assert_eq!(result.len(), 5);
        assert!((result[0] - 1.0).abs() < 0.001);
        // EMA grows towards recent values
        assert!(result[4] > result[0]);
    }

    #[test]
    fn test_median_filter() {
        // Test with spike
        let data = vec![1.0, 1.0, 100.0, 1.0, 1.0];
        let result = median_filter(&data, 3);

        assert_eq!(result.len(), 5);
        // The spike should be removed
        assert!((result[2] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_butterworth_lowpass_removes_high_freq() {
        // Create a signal with low frequency + high frequency noise
        let data: Vec<f64> = (0..200)
            .map(|i| {
                let t = i as f64 * 0.01;
                // 1 Hz signal + 10 Hz noise (at 100 Hz sample rate)
                (2.0 * PI * 1.0 * t).sin() + 0.5 * (2.0 * PI * 10.0 * t).sin()
            })
            .collect();

        // Apply lowpass filter with cutoff at 0.05 (5 Hz at 100 Hz sample rate)
        let filtered = butterworth_lowpass_filtfilt(&data, 0.05, 4);

        assert_eq!(filtered.len(), data.len());

        // The filtered signal should have reduced high frequency content
        let orig_power: f64 = data.iter().map(|x| x * x).sum::<f64>();
        let filt_power: f64 = filtered.iter().map(|x| x * x).sum::<f64>();

        // Filtered should have less power due to noise removal
        assert!(filt_power < orig_power);

        // But should retain most of the low-frequency signal
        assert!(filt_power > orig_power * 0.3);
    }

    #[test]
    fn test_butterworth_preserves_dc() {
        // DC signal should pass through lowpass unchanged
        let data = vec![5.0; 200];
        let filtered = butterworth_lowpass_filtfilt(&data, 0.1, 4);

        // All values should be very close to 5.0
        for &v in &filtered {
            assert!((v - 5.0).abs() < 0.01, "DC not preserved: got {}", v);
        }
    }

    #[test]
    fn test_butterworth_highpass_removes_dc() {
        // DC + AC signal
        let data: Vec<f64> = (0..200)
            .map(|i| {
                let t = i as f64 * 0.01;
                10.0 + (2.0 * PI * 5.0 * t).sin() // DC offset + 5 Hz signal
            })
            .collect();

        // Highpass with cutoff at 0.02 (2 Hz at 100 Hz sample rate)
        let filtered = butterworth_highpass_filtfilt(&data, 0.02, 2);

        assert_eq!(filtered.len(), data.len());

        // Mean should be close to zero (DC removed)
        let mean: f64 = filtered.iter().sum::<f64>() / filtered.len() as f64;
        assert!(mean.abs() < 1.0, "DC not removed: mean = {}", mean);
    }

    #[test]
    fn test_butterworth_all_orders() {
        // Test that all orders 1-8 work without panicking
        let data: Vec<f64> = (0..100).map(|i| (i as f64 * 0.1).sin()).collect();

        for order in 1..=8 {
            let filtered = butterworth_lowpass_filtfilt(&data, 0.2, order);
            assert_eq!(filtered.len(), data.len(), "Order {} failed", order);

            // Verify no NaN values
            assert!(
                !filtered.iter().any(|x| x.is_nan()),
                "Order {} produced NaN",
                order
            );
        }
    }

    #[test]
    fn test_butterworth_handles_nan() {
        // Data with NaN values should be handled gracefully
        let mut data: Vec<f64> = (0..100).map(|i| i as f64).collect();
        data[50] = f64::NAN;

        let filtered = butterworth_lowpass_filtfilt(&data, 0.1, 2);

        assert_eq!(filtered.len(), data.len());
        assert!(!filtered.iter().any(|x| x.is_nan()), "Output contains NaN");
    }

    #[test]
    fn test_butterworth_edge_transients() {
        // Step function - should have minimal edge transients with proper padding
        let mut data = vec![0.0; 100];
        data.extend(vec![1.0; 100]);

        let filtered = butterworth_lowpass_filtfilt(&data, 0.1, 2);

        // First few samples should be close to 0 (not wildly oscillating)
        assert!(filtered[0].abs() < 0.2, "Edge transient too large at start");

        // Last few samples should be close to 1
        assert!(
            (filtered[filtered.len() - 1] - 1.0).abs() < 0.2,
            "Edge transient too large at end"
        );
    }

    #[test]
    fn test_reflect_pad() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let padded = reflect_pad(&data, 2);

        // Should be: [2*1-3, 2*1-2, 1, 2, 3, 4, 5, 2*5-4, 2*5-3]
        //          = [-1, 0, 1, 2, 3, 4, 5, 6, 7]
        assert_eq!(padded.len(), 9);
        assert!((padded[0] - (-1.0)).abs() < 0.001);
        assert!((padded[1] - 0.0).abs() < 0.001);
        assert!((padded[2] - 1.0).abs() < 0.001);
        assert!((padded[6] - 5.0).abs() < 0.001);
        assert!((padded[7] - 6.0).abs() < 0.001);
        assert!((padded[8] - 7.0).abs() < 0.001);
    }

    #[test]
    fn test_sos_lowpass_coefficients() {
        // Verify that the SOS coefficients are reasonable
        let sos = butterworth_lowpass_sos(0.2, 2);

        assert_eq!(sos.len(), 1); // Order 2 = 1 biquad

        // DC gain should be 1 (sum of b coeffs / sum of a coeffs)
        let section = &sos[0];
        let b_sum = section.b0 + section.b1 + section.b2;
        let a_sum = 1.0 + section.a1 + section.a2;
        let dc_gain = b_sum / a_sum;

        assert!(
            (dc_gain - 1.0).abs() < 0.01,
            "DC gain should be 1, got {}",
            dc_gain
        );
    }

    #[test]
    fn test_sos_highpass_coefficients() {
        // Verify highpass has zero DC gain
        let sos = butterworth_highpass_sos(0.2, 2);

        assert_eq!(sos.len(), 1);

        // DC gain should be 0 for highpass
        let section = &sos[0];
        let b_sum = section.b0 + section.b1 + section.b2;

        assert!(b_sum.abs() < 0.01, "HP DC gain should be 0, got {}", b_sum);
    }

    #[test]
    fn test_butterworth_symmetry() {
        // Zero-phase filtering should be symmetric around center
        let mut data = vec![0.0; 50];
        data.push(1.0); // Impulse at center
        data.extend(vec![0.0; 49]);

        let filtered = butterworth_lowpass_filtfilt(&data, 0.1, 2);

        // Response should be symmetric around the impulse
        let center = 50;
        for i in 1..20 {
            let left = filtered[center - i];
            let right = filtered[center + i];
            assert!(
                (left - right).abs() < 0.01,
                "Not symmetric at offset {}: left={}, right={}",
                i,
                left,
                right
            );
        }
    }
}
