//! Event-detection primitives shared by the table generators: invalid-sample
//! masking, steadiness gates, threshold crossings with interpolation, and
//! rate-of-change run detection.

use super::stats::{index_range, median};

/// Haltech writes an i32 sentinel family (`-2147483617`, `-2147483637`, ...)
/// for "no reading"; any other exporter that leaks an int32 extreme is caught
/// by the same band. Values this large have no physical meaning in a log.
pub const SENTINEL_MAGNITUDE: f64 = 2_147_483_648.0 - 64.0;

/// Whether a raw sample is an ECU "no reading" sentinel or non-finite.
#[inline]
pub fn is_invalid_sample(v: f64) -> bool {
    !v.is_finite() || v.abs() >= SENTINEL_MAGNITUDE
}

/// Replace sentinel / non-finite / out-of-band samples with `NaN` so later
/// math (which skips non-finite values) never sees them. `band` is an
/// optional inclusive plausibility range for the role.
pub fn mask_invalid(values: &[f64], band: Option<(f64, f64)>) -> Vec<f64> {
    values
        .iter()
        .map(|&v| {
            if is_invalid_sample(v) {
                return f64::NAN;
            }
            match band {
                Some((lo, hi)) if v < lo || v > hi => f64::NAN,
                _ => v,
            }
        })
        .collect()
}

/// Fraction of `NaN` samples in a slice (0 for an empty slice).
pub fn invalid_fraction(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().filter(|v| !v.is_finite()).count() as f64 / values.len() as f64
}

/// Peak-to-peak range of the finite samples in `values[range]`. `None` when
/// the range has no finite sample.
pub fn span(values: &[f64], range: std::ops::Range<usize>) -> Option<f64> {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &v in &values[range.start.min(values.len())..range.end.min(values.len())] {
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    if lo.is_finite() { Some(hi - lo) } else { None }
}

/// Whether the channel stays within a total band of `2 * half_band` over
/// `[t0, t1)`. `None` when no finite sample is in the window.
pub fn is_steady(times: &[f64], values: &[f64], t0: f64, t1: f64, half_band: f64) -> Option<bool> {
    span(values, index_range(times, t0, t1)).map(|s| s <= 2.0 * half_band)
}

/// Median of the finite samples in `[t0, t1)`.
pub fn window_median(times: &[f64], values: &[f64], t0: f64, t1: f64) -> Option<f64> {
    let r = index_range(times, t0, t1);
    median(&values[r.start.min(values.len())..r.end.min(values.len())])
}

/// Whether any finite sample in `[t0, t1)` satisfies `pred`.
pub fn any_in_window(
    times: &[f64],
    values: &[f64],
    t0: f64,
    t1: f64,
    pred: impl Fn(f64) -> bool,
) -> bool {
    let r = index_range(times, t0, t1);
    values[r.start.min(values.len())..r.end.min(values.len())]
        .iter()
        .any(|&v| v.is_finite() && pred(v))
}

/// A threshold crossing located between two update instants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    /// Interpolated crossing time.
    pub time: f64,
    /// Index of the update instant at which the threshold was first met.
    pub index: usize,
}

/// First point in `instants` (indices into `times` / `values`, increasing)
/// where `signed_dev(i) >= threshold`, with the crossing time linearly
/// interpolated from the previous sensor reading.
///
/// The previous reading is the last update instant before `i`, but no
/// earlier than `times[i] - update_interval`: a sample-and-hold sensor that
/// reported the same value for a second has still been *sampling* every
/// `update_interval`, so the reading before the crossing one was taken about
/// one interval earlier, not at the last value change. Interpolating from
/// the last change would place every crossing after a flat baseline far too
/// early; interpolating from the adjacent log sample would place it too late
/// for a sensor slower than the log.
///
/// `seed` is an optional update instant before the first of `instants`
/// that supplies the previous reading for the first candidate.
///
/// Returns `None` when no instant reaches the threshold.
pub fn find_crossing(
    times: &[f64],
    instants: &[usize],
    threshold: f64,
    update_interval: f64,
    seed: Option<usize>,
    signed_dev: impl Fn(usize) -> f64,
) -> Option<Crossing> {
    let mut prev: Option<(usize, f64)> = seed
        .map(|s| (s, signed_dev(s)))
        .filter(|(_, d)| d.is_finite());
    for &i in instants {
        let d = signed_dev(i);
        if !d.is_finite() {
            continue;
        }
        if d >= threshold {
            let time = match prev {
                Some((pi, pd)) if d > pd => {
                    let t_prev = times[pi].max(times[i] - update_interval).min(times[i]);
                    let frac = ((threshold - pd) / (d - pd)).clamp(0.0, 1.0);
                    t_prev + frac * (times[i] - t_prev)
                }
                _ => times[i],
            };
            return Some(Crossing { time, index: i });
        }
        prev = Some((i, d));
    }
    None
}

/// A contiguous run where a rate signal exceeded a threshold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RateRun {
    /// First index above threshold.
    pub start: usize,
    /// Last index above threshold (inclusive).
    pub end: usize,
    /// Index of the peak rate within the run.
    pub peak_index: usize,
    /// Peak rate value.
    pub peak: f64,
}

/// Find runs of at least `min_samples` consecutive samples with
/// `rate[i] >= threshold`.
pub fn find_rate_runs(rate: &[f64], threshold: f64, min_samples: usize) -> Vec<RateRun> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < rate.len() {
        if rate[i].is_finite() && rate[i] >= threshold {
            let start = i;
            let mut peak_index = i;
            let mut peak = rate[i];
            while i < rate.len() && rate[i].is_finite() && rate[i] >= threshold {
                if rate[i] > peak {
                    peak = rate[i];
                    peak_index = i;
                }
                i += 1;
            }
            let end = i - 1;
            if end + 1 - start >= min_samples.max(1) {
                runs.push(RateRun {
                    start,
                    end,
                    peak_index,
                    peak,
                });
            }
        } else {
            i += 1;
        }
    }
    runs
}

