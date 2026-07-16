//! Expression parsing and evaluation engine for computed channels
//!
//! This module handles parsing mathematical formulas that reference channel data,
//! including support for time-shifted values (both index-based and time-based),
//! and pre-computed channel statistics for anomaly detection.
//!
//! Formulas are compiled once by the built-in [`engine`] (which replaced the
//! unmaintained `meval` crate while preserving its grammar exactly) and then
//! evaluated per record against a slice of variable values, so evaluating a
//! formula across a large log does no per-record parsing or allocation.

mod engine;

use crate::computed::{ChannelReference, TimeShift};
use crate::parsers::types::Value;
use engine::CompiledExpr;
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

/// Pre-computed statistics for a channel, used for anomaly detection formulas
#[derive(Clone, Debug, Default)]
pub struct ChannelStatistics {
    /// Arithmetic mean of all values
    pub mean: f64,
    /// Standard deviation of all values
    pub stdev: f64,
    /// Minimum value
    pub min: f64,
    /// Maximum value
    pub max: f64,
    /// Range (max - min)
    pub range: f64,
}

/// Compute statistics for a single channel
pub fn compute_channel_statistics(
    channel_idx: usize,
    log_data: &[Vec<Value>],
) -> ChannelStatistics {
    if log_data.is_empty() {
        return ChannelStatistics::default();
    }

    let values: Vec<f64> = log_data
        .iter()
        .filter_map(|row| row.get(channel_idx).map(|v| v.as_f64()))
        .filter(|v| v.is_finite())
        .collect();

    if values.is_empty() {
        return ChannelStatistics::default();
    }

    let n = values.len() as f64;
    let sum: f64 = values.iter().sum();
    let mean = sum / n;

    let variance: f64 = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let stdev = variance.sqrt();

    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    ChannelStatistics {
        mean,
        stdev,
        min,
        max,
        range: max - min,
    }
}

/// Compute statistics for all channels in a log
pub fn compute_all_channel_statistics(
    channel_names: &[String],
    log_data: &[Vec<Value>],
) -> HashMap<String, ChannelStatistics> {
    let mut stats = HashMap::new();
    for (idx, name) in channel_names.iter().enumerate() {
        stats.insert(name.clone(), compute_channel_statistics(idx, log_data));
    }
    stats
}

/// Regex for parsing quoted channel references with optional time shifts
/// Pattern: "Channel Name" (anything in quotes) with optional time shift
static QUOTED_CHANNEL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#""([^"]+)"(?:\[([+-]?\d+)\]|@([+-]?\d+\.?\d*)s)?"#).expect("Invalid regex pattern")
});

/// Regex for parsing unquoted channel references with optional time shifts
/// Pattern: ChannelName (identifier-like) with optional time shift
static UNQUOTED_CHANNEL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"([a-zA-Z_][a-zA-Z0-9_]*)(?:\[([+-]?\d+)\]|@([+-]?\d+\.?\d*)s)?"#)
        .expect("Invalid regex pattern")
});

/// Function and constant names reserved by the expression engine; these are
/// never treated as channel names. (Channels with these exact names can still
/// be referenced by quoting them.)
const RESERVED_NAMES: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh", "asinh", "acosh",
    "atanh", "sqrt", "abs", "exp", "ln", "log", "log2", "log10", "floor", "ceil", "round", "trunc",
    "fract", "signum", "max", "min", "pow", "pi", "e", "tau", "phi",
];

/// Prefixes for statistical variables that should not be treated as channel names
const STATS_PREFIXES: &[&str] = &["_mean_", "_stdev_", "_min_", "_max_", "_range_"];

/// Whether a formula references statistical variables
/// (`_mean_*`, `_stdev_*`, `_min_*`, `_max_*`, `_range_*`) and therefore needs
/// pre-computed channel statistics to evaluate.
pub fn formula_uses_statistics(formula: &str) -> bool {
    STATS_PREFIXES.iter().any(|p| formula.contains(p))
}

