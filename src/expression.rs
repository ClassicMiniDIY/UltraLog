//! Expression parsing and evaluation engine for computed channels
//!
//! This module handles parsing mathematical formulas that reference channel data,
//! including support for time-shifted values (both index-based and time-based).

use crate::computed::{ChannelReference, TimeShift};
use crate::parsers::types::Value;
use meval::{Context, Expr};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

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

/// Known meval functions and constants that should not be treated as channel names
const RESERVED_NAMES: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "sinh", "cosh", "tanh", "asinh", "acosh",
    "atanh", "sqrt", "abs", "exp", "ln", "log", "log2", "log10", "floor", "ceil", "round", "trunc",
    "fract", "signum", "max", "min", "pi", "e", "tau", "phi",
];

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

        // Skip reserved names (meval functions/constants)
        if RESERVED_NAMES.contains(&name.to_lowercase().as_str()) {
            continue;
        }

        // Skip if this position is inside a quoted reference
        let start_pos = caps.get(0).unwrap().start();
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

    // Try to parse the formula with meval (using dummy variables)
    let test_formula = prepare_formula_for_meval(formula, &refs);

    // Create a context with all variables set to 1.0
    let mut ctx = Context::new();
    for r in &refs {
        let var_name = sanitize_var_name(&r.full_match);
        ctx.var(&var_name, 1.0);
    }

    match test_formula.parse::<Expr>() {
        Ok(expr) => {
            // Try to evaluate with dummy values
            match expr.eval_with_context(&ctx) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Evaluation error: {}", e)),
            }
        }
        Err(e) => Err(format!("Parse error: {}", e)),
    }
}

/// Prepare a formula for meval by replacing channel references with sanitized variable names
fn prepare_formula_for_meval(formula: &str, refs: &[ChannelReference]) -> String {
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

/// Sanitize a channel reference into a valid meval variable name
fn sanitize_var_name(full_match: &str) -> String {
    // Replace non-alphanumeric chars with underscores, ensure starts with letter
    let sanitized: String = full_match
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();

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

/// Evaluate a formula for all records in the log
pub fn evaluate_all_records(
    formula: &str,
    bindings: &HashMap<String, usize>,
    log_data: &[Vec<Value>],
    times: &[f64],
) -> Result<Vec<f64>, String> {
    if log_data.is_empty() {
        return Ok(Vec::new());
    }

    let refs = extract_channel_references(formula);
    let prepared_formula = prepare_formula_for_meval(formula, &refs);

    // Parse the formula once
    let expr: Expr = prepared_formula
        .parse()
        .map_err(|e| format!("Parse error: {}", e))?;

    let num_records = log_data.len();
    let mut results = Vec::with_capacity(num_records);

    for record_idx in 0..num_records {
        let mut ctx = Context::new();

        // Set each channel variable to its value at the appropriate record
        for r in &refs {
            let channel_idx = bindings.get(&r.name).copied().unwrap_or(0);
            let value = get_shifted_value(record_idx, &r.time_shift, channel_idx, log_data, times);
            let var_name = sanitize_var_name(&r.full_match);
            ctx.var(&var_name, value);
        }

        match expr.eval_with_context(&ctx) {
            Ok(value) => {
                // Handle NaN and infinity
                if value.is_nan() || value.is_infinite() {
                    results.push(0.0);
                } else {
                    results.push(value);
                }
            }
            Err(_) => {
                results.push(0.0);
            }
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
