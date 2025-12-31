//! Analysis module for ECU log analysis algorithms.
//!
//! This module provides a unified framework for implementing analysis algorithms
//! that can process log data and produce results that integrate with UltraLog's
//! computed channels system.
//!
//! The architecture follows a trait-based design where each analyzer implements
//! the `Analyzer` trait, enabling:
//! - Dynamic discovery of available analyzers based on loaded channels
//! - Configurable parameters via UI
//! - Results that can be visualized or converted to computed channels

pub mod afr;
pub mod derived;
pub mod filters;
pub mod statistics;

use crate::parsers::types::Log;
use std::collections::HashMap;
use std::time::Instant;

/// Errors that can occur during analysis
#[derive(Debug, Clone)]
pub enum AnalysisError {
    /// A required channel is missing from the log data
    MissingChannel(String),
    /// Not enough data points for the analysis
    InsufficientData { needed: usize, got: usize },
    /// Invalid parameter configuration
    InvalidParameter(String),
    /// General computation error
    ComputationError(String),
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisError::MissingChannel(ch) => write!(f, "Missing required channel: {}", ch),
            AnalysisError::InsufficientData { needed, got } => {
                write!(f, "Insufficient data: need {} points, got {}", needed, got)
            }
            AnalysisError::InvalidParameter(msg) => write!(f, "Invalid parameter: {}", msg),
            AnalysisError::ComputationError(msg) => write!(f, "Computation error: {}", msg),
        }
    }
}

impl std::error::Error for AnalysisError {}

/// Metadata about analysis results for UI display
#[derive(Clone, Debug, Default)]
pub struct AnalysisMetadata {
    /// Name of the algorithm used
    pub algorithm: String,
    /// Key parameters and their values
    pub parameters: Vec<(String, String)>,
    /// Warning messages about the analysis
    pub warnings: Vec<String>,
    /// Time taken for computation in milliseconds
    pub computation_time_ms: u64,
}

/// Result of an analysis operation
#[derive(Clone, Debug)]
pub struct AnalysisResult {
    /// Name for the result (used as channel name if added)
    pub name: String,
    /// Unit for the result values
    pub unit: String,
    /// The computed values (one per timestamp)
    pub values: Vec<f64>,
    /// Metadata about the analysis
    pub metadata: AnalysisMetadata,
}

impl AnalysisResult {
    /// Create a new analysis result
    pub fn new(name: impl Into<String>, unit: impl Into<String>, values: Vec<f64>) -> Self {
        Self {
            name: name.into(),
            unit: unit.into(),
            values,
            metadata: AnalysisMetadata::default(),
        }
    }

    /// Add metadata to the result
    pub fn with_metadata(mut self, metadata: AnalysisMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Check if the analysis produced any warnings
    pub fn has_warnings(&self) -> bool {
        !self.metadata.warnings.is_empty()
    }
}

/// Configuration for an analyzer that can be serialized
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AnalyzerConfig {
    /// Unique identifier for the analyzer
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Parameter values as key-value pairs
    pub parameters: HashMap<String, String>,
}

/// Core trait for all analysis algorithms
pub trait Analyzer: Send + Sync {
    /// Unique identifier for this analyzer
    fn id(&self) -> &str;

    /// Human-readable algorithm name
    fn name(&self) -> &str;

    /// Description for UI tooltips
    fn description(&self) -> &str;

    /// Category for grouping in UI (e.g., "Filters", "Statistics", "AFR", "Knock")
    fn category(&self) -> &str;

    /// List of required channel names (normalized names preferred)
    fn required_channels(&self) -> Vec<&str>;

    /// Optional channels that enhance analysis if present
    fn optional_channels(&self) -> Vec<&str> {
        vec![]
    }

    /// Execute analysis on log data
    fn analyze(&self, log: &Log) -> Result<AnalysisResult, AnalysisError>;

    /// Get current configuration
    fn get_config(&self) -> AnalyzerConfig;

    /// Apply configuration
    fn set_config(&mut self, config: &AnalyzerConfig);

    /// Clone into a boxed trait object
    fn clone_box(&self) -> Box<dyn Analyzer>;
}

impl Clone for Box<dyn Analyzer> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Helper trait for accessing log data by channel name
pub trait LogDataAccess {
    /// Get channel values by name (case-insensitive)
    fn get_channel_values(&self, name: &str) -> Option<Vec<f64>>;

    /// Check if a channel exists
    fn has_channel(&self, name: &str) -> bool;

    /// Get all channel names
    fn channel_names(&self) -> Vec<String>;

    /// Get the time vector
    fn times(&self) -> &[f64];
}

impl LogDataAccess for Log {
    fn get_channel_values(&self, name: &str) -> Option<Vec<f64>> {
        // Find the channel index (case-insensitive)
        let channel_idx = self
            .channels
            .iter()
            .position(|c| c.name().eq_ignore_ascii_case(name))?;

        // Extract values from the data matrix
        let values: Vec<f64> = self
            .data
            .iter()
            .filter_map(|row| row.get(channel_idx).map(|v| v.as_f64()))
            .collect();

        Some(values)
    }