/// Extract all channel references from a formula
pub fn extract_channel_references(formula: &str) -> Vec<ChannelReference> {
    let mut references = Vec::new();

    // First, extract quoted channel names (these take precedence)
    for caps in QUOTED_CHANNEL_REGEX.captures_iter(formula) {
        let name = caps.get(1).unwrap().as_str().to_string();
        let index_shift = caps.get(2).map(|m| m.as_str());
        let time_shift_str = caps.get(3).map(|m| m.as_str());
        let full_match = caps.get(0).unwrap().as_str().to_string();

        let time_shift = parse_time_shift(index_shift, time_shift_str);

        references.push(ChannelReference {
            name,
            time_shift,
            full_match,
        });
    }

    // Then extract unquoted channel names
    for caps in UNQUOTED_CHANNEL_REGEX.captures_iter(formula) {
        let name = caps.get(1).unwrap().as_str().to_string();
        let index_shift = caps.get(2).map(|m| m.as_str());
        let time_shift_str = caps.get(3).map(|m| m.as_str());
        let full_match = caps.get(0).unwrap().as_str().to_string();

        // Skip reserved names (engine functions/constants)
        if RESERVED_NAMES.contains(&name.to_lowercase().as_str()) {
            continue;
        }

        // Skip statistical variable references (e.g., _mean_RPM, _stdev_AFR)
        if STATS_PREFIXES.iter().any(|p| full_match.starts_with(p)) {
            continue;
        }

        let start_pos = caps.get(0).unwrap().start();

        // Skip the exponent fragment of a scientific-notation literal (the
        // "e2" in "1e2"): an identifier of the form e<digits> immediately
        // preceded by a digit or '.' is part of a number, not a channel.
        let is_exponent_fragment = {
            let mut name_chars = name.chars();
            matches!(name_chars.next(), Some('e' | 'E'))
                && name_chars.all(|c| c.is_ascii_digit())
                && formula[..start_pos]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_ascii_digit() || c == '.')
        };
        if is_exponent_fragment {
            continue;
        }

        // Skip if this position is inside a quoted reference
        let is_inside_quoted = references.iter().any(|r| {
            if let Some(pos) = formula.find(&r.full_match) {
                start_pos >= pos && start_pos < pos + r.full_match.len()
            } else {
                false
            }
        });

        if is_inside_quoted {
            continue;
        }

        let time_shift = parse_time_shift(index_shift, time_shift_str);

        references.push(ChannelReference {
            name,
            time_shift,
            full_match,
        });
    }

    // Deduplicate by full_match
    references.sort_by(|a, b| b.full_match.len().cmp(&a.full_match.len())); // Sort by length descending
    let mut seen = std::collections::HashSet::new();
    references.retain(|r| seen.insert(r.full_match.clone()));

    references
}

/// Helper to parse time shift from capture groups
fn parse_time_shift(index_shift: Option<&str>, time_shift_str: Option<&str>) -> TimeShift {
    if let Some(idx_str) = index_shift {
        match idx_str.parse::<i32>() {
            Ok(offset) => TimeShift::IndexOffset(offset),
            Err(_) => TimeShift::None,
        }
    } else if let Some(time_str) = time_shift_str {
        match time_str.parse::<f64>() {
            Ok(offset) => TimeShift::TimeOffset(offset),
            Err(_) => TimeShift::None,
        }
    } else {
        TimeShift::None
    }
}

/// Validate a formula for syntax errors and channel availability
pub fn validate_formula(formula: &str, available_channels: &[String]) -> Result<(), String> {
    if formula.trim().is_empty() {
        return Err("Formula cannot be empty".to_string());
    }

    // Extract channel references
    let refs = extract_channel_references(formula);

    // Check that all referenced channels exist
    let missing: Vec<_> = refs
        .iter()
        .filter(|r| {
            !available_channels
                .iter()
                .any(|c| c.eq_ignore_ascii_case(&r.name))
        })
        .map(|r| r.name.clone())
        .collect();

    if !missing.is_empty() {
        return Err(format!("Unknown channels: {}", missing.join(", ")));
    }

    // Compile the formula and check that every variable it uses resolves to
    // either a channel reference or a statistical variable.
    let compiled = compile_formula(formula, &refs)?;

    let ref_names: std::collections::HashSet<String> = refs
        .iter()
        .map(|r| sanitize_var_name(&r.full_match))
        .collect();

    // Dummy statistics for every available channel (mirrors evaluation-time
    // injection so stats-based formulas validate).
    let mut stat_names = std::collections::HashSet::new();
    for channel in available_channels {
        let safe_name = sanitize_var_name(channel);
        for prefix in STATS_PREFIXES {
            stat_names.insert(format!("{prefix}{safe_name}"));
        }
    }

    for var_name in compiled.var_names() {
        if !ref_names.contains(var_name) && !stat_names.contains(var_name) {
            return Err(format!("Evaluation error: unknown variable '{var_name}'"));
        }
    }

    Ok(())
}