/// Merge runs whose starts are within `gap_s` of the previous run's end into
/// one event, keeping the larger peak. Overlapping tip-ins (a double stab of
/// the throttle) cannot be attributed separately.
pub fn merge_runs(times: &[f64], runs: &[RateRun], gap_s: f64) -> Vec<RateRun> {
    let mut merged: Vec<RateRun> = Vec::new();
    for run in runs {
        if let Some(last) = merged.last_mut()
            && times[run.start] - times[last.end] <= gap_s
        {
            last.end = run.end;
            if run.peak > last.peak {
                last.peak = run.peak;
                last.peak_index = run.peak_index;
            }
            continue;
        }
        merged.push(*run);
    }
    merged
}

/// Integrate `f(i)` over `[start, end]` with the trapezoid rule on `times`.
pub fn integrate(times: &[f64], start: usize, end: usize, f: impl Fn(usize) -> f64) -> f64 {
    if end <= start || end >= times.len() {
        return 0.0;
    }
    let mut area = 0.0;
    let mut prev_v = f(start);
    for i in (start + 1)..=end {
        let v = f(i);
        let dt = times[i] - times[i - 1];
        if prev_v.is_finite() && v.is_finite() && dt > 0.0 {
            area += 0.5 * (prev_v + v) * dt;
        }
        if v.is_finite() {
            prev_v = v;
        }
    }
    area
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masking_drops_sentinels_and_out_of_band() {
        let v = [1.0, -2147483637.0, f64::NAN, 50.0, -5.0];
        let m = mask_invalid(&v, Some((0.0, 10.0)));
        assert_eq!(m[0], 1.0);
        assert!(m[1].is_nan());
        assert!(m[2].is_nan());
        assert!(m[3].is_nan());
        assert!(m[4].is_nan());
        assert!((invalid_fraction(&m) - 0.8).abs() < 1e-9);
        assert!(!is_invalid_sample(-1e6));
    }

    #[test]
    fn steadiness_uses_peak_to_peak() {
        let times: Vec<f64> = (0..10).map(|i| i as f64 * 0.1).collect();
        let values = [
            1000.0, 1050.0, 1100.0, 1300.0, 1000.0, 1000.0, 1000.0, 1000.0, 1000.0, 1000.0,
        ];
        assert_eq!(is_steady(&times, &values, 0.0, 0.3, 200.0), Some(true));
        assert_eq!(is_steady(&times, &values, 0.0, 0.5, 100.0), Some(false));
        assert_eq!(is_steady(&times, &values, 5.0, 6.0, 100.0), None);
        assert_eq!(window_median(&times, &values, 0.4, 1.0), Some(1000.0));
    }

    #[test]
    fn crossing_interpolates_between_update_instants() {
        // Value held for 5 samples per update; deviation rises 0, 0, 0.5, 1.0.
        let times: Vec<f64> = (0..20).map(|i| i as f64 * 0.01).collect();
        let values: Vec<f64> = (0..20).map(|i| [0.0, 0.0, 0.5, 1.0][i / 5]).collect();
        let instants = super::super::stats::update_instants(&values);
        // Instants at 0, 10, 15 (index 5 is a hold of 0.0).
        assert_eq!(instants, vec![0, 10, 15]);
        let c = find_crossing(&times, &instants, 0.75, 0.05, None, |i| values[i]).unwrap();
        assert_eq!(c.index, 15);
        // Halfway between t=0.10 (0.5) and t=0.15 (1.0).
        assert!((c.time - 0.125).abs() < 1e-9);
        assert!(find_crossing(&times, &instants, 2.0, 0.05, None, |i| values[i]).is_none());
        // A long flat hold before the crossing: the previous reading is
        // taken one update interval before, not at the last value change.
        let c = find_crossing(&times, &instants[1..], 0.25, 0.05, Some(0), |i| values[i]).unwrap();
        assert_eq!(c.index, 10);
        assert!((c.time - 0.075).abs() < 1e-9, "{}", c.time);
        // Without a seed the first instant cannot be interpolated.
        let c = find_crossing(&times, &instants[1..], 0.25, 0.05, None, |i| values[i]).unwrap();
        assert!((c.time - 0.10).abs() < 1e-9);
    }

    #[test]
    fn rate_runs_and_merging() {
        let rate = [
            0.0, 60.0, 80.0, 10.0, 70.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 90.0, 95.0,
        ];
        let runs = find_rate_runs(&rate, 50.0, 1);
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].start, 1);
        assert_eq!(runs[0].end, 2);
        assert_eq!(runs[0].peak, 80.0);
        let two = find_rate_runs(&rate, 50.0, 2);
        assert_eq!(two.len(), 2);
        let times: Vec<f64> = (0..rate.len()).map(|i| i as f64 * 0.1).collect();
        let merged = merge_runs(&times, &runs, 0.25);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].start, 1);
        assert_eq!(merged[0].end, 4);
        assert_eq!(merged[0].peak, 80.0);
        assert_eq!(merged[1].peak, 95.0);
    }

    #[test]
    fn trapezoid_integral() {
        let times = [0.0, 1.0, 2.0];
        let v = [0.0, 2.0, 2.0];
        assert!((integrate(&times, 0, 2, |i| v[i]) - 3.0).abs() < 1e-9);
        assert_eq!(integrate(&times, 2, 2, |i| v[i]), 0.0);
    }
}