    fn has_channel(&self, name: &str) -> bool {
        self.channels
            .iter()
            .any(|c| c.name().eq_ignore_ascii_case(name))
    }

    fn channel_names(&self) -> Vec<String> {
        self.channels.iter().map(|c| c.name()).collect()
    }

    fn times(&self) -> &[f64] {
        &self.times
    }
}

/// Registry of available analyzers
#[derive(Default)]
pub struct AnalyzerRegistry {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl AnalyzerRegistry {
    /// Create a new registry with default analyzers
    pub fn new() -> Self {
        let mut registry = Self {
            analyzers: Vec::new(),
        };

        // Register built-in analyzers
        registry.register_defaults();

        registry
    }

    /// Register default analyzers
    fn register_defaults(&mut self) {
        // Filters
        self.register(Box::new(filters::MovingAverageAnalyzer::default()));
        self.register(Box::new(
            filters::ExponentialMovingAverageAnalyzer::default(),
        ));
        self.register(Box::new(filters::MedianFilterAnalyzer::default()));
        self.register(Box::new(filters::ButterworthLowpassAnalyzer::default()));
        self.register(Box::new(filters::ButterworthHighpassAnalyzer::default()));

        // Statistics
        self.register(Box::new(statistics::DescriptiveStatsAnalyzer::default()));
        self.register(Box::new(statistics::CorrelationAnalyzer::default()));
        self.register(Box::new(statistics::RateOfChangeAnalyzer::default()));

        // AFR Analysis
        self.register(Box::new(afr::FuelTrimDriftAnalyzer::default()));
        self.register(Box::new(afr::RichLeanZoneAnalyzer::default()));
        self.register(Box::new(afr::AfrDeviationAnalyzer::default()));

        // Derived Calculations
        self.register(Box::new(derived::VolumetricEfficiencyAnalyzer::default()));
        self.register(Box::new(derived::InjectorDutyCycleAnalyzer::default()));
        self.register(Box::new(derived::LambdaCalculator::default()));
    }

    /// Register a new analyzer
    pub fn register(&mut self, analyzer: Box<dyn Analyzer>) {
        self.analyzers.push(analyzer);
    }

    /// Get all registered analyzers
    pub fn all(&self) -> &[Box<dyn Analyzer>] {
        &self.analyzers
    }

    /// Get analyzers available for the given log data
    pub fn available_for(&self, log: &Log) -> Vec<&dyn Analyzer> {
        self.analyzers
            .iter()
            .filter(|a| a.required_channels().iter().all(|ch| log.has_channel(ch)))
            .map(|a| a.as_ref())
            .collect()
    }

    /// Get analyzers by category
    pub fn by_category(&self) -> HashMap<String, Vec<&dyn Analyzer>> {
        let mut categories: HashMap<String, Vec<&dyn Analyzer>> = HashMap::new();

        for analyzer in &self.analyzers {
            categories
                .entry(analyzer.category().to_string())
                .or_default()
                .push(analyzer.as_ref());
        }

        categories
    }

    /// Find an analyzer by ID
    pub fn find_by_id(&self, id: &str) -> Option<&dyn Analyzer> {
        self.analyzers
            .iter()
            .find(|a| a.id() == id)
            .map(|a| a.as_ref())
    }

    /// Find an analyzer by ID and return a mutable reference
    pub fn find_by_id_mut(&mut self, id: &str) -> Option<&mut Box<dyn Analyzer>> {
        self.analyzers.iter_mut().find(|a| a.id() == id)
    }
}

/// Helper function to measure analysis execution time
pub fn timed_analyze<F, T>(f: F) -> (T, u64)
where
    F: FnOnce() -> T,
{
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed().as_millis() as u64;
    (result, elapsed)
}

/// Helper to get a required channel or return an error
pub fn require_channel(log: &Log, name: &str) -> Result<Vec<f64>, AnalysisError> {
    log.get_channel_values(name)
        .ok_or_else(|| AnalysisError::MissingChannel(name.to_string()))
}

/// Helper to check minimum data length
pub fn require_min_length(data: &[f64], min_len: usize) -> Result<(), AnalysisError> {
    if data.len() < min_len {
        Err(AnalysisError::InsufficientData {
            needed: min_len,
            got: data.len(),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_error_display() {
        let err = AnalysisError::MissingChannel("RPM".to_string());
        assert!(err.to_string().contains("RPM"));

        let err = AnalysisError::InsufficientData {
            needed: 100,
            got: 50,
        };
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("50"));
    }

    #[test]
    fn test_analysis_result_new() {
        let result = AnalysisResult::new("Test", "units", vec![1.0, 2.0, 3.0]);
        assert_eq!(result.name, "Test");
        assert_eq!(result.unit, "units");
        assert_eq!(result.values.len(), 3);
        assert!(!result.has_warnings());
    }
}