/// Compile a formula: replace channel references with sanitized variable
/// names, then parse with the expression engine.
fn compile_formula(formula: &str, refs: &[ChannelReference]) -> Result<CompiledExpr, String> {
    let prepared = prepare_formula(formula, refs);
    CompiledExpr::parse(&prepared).map_err(|e| format!("Parse error: {e}"))
}

/// Prepare a formula for the engine by replacing channel references with
/// sanitized variable names
fn prepare_formula(formula: &str, refs: &[ChannelReference]) -> String {
    let mut result = formula.to_string();

    // Sort refs by length (longest first) to avoid partial replacements
    let mut sorted_refs: Vec<_> = refs.iter().collect();
    sorted_refs.sort_by(|a, b| b.full_match.len().cmp(&a.full_match.len()));

    for r in sorted_refs {
        let var_name = sanitize_var_name(&r.full_match);
        result = result.replace(&r.full_match, &var_name);
    }

    result
}

/// Sanitize a channel reference into a valid engine variable name.
/// Encodes special characters distinctly to avoid collisions
/// (e.g., `RPM[-1]` vs `RPM[+1]` must produce different names).
fn sanitize_var_name(full_match: &str) -> String {
    let mut sanitized = String::with_capacity(full_match.len());
    for c in full_match.chars() {
        if c.is_alphanumeric() {
            sanitized.push(c);
        } else {
            // Encode special chars distinctly to prevent collisions
            match c {
                '+' => sanitized.push_str("_plus_"),
                '-' => sanitized.push_str("_neg_"),
                '[' => sanitized.push_str("_lb_"),
                ']' => sanitized.push_str("_rb_"),
                '@' => sanitized.push_str("_at_"),
                '.' => sanitized.push_str("_dot_"),
                '"' => sanitized.push_str("_q_"),
                _ => sanitized.push('_'),
            }
        }
    }

    if sanitized
        .chars()
        .next()
        .map(|c| c.is_numeric())
        .unwrap_or(true)
    {
        format!("v_{}", sanitized)
    } else {
        sanitized
    }
}

/// Build channel bindings from references to file channel indices
pub fn build_channel_bindings(
    refs: &[ChannelReference],
    available_channels: &[String],
) -> Result<HashMap<String, usize>, String> {
    let mut bindings = HashMap::new();

    for r in refs {
        // Find channel index (case-insensitive match)
        let idx = available_channels
            .iter()
            .position(|c| c.eq_ignore_ascii_case(&r.name))
            .ok_or_else(|| format!("Channel not found: {}", r.name))?;

        bindings.insert(r.name.clone(), idx);
    }

    Ok(bindings)
}

/// Where a compiled variable slot gets its value from during evaluation.
enum SlotSource<'a> {
    /// A channel reference: read from the log with an optional time shift.
    Channel {
        channel_index: usize,
        time_shift: &'a TimeShift,
    },
    /// A constant for the whole evaluation (statistical variables).
    Constant(f64),
}

/// Resolve every variable slot of a compiled formula to its value source.
fn resolve_slots<'a>(
    compiled: &CompiledExpr,
    refs: &'a [ChannelReference],
    bindings: &HashMap<String, usize>,
    stat_values: &HashMap<String, f64>,
) -> Result<Vec<SlotSource<'a>>, String> {
    let ref_by_var: HashMap<String, &ChannelReference> = refs
        .iter()
        .map(|r| (sanitize_var_name(&r.full_match), r))
        .collect();

    compiled
        .var_names()
        .iter()
        .map(|var_name| {
            if let Some(r) = ref_by_var.get(var_name) {
                let channel_index = bindings.get(&r.name).copied().ok_or_else(|| {
                    format!("Evaluation error: no binding for channel '{}'", r.name)
                })?;
                Ok(SlotSource::Channel {
                    channel_index,
                    time_shift: &r.time_shift,
                })
            } else if let Some(value) = stat_values.get(var_name) {
                Ok(SlotSource::Constant(*value))
            } else {
                Err(format!("Evaluation error: unknown variable '{var_name}'"))
            }
        })
        .collect()
}

