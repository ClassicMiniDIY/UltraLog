//! Robust statistics and sample-timing helpers shared by the table generators.
//!
//! Everything here is order-statistic based (median / MAD) rather than
//! mean / standard deviation: a handful of mis-attributed events in a tuning
//! table cell must not drag the cell value, and a single sentinel sample must
//! not blow up a noise estimate.

/// Median of a slice. Non-finite values are ignored. Returns `None` when no
/// finite value is present.
pub fn median(values: &[f64]) -> Option<f64> {
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    Some(if n.is_multiple_of(2) {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    })
}

/// Median absolute deviation around the median. `None` when the slice has no
/// finite value.
pub fn mad(values: &[f64]) -> Option<f64> {
    let m = median(values)?;
    let deviations: Vec<f64> = values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .map(|v| (v - m).abs())
        .collect();
    median(&deviations)
}

/// Percentile in `[0, 100]` by nearest-rank on the sorted finite values.
pub fn percentile(values: &[f64], pct: f64) -> Option<f64> {
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pct = pct.clamp(0.0, 100.0);
    let rank = ((pct / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
    Some(sorted[rank.min(sorted.len() - 1)])
}

/// Median spacing between consecutive timestamps. `None` for fewer than two
/// samples or when every spacing is zero.
pub fn median_interval(times: &[f64]) -> Option<f64> {
    if times.len() < 2 {
        return None;
    }
    let deltas: Vec<f64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    let m = median(&deltas)?;
    if m > 0.0 { Some(m) } else { None }
}

/// Indices at which `values` changes to a new finite value, starting with the
/// first finite sample. This is the sample-and-hold "update instant" list: a
/// CAN wideband logged at 10 Hz inside a 100 Hz log only changes value every
/// tenth sample, and any threshold-crossing or noise math that looked at raw
/// samples would be looking at nine copies of the same reading.
pub fn update_instants(values: &[f64]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut last: Option<f64> = None;
    for (i, &v) in values.iter().enumerate() {
        if !v.is_finite() {
            continue;
        }
        match last {
            Some(prev) if prev == v => {}
            _ => {
                out.push(i);
                last = Some(v);
            }
        }
    }
    out
}

/// Median spacing between distinct-value updates of a channel, i.e. the rate
/// the sensor is actually delivering new readings at, independent of the log
/// rate. `None` when the channel never changes.
pub fn effective_update_interval(times: &[f64], values: &[f64]) -> Option<f64> {
    let instants = update_instants(values);
    if instants.len() < 2 {
        return None;
    }
    let deltas: Vec<f64> = instants
        .windows(2)
        .map(|w| times[w[1]] - times[w[0]])
        .collect();
    let m = median(&deltas)?;
    if m > 0.0 { Some(m) } else { None }
}

/// Robust noise estimate of a sample-and-hold signal.
///
/// Takes the first differences between consecutive *update instants* (never
/// between raw samples, which are mostly exact repeats), and scales the MAD of
/// those differences to a Gaussian sigma: `1.4826 * MAD / sqrt(2)`. The
/// `sqrt(2)` corrects for differencing two independent samples.
///
/// Returns `None` when there are fewer than three updates.
pub fn robust_sigma_diff(values: &[f64]) -> Option<f64> {
    let instants = update_instants(values);
    if instants.len() < 3 {
        return None;
    }
    let diffs: Vec<f64> = instants
        .windows(2)
        .map(|w| values[w[1]] - values[w[0]])
        .collect();
    mad(&diffs).map(|m| 1.4826 * m / std::f64::consts::SQRT_2)
}

/// Index of the first sample with `times[i] >= t` (binary search; `times`
/// must be non-decreasing).
pub fn index_at_or_after(times: &[f64], t: f64) -> usize {
    times.partition_point(|&x| x < t)
}

/// Half-open index range `[start, end)` covering `t0 <= times[i] < t1`.
pub fn index_range(times: &[f64], t0: f64, t1: f64) -> std::ops::Range<usize> {
    let start = index_at_or_after(times, t0);
    let end = index_at_or_after(times, t1);
    start..end.max(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_and_mad_basic() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[f64::NAN, 5.0]), Some(5.0));
        assert_eq!(mad(&[1.0, 2.0, 3.0, 4.0, 100.0]), Some(1.0));
    }

    #[test]
    fn percentile_bounds() {
        let v: Vec<f64> = (0..=100).map(|i| i as f64).collect();
        assert_eq!(percentile(&v, 0.0), Some(0.0));
        assert_eq!(percentile(&v, 100.0), Some(100.0));
        assert_eq!(percentile(&v, 50.0), Some(50.0));
        assert_eq!(percentile(&[], 50.0), None);
    }

    #[test]
    fn update_instants_skip_holds() {
        let v = [1.0, 1.0, 1.0, 2.0, 2.0, f64::NAN, 3.0, 3.0];
        assert_eq!(update_instants(&v), vec![0, 3, 6]);
    }

    #[test]
    fn effective_rate_is_independent_of_log_rate() {
        // 100 Hz log, sensor updating every 10 samples.
        let times: Vec<f64> = (0..200).map(|i| i as f64 * 0.01).collect();
        let values: Vec<f64> = (0..200).map(|i| (i / 10) as f64).collect();
        let dt = median_interval(&times).unwrap();
        let u = effective_update_interval(&times, &values).unwrap();
        assert!((dt - 0.01).abs() < 1e-9);
        assert!((u - 0.1).abs() < 1e-9);
    }

    #[test]
    fn robust_sigma_ignores_holds() {
        // Alternating +/-0.01 steps at every update; raw samples hold 5x.
        // 101 groups give 100 differences, half +0.01 and half -0.01, so
        // their median is 0 and the MAD is 0.01.
        let mut values = Vec::new();
        for i in 0..101 {
            let v = if i % 2 == 0 { 1.0 } else { 1.01 };
            for _ in 0..5 {
                values.push(v);
            }
        }
        let sigma = robust_sigma_diff(&values).unwrap();
        // MAD of |diff| = 0.01 -> 1.4826 * 0.01 / sqrt(2)
        assert!((sigma - 1.4826 * 0.01 / std::f64::consts::SQRT_2).abs() < 1e-9);
        assert_eq!(robust_sigma_diff(&[1.0; 50]), None);
    }

    #[test]
    fn index_helpers() {
        let times = [0.0, 0.1, 0.2, 0.3, 0.4];
        assert_eq!(index_at_or_after(&times, 0.15), 2);
        assert_eq!(index_at_or_after(&times, 0.2), 2);
        assert_eq!(index_at_or_after(&times, 9.0), 5);
        assert_eq!(index_range(&times, 0.1, 0.3), 1..3);
        assert_eq!(index_range(&times, 0.35, 0.2), 4..4);
    }
}