/// Build the map of statistical variable names to their values
/// (`_mean_ChannelName`, `_stdev_ChannelName`, ...).
fn build_stat_values(statistics: &HashMap<String, ChannelStatistics>) -> HashMap<String, f64> {
    let mut values = HashMap::with_capacity(statistics.len() * STATS_PREFIXES.len());
    for (channel_name, channel_stats) in statistics {
        let safe_name = sanitize_var_name(channel_name);
        values.insert(format!("_mean_{safe_name}"), channel_stats.mean);
        values.insert(format!("_stdev_{safe_name}"), channel_stats.stdev);
        values.insert(format!("_min_{safe_name}"), channel_stats.min);
        values.insert(format!("_max_{safe_name}"), channel_stats.max);
        values.insert(format!("_range_{safe_name}"), channel_stats.range);
    }
    values
}

/// Evaluate a formula for all records in the log
///
/// If the formula uses statistical variables (`_mean_ChannelName`, ...), use
/// [`evaluate_all_records_with_stats`] and provide the statistics.
pub fn evaluate_all_records(
    formula: &str,
    bindings: &HashMap<String, usize>,
    log_data: &[Vec<Value>],
    times: &[f64],
) -> Result<Vec<f64>, String> {
    evaluate_all_records_with_stats(formula, bindings, log_data, times, None)
}

/// Evaluate a formula for all records in the log with optional pre-computed statistics
///
/// If `statistics` is provided, injects statistical variables for each channel:
/// - `_mean_ChannelName`, `_stdev_ChannelName`, `_min_ChannelName`, `_max_ChannelName`, `_range_ChannelName`
pub fn evaluate_all_records_with_stats(
    formula: &str,
    bindings: &HashMap<String, usize>,
    log_data: &[Vec<Value>],
    times: &[f64],
    statistics: Option<&HashMap<String, ChannelStatistics>>,
) -> Result<Vec<f64>, String> {
    if log_data.is_empty() {
        return Ok(Vec::new());
    }

    let refs = extract_channel_references(formula);

    // Compile once; per-record evaluation only fills the variable slots.
    let compiled = compile_formula(formula, &refs)?;
    let stat_values = statistics.map(build_stat_values).unwrap_or_default();
    let slots = resolve_slots(&compiled, &refs, bindings, &stat_values)?;

    let mut values = vec![0.0_f64; slots.len()];
    // Statistical variables are constant across records; fill them once.
    for (slot, value) in slots.iter().zip(values.iter_mut()) {
        if let SlotSource::Constant(c) = slot {
            *value = *c;
        }
    }

    let num_records = log_data.len();
    let mut results = Vec::with_capacity(num_records);
    let mut stack = Vec::new();

    for record_idx in 0..num_records {
        for (slot, value) in slots.iter().zip(values.iter_mut()) {
            if let SlotSource::Channel {
                channel_index,
                time_shift,
            } = slot
            {
                *value = get_shifted_value(record_idx, time_shift, *channel_index, log_data, times);
            }
        }

        let result = compiled.eval_with_stack(&values, &mut stack);
        // Handle NaN and infinity
        if result.is_finite() {
            results.push(result);
        } else {
            results.push(0.0);
        }
    }

    Ok(results)
}

/// Get a channel value with time shift applied
fn get_shifted_value(
    record_index: usize,
    time_shift: &TimeShift,
    channel_index: usize,
    log_data: &[Vec<Value>],
    times: &[f64],
) -> f64 {
    let target_idx = match time_shift {
        TimeShift::None => record_index,

        TimeShift::IndexOffset(offset) => {
            let target = record_index as i64 + *offset as i64;
            target.clamp(0, log_data.len().saturating_sub(1) as i64) as usize
        }

        TimeShift::TimeOffset(seconds) => {
            let current_time = times.get(record_index).copied().unwrap_or(0.0);
            let target_time = current_time + seconds;
            find_record_at_time(times, target_time)
        }
    };

    log_data
        .get(target_idx)
        .and_then(|row| row.get(channel_index))
        .map(|v| v.as_f64())
        .unwrap_or(0.0)
}

/// Find the record index closest to a given time using binary search
fn find_record_at_time(times: &[f64], target_time: f64) -> usize {
    if times.is_empty() {
        return 0;
    }

    // Clamp to valid time range
    let clamped_time = target_time.clamp(
        *times.first().unwrap_or(&0.0),
        *times.last().unwrap_or(&0.0),
    );

    // Binary search for closest time
    match times.binary_search_by(|t| {
        t.partial_cmp(&clamped_time)
            .unwrap_or(std::cmp::Ordering::Equal)
    }) {
        Ok(idx) => idx,
        Err(idx) => {
            // idx is where we'd insert to maintain order
            if idx == 0 {
                0
            } else if idx >= times.len() {
                times.len() - 1
            } else {
                // Check which neighbor is closer
                let prev_diff = (times[idx - 1] - clamped_time).abs();
                let next_diff = (times[idx] - clamped_time).abs();
                if prev_diff <= next_diff {
                    idx - 1
                } else {
                    idx
                }
            }
        }
    }
}

/// Generate preview values for a formula (first N values)
pub fn generate_preview(
    formula: &str,
    bindings: &HashMap<String, usize>,
    log_data: &[Vec<Value>],
    times: &[f64],
    count: usize,
) -> Result<Vec<f64>, String> {
    let all_values = evaluate_all_records(formula, bindings, log_data, times)?;
    Ok(all_values.into_iter().take(count).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_simple_reference() {
        let refs = extract_channel_references("RPM * 2");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "RPM");
        assert_eq!(refs[0].time_shift, TimeShift::None);
    }

    #[test]
    fn test_extract_quoted_reference() {
        let refs = extract_channel_references("\"Manifold Pressure\" + 10");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "Manifold Pressure");
    }

    #[test]
    fn test_extract_index_offset() {
        let refs = extract_channel_references("RPM[-1]");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "RPM");
        assert_eq!(refs[0].time_shift, TimeShift::IndexOffset(-1));
    }

    #[test]
    fn test_extract_time_offset() {
        let refs = extract_channel_references("RPM@-0.1s");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "RPM");
        assert_eq!(refs[0].time_shift, TimeShift::TimeOffset(-0.1));
    }

    #[test]
    fn test_extract_multiple_references() {
        let refs = extract_channel_references("RPM + Boost * 2");
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_skip_reserved_names() {
        let refs = extract_channel_references("sin(RPM) + cos(Boost)");
        // Should find RPM and Boost, but not sin and cos
        assert_eq!(refs.len(), 2);
        assert!(refs.iter().all(|r| r.name != "sin" && r.name != "cos"));
    }

    #[test]
    fn test_validate_valid_formula() {
        let channels = vec!["RPM".to_string(), "Boost".to_string()];
        let result = validate_formula("RPM + Boost", &channels);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_missing_channel() {
        let channels = vec!["RPM".to_string()];
        let result = validate_formula("RPM + MissingChannel", &channels);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown channels"));
    }

    #[test]
    fn test_validate_empty_formula() {
        let channels = vec!["RPM".to_string()];
        let result = validate_formula("", &channels);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_stats_formula() {
        let channels = vec!["RPM".to_string()];
        let result = validate_formula("(RPM - _mean_RPM) / _stdev_RPM", &channels);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_skips_scientific_notation_exponents() {
        // "e2" in "1e2" is an exponent, not a channel.
        assert!(extract_channel_references("RPM * 1e2").len() == 1);
        assert!(extract_channel_references("2E10 + 1.5e3").is_empty());
        // ...but a genuine channel named like an exponent is still extracted
        // when it does not directly follow a digit.
        let refs = extract_channel_references("RPM + e2");
        assert!(refs.iter().any(|r| r.name == "e2"));
    }

    #[test]
    fn test_validate_scientific_notation() {
        let channels = vec!["RPM".to_string()];
        assert!(validate_formula("RPM * 1e2", &channels).is_ok());
        assert!(validate_formula("RPM * 1.5E-3 + 2e10", &channels).is_ok());
    }

    #[test]
    fn test_evaluate_missing_binding_errors() {
        let data = vec![vec![Value::Float(1.0)]];
        let times = vec![0.0];
        // Bindings intentionally empty: the referenced channel is unresolved.
        let bindings = HashMap::new();

        let result = evaluate_all_records("RPM * 2", &bindings, &data, &times);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no binding"));
    }

    #[test]
    fn test_validate_variadic_min_max() {
        let channels = vec!["RPM".to_string(), "Boost".to_string(), "TPS".to_string()];
        assert!(validate_formula("min(RPM, Boost, TPS)", &channels).is_ok());
        assert!(validate_formula("max(RPM, Boost, TPS, 100)", &channels).is_ok());
    }

    #[test]
    fn test_evaluate_simple() {
        let data = vec![
            vec![Value::Float(1000.0), Value::Float(10.0)],
            vec![Value::Float(2000.0), Value::Float(20.0)],
            vec![Value::Float(3000.0), Value::Float(30.0)],
        ];
        let times = vec![0.0, 0.1, 0.2];
        let mut bindings = HashMap::new();
        bindings.insert("RPM".to_string(), 0);
        bindings.insert("Boost".to_string(), 1);

        let result = evaluate_all_records("RPM + Boost", &bindings, &data, &times).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], 1010.0);
        assert_eq!(result[1], 2020.0);
        assert_eq!(result[2], 3030.0);
    }

    #[test]
    fn test_evaluate_with_index_offset() {
        let data = vec![
            vec![Value::Float(1000.0)],
            vec![Value::Float(2000.0)],
            vec![Value::Float(3000.0)],
        ];
        let times = vec![0.0, 0.1, 0.2];
        let mut bindings = HashMap::new();
        bindings.insert("RPM".to_string(), 0);

        // RPM - RPM[-1] should give the change from previous sample
        let result = evaluate_all_records("RPM - RPM[-1]", &bindings, &data, &times).unwrap();
        assert_eq!(result.len(), 3);
        // First record: 1000 - 1000 (clamped to 0) = 0
        assert_eq!(result[0], 0.0);
        // Second record: 2000 - 1000 = 1000
        assert_eq!(result[1], 1000.0);
        // Third record: 3000 - 2000 = 1000
        assert_eq!(result[2], 1000.0);
    }

    #[test]
    fn test_evaluate_with_stats() {
        let data = vec![
            vec![Value::Float(1.0)],
            vec![Value::Float(2.0)],
            vec![Value::Float(3.0)],
        ];
        let times = vec![0.0, 0.1, 0.2];
        let mut bindings = HashMap::new();
        bindings.insert("RPM".to_string(), 0);

        let channels = vec!["RPM".to_string()];
        let statistics = compute_all_channel_statistics(&channels, &data);

        let result = evaluate_all_records_with_stats(
            "RPM - _mean_RPM",
            &bindings,
            &data,
            &times,
            Some(&statistics),
        )
        .unwrap();
        assert_eq!(result.len(), 3);
        assert!((result[0] - (-1.0)).abs() < 1e-12);
        assert!((result[1] - 0.0).abs() < 1e-12);
        assert!((result[2] - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_evaluate_stats_formula_without_statistics_errors() {
        let data = vec![vec![Value::Float(1.0)]];
        let times = vec![0.0];
        let mut bindings = HashMap::new();
        bindings.insert("RPM".to_string(), 0);

        // Statistical variables require statistics to be provided; previously
        // (with meval) this silently produced all zeros.
        let result = evaluate_all_records("RPM - _mean_RPM", &bindings, &data, &times);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("_mean_RPM"));
    }

    #[test]
    fn test_find_record_at_time() {
        let times = vec![0.0, 0.1, 0.2, 0.3, 0.4];

        assert_eq!(find_record_at_time(&times, 0.0), 0);
        assert_eq!(find_record_at_time(&times, 0.1), 1);
        assert_eq!(find_record_at_time(&times, 0.15), 1); // Closer to 0.1
        assert_eq!(find_record_at_time(&times, 0.16), 2); // Closer to 0.2
        assert_eq!(find_record_at_time(&times, -1.0), 0); // Clamped
        assert_eq!(find_record_at_time(&times, 10.0), 4); // Clamped
    }
}
