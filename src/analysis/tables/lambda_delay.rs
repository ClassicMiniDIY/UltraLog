//! Lambda delay table generator (issue #4).
//!
//! Physical model: a step in injector pulse width enriches the charge, and
//! the wideband reads it after exhaust transport plus sensor response. That
//! total, mapped over RPM × load, is what a closed-loop O2 controller wants
//! in its delay table.
//!
//! Algorithm, per log:
//!
//! 1. Mask invalid samples (sentinels, out-of-band) per role.
//! 2. Find pulse-width steps: the first index where the dead-time-adjusted PW
//!    `step_window_ms` ahead differs from the pre-window median by at least
//!    `min_step_pct`. The step instant is the largest single-sample jump in
//!    that window, i.e. the first sample carrying the new value.
//! 3. Gate: steady RPM / load across the window, no other step within
//!    `min_event_spacing_ms`, PW above the floor, no fuel cut, no clutch, warm
//!    engine, closed-loop correction not moving, enough valid lambda samples.
//! 4. Measure: lambda baseline is the median over the 250 ms before the step;
//!    the response threshold is `max(response_k × σ, min_response_delta)`
//!    where σ is the robust noise of the sensor's *update instants*; the
//!    crossing time is interpolated between update instants. **Dead time**
//!    (first crossing) is the primary value; **t63** (63 % of the settled
//!    deflection) is a secondary view.
//! 5. Bin by RPM × load at the step instant. Median per cell.
//!
//! The measured value includes roughly half an engine cycle of injection
//! scheduling plus a cycle to the exhaust port; that is what a closed-loop
//! delay table should contain, so it is reported, not subtracted.

use std::collections::HashMap;

use super::channel_map::{ChannelMapping, ChannelRole, LoadKind, RoleSpec};
use super::events::{
    any_in_window, find_crossing, invalid_fraction, is_steady, mask_invalid, span, window_median,
};
use super::stats::{
    effective_update_interval, index_range, median, median_interval, robust_sigma_diff,
    update_instants,
};
use super::{
    AxisSpec, GeneratorContext, MeasureSpec, RejectReason, RunReport, TableAnalyzer, TableEvent,
    TableParam, TableParamKind, mapped_column, param_f64, required_column,
};
use crate::analysis::afr::{FuelMixtureUnit, STOICH_AFR_GASOLINE, detect_fuel_mixture_unit};
use crate::analysis::{AnalysisError, AnalyzerConfig, timed_analyze};
use crate::parsers::types::Log;

pub const ID: &str = "lambda_delay";

/// Slowest log rate the generator will work with (4 Hz).
const MAX_SAMPLE_INTERVAL_S: f64 = 0.25;
/// Lambda update interval above which a resolution warning is emitted.
const SLOW_LAMBDA_UPDATE_S: f64 = 0.05;
/// Baseline window before the step for PW / RPM / load medians.
const PW_BASELINE_S: f64 = 0.3;
/// Baseline window before the step for the lambda median.
const LAMBDA_BASELINE_S: f64 = 0.25;
/// Fraction of masked lambda samples in a window that rejects the event.
const MAX_INVALID_FRACTION: f64 = 0.10;
/// Window before the step for the local noise estimate.
const NOISE_WINDOW_S: f64 = 1.0;
/// Update instants the local noise window needs before it is trusted.
const MIN_LOCAL_NOISE_UPDATES: usize = 8;

/// Gating profile: strict for trim-bump logs, relaxed for driving logs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    Strict,
    Relaxed,
}

impl Profile {
    pub const CHOICES: &'static [&'static str] = &["strict", "relaxed"];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Relaxed => "relaxed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "strict" => Some(Self::Strict),
            "relaxed" => Some(Self::Relaxed),
            _ => None,
        }
    }

    /// `(rpm half-band, event spacing ms)` the profile implies. The load band
    /// is unchanged: a 15 kPa rise is a tip-in whose lean-first response
    /// would measure wall wetting, not sensor delay.
    fn gates(self) -> (f64, f64) {
        match self {
            Self::Strict => (200.0, 600.0),
            Self::Relaxed => (400.0, 400.0),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LambdaDelayGenerator {
    /// Minimum PW step relative to the pre-step median, in percent.
    pub min_step_pct: f64,
    /// The step must be complete within this many milliseconds.
    pub step_window_ms: f64,
    /// Another step within this spacing rejects both (attribution guard).
    pub min_event_spacing_ms: f64,
    /// Response threshold multiplier on the sensor noise sigma.
    pub response_k: f64,
    /// Response threshold floor in lambda units (converted for AFR channels).
    pub min_response_delta: f64,
    /// Reject if the sensor has not responded within this window.
    pub response_timeout_ms: f64,
    /// PW below this (in the channel's own unit) is a fuel-cut region.
    pub pw_floor: f64,
    /// Injector dead time subtracted before the relative step test.
    pub injector_deadtime_ms: f64,
    /// RPM half-band for the steadiness gate.
    pub steady_rpm_band: f64,
    /// MAP half-band (kPa) for the steadiness gate.
    pub steady_map_band: f64,
    /// TPS half-band (%) for the steadiness gate.
    pub steady_tps_band: f64,
    /// Closed-loop correction movement (channel units) that rejects an event.
    pub closed_loop_band: f64,
    /// Minimum coolant temperature when a coolant role is mapped.
    pub min_coolant_temp: f64,
    pub profile: Profile,
}

impl Default for LambdaDelayGenerator {
    fn default() -> Self {
        Self {
            min_step_pct: 8.0,
            step_window_ms: 150.0,
            min_event_spacing_ms: 600.0,
            response_k: 3.0,
            min_response_delta: 0.005,
            response_timeout_ms: 1500.0,
            pw_floor: 1.0,
            injector_deadtime_ms: 0.0,
            steady_rpm_band: 200.0,
            steady_map_band: 8.0,
            steady_tps_band: 5.0,
            closed_loop_band: 1.0,
            min_coolant_temp: 60.0,
            profile: Profile::Strict,
        }
    }
}

impl LambdaDelayGenerator {
    /// Apply a profile's gate values.
    pub fn apply_profile(&mut self, profile: Profile) {
        let (rpm, spacing) = profile.gates();
        self.profile = profile;
        self.steady_rpm_band = rpm;
        self.min_event_spacing_ms = spacing;
    }

    fn measures_list() -> Vec<MeasureSpec> {
        vec![
            MeasureSpec {
                key: "dead_time_ms",
                label: "Dead time",
                unit: "ms",
                decimals: 0,
            },
            MeasureSpec {
                key: "t63_ms",
                label: "Rise time (t63)",
                unit: "ms",
                decimals: 0,
            },
            MeasureSpec {
                key: "response_magnitude",
                label: "Response magnitude",
                unit: "λ/AFR",
                decimals: 3,
            },
            MeasureSpec {
                key: "step_pct",
                label: "PW step",
                unit: "%",
                decimals: 1,
            },
        ]
    }
}

/// A pulse-width step before gating.
#[derive(Clone, Copy, Debug)]
struct StepCandidate {
    /// Index of the first sample carrying the new value.
    index: usize,
    /// Relative step size (signed fraction).
    rel: f64,
    /// Pre-step PW median (dead-time adjusted).
    baseline: f64,
}

/// Find PW steps. Returns candidates ordered by time, one per step edge.
fn find_pw_steps(
    times: &[f64],
    pw: &[f64],
    min_step: f64,
    step_window_s: f64,
) -> Vec<StepCandidate> {
    let n = times.len();
    let mut out: Vec<StepCandidate> = Vec::new();
    let mut prev_qualifies = false;
    for i in 0..n {
        let t = times[i];
        let pre = index_range(times, t - PW_BASELINE_S, t);
        if pre.len() < 2 {
            prev_qualifies = false;
            continue;
        }
        let Some(baseline) = median(&pw[pre]) else {
            prev_qualifies = false;
            continue;
        };
        if baseline.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            prev_qualifies = false;
            continue;
        }
        // Last index within the step window.
        let k = times
            .partition_point(|&x| x <= t + step_window_s)
            .saturating_sub(1);
        if k <= i || !pw[k].is_finite() {
            prev_qualifies = false;
            continue;
        }
        let rel = (pw[k] - baseline) / baseline;
        let qualifies = rel.abs() >= min_step;
        if qualifies && !prev_qualifies {
            // Locate the edge: the largest single-sample jump in [i, k].
            let mut best = i;
            let mut best_jump = 0.0;
            for j in i..k {
                if pw[j].is_finite() && pw[j + 1].is_finite() {
                    let jump = ((pw[j + 1] - pw[j]) * rel.signum()).max(0.0);
                    if jump > best_jump {
                        best_jump = jump;
                        best = j;
                    }
                }
            }
            let index = best + 1;
            // A run that broke on noise and restarted re-finds the same edge.
            if out.last().is_none_or(|c| c.index != index) {
                out.push(StepCandidate {
                    index,
                    rel,
                    baseline,
                });
            }
        }
        prev_qualifies = qualifies;
    }
    out
}

impl TableAnalyzer for LambdaDelayGenerator {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "Lambda Delay Table"
    }

    fn description(&self) -> &'static str {
        "Measures the time between injector pulse-width steps and the wideband response, \
         binned by RPM and load, for closed-loop O2 control delay tables."
    }

    fn roles(&self) -> Vec<RoleSpec> {
        vec![
            RoleSpec::required(ChannelRole::Rpm),
            RoleSpec::optional(ChannelRole::Map),
            RoleSpec::optional(ChannelRole::Tps),
            RoleSpec::required(ChannelRole::PulseWidth),
            RoleSpec::required(ChannelRole::Lambda),
            RoleSpec::optional(ChannelRole::FuelCut),
            RoleSpec::optional(ChannelRole::ClosedLoopState),
            RoleSpec::optional(ChannelRole::CoolantTemp),
            RoleSpec::optional(ChannelRole::Clutch),
        ]
    }

    fn measures(&self) -> Vec<MeasureSpec> {
        Self::measures_list()
    }

    fn default_axes(&self, log: &Log, mapping: &ChannelMapping) -> (AxisSpec, AxisSpec) {
        let rpm = mapped_column(log, mapping, ChannelRole::Rpm)
            .ok()
            .flatten()
            .map(|v| mask_invalid(&v, Some((0.0, 20_000.0))))
            .unwrap_or_default();
        let fallback_rpm: Vec<f64> = (1..=16).map(|i| i as f64 * 500.0).collect();
        let x = AxisSpec::from_data("RPM", "", &rpm, 500.0, &fallback_rpm);
        let y = match mapping.load_kind {
            LoadKind::Map => {
                let map = mapped_column(log, mapping, ChannelRole::Map)
                    .ok()
                    .flatten()
                    .map(|v| mask_invalid(&v, Some((-110.0, 600.0))))
                    .unwrap_or_default();
                let fallback: Vec<f64> = (0..=15).map(|i| i as f64 * 20.0).collect();
                AxisSpec::from_data("MAP", "kPa", &map, 10.0, &fallback)
            }
            LoadKind::Tps => {
                let tps = mapped_column(log, mapping, ChannelRole::Tps)
                    .ok()
                    .flatten()
                    .map(|v| mask_invalid(&v, Some((0.0, 100.0))))
                    .unwrap_or_default();
                let fallback: Vec<f64> = (0..=10).map(|i| i as f64 * 10.0).collect();
                AxisSpec::from_data("TPS", "%", &tps, 10.0, &fallback)
            }
        };
        (x, y)
    }

    fn analyze(
        &self,
        log: &Log,
        log_name: &str,
        mapping: &ChannelMapping,
        axes: &(AxisSpec, AxisSpec),
        _ctx: &GeneratorContext<'_>,
    ) -> Result<(Vec<TableEvent>, RunReport), AnalysisError> {
        let times = &log.times;
        if times.len() < 10 {
            return Err(AnalysisError::InsufficientData {
                needed: 10,
                got: times.len(),
            });
        }
        let dt = median_interval(times).ok_or_else(|| {
            AnalysisError::ComputationError("log has no usable time axis".to_string())
        })?;
        if dt > MAX_SAMPLE_INTERVAL_S {
            return Err(AnalysisError::InvalidParameter(format!(
                "log rate is {:.1} Hz; lambda delay needs at least 4 Hz",
                1.0 / dt
            )));
        }

        let rpm = mask_invalid(
            &required_column(log, mapping, ChannelRole::Rpm)?,
            Some((0.0, 20_000.0)),
        );
        let (load, load_band) = match mapping.load_kind {
            LoadKind::Map => (
                mask_invalid(
                    &required_column(log, mapping, ChannelRole::Map)?,
                    Some((-110.0, 600.0)),
                ),
                self.steady_map_band,
            ),
            LoadKind::Tps => (
                mask_invalid(
                    &required_column(log, mapping, ChannelRole::Tps)?,
                    Some((0.0, 100.0)),
                ),
                self.steady_tps_band,
            ),
        };
        let pw_raw = mask_invalid(
            &required_column(log, mapping, ChannelRole::PulseWidth)?,
            Some((0.0, 1e6)),
        );
        let pw: Vec<f64> = pw_raw
            .iter()
            .map(|v| v - self.injector_deadtime_ms)
            .collect();
        let lambda_raw = mask_invalid(&required_column(log, mapping, ChannelRole::Lambda)?, None);
        let unit = detect_fuel_mixture_unit(
            &lambda_raw
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .collect::<Vec<_>>(),
        );
        let lambda_band = match unit {
            FuelMixtureUnit::Lambda => (0.4, 2.0),
            FuelMixtureUnit::Afr => (5.0, 30.0),
        };
        let lambda = mask_invalid(&lambda_raw, Some(lambda_band));
        let fuel_cut = mapped_column(log, mapping, ChannelRole::FuelCut)?;
        let closed_loop = mapped_column(log, mapping, ChannelRole::ClosedLoopState)?
            .map(|v| mask_invalid(&v, None));
        let coolant =
            mapped_column(log, mapping, ChannelRole::CoolantTemp)?.map(|v| mask_invalid(&v, None));
        let clutch = mapped_column(log, mapping, ChannelRole::Clutch)?;

        let mut warnings = Vec::new();
        let update_interval = effective_update_interval(times, &lambda);
        match update_interval {
            Some(u) if u > SLOW_LAMBDA_UPDATE_S => warnings.push(format!(
                "Lambda updates at {:.0} Hz inside a {:.0} Hz log - delay resolution is about ±{:.0} ms",
                1.0 / u,
                1.0 / dt,
                u * 500.0
            )),
            None => warnings.push("Lambda channel never changes value; every event will be rejected".to_string()),
            _ => {}
        }
        if closed_loop.is_none() {
            warnings.push(
                "No closed-loop correction role mapped; results assume open loop".to_string(),
            );
        }
        warnings.push(format!("Lambda channel detected as {}", unit.unit_name()));

        let min_delta = match unit {
            FuelMixtureUnit::Lambda => self.min_response_delta,
            FuelMixtureUnit::Afr => self.min_response_delta * STOICH_AFR_GASOLINE,
        };
        // Whole-log noise estimate; in a driving log most differences are
        // real mixture changes, so a quieter local estimate from the second
        // before each step wins when there are enough updates for one.
        let global_sigma = robust_sigma_diff(&lambda).unwrap_or(0.0);
        let instants = update_instants(&lambda);
        let step_window_s = self.step_window_ms / 1000.0;
        let spacing_s = self.min_event_spacing_ms / 1000.0;
        let timeout_s = self.response_timeout_ms / 1000.0;
        let rate_factor = update_interval.map_or(0.25, |u| (0.02 / u).clamp(0.25, 1.0)) as f32;

        let (events, elapsed) = timed_analyze(|| {
            let candidates = find_pw_steps(times, &pw, self.min_step_pct / 100.0, step_window_s);
            let step_times: Vec<f64> = candidates.iter().map(|c| times[c.index]).collect();
            let mut events = Vec::with_capacity(candidates.len());
            for (ci, cand) in candidates.iter().enumerate() {
                let t_step = times[cand.index];
                let dir = cand.rel.signum();
                let w0 = t_step - PW_BASELINE_S;
                let w1 = t_step + timeout_s;
                let rpm_at = window_median(times, &rpm, w0, t_step).unwrap_or(f64::NAN);
                let load_at = window_median(times, &load, w0, t_step).unwrap_or(f64::NAN);
                let mut values = vec![f64::NAN, f64::NAN, f64::NAN, cand.rel * 100.0];
                let mut note = format!("{} step", if dir > 0.0 { "rising" } else { "falling" });
                let event = |reject: Option<RejectReason>,
                             values: Vec<f64>,
                             quality: f32,
                             note: String| TableEvent {
                    log_id: 0,
                    log_name: log_name.to_string(),
                    time: t_step,
                    rpm: rpm_at,
                    axis_value: load_at,
                    values,
                    quality,
                    reject,
                    note,
                };

                let overlap = step_times
                    .iter()
                    .enumerate()
                    .any(|(j, &t)| j != ci && (t - t_step).abs() < spacing_s);
                if overlap {
                    events.push(event(Some(RejectReason::Overlap), values, 0.0, note));
                    continue;
                }
                if cand.baseline < self.pw_floor {
                    events.push(event(Some(RejectReason::LowPw), values, 0.0, note));
                    continue;
                }
                if let Some(fc) = &fuel_cut
                    && any_in_window(times, fc, w0, w1, |v| v > 0.5)
                {
                    events.push(event(Some(RejectReason::FuelCut), values, 0.0, note));
                    continue;
                }
                if let Some(cl) = &clutch
                    && any_in_window(times, cl, w0, w1, |v| v > 0.5)
                {
                    events.push(event(Some(RejectReason::Clutch), values, 0.0, note));
                    continue;
                }
                if let Some(ct) = &coolant
                    && window_median(times, ct, w0, w1).is_some_and(|c| c < self.min_coolant_temp)
                {
                    events.push(event(Some(RejectReason::ColdEngine), values, 0.0, note));
                    continue;
                }
                if let Some(cl) = &closed_loop
                    && span(
                        cl,
                        index_range(times, t_step - step_window_s, t_step + step_window_s),
                    )
                    .is_some_and(|s| s > self.closed_loop_band)
                {
                    events.push(event(
                        Some(RejectReason::ClosedLoopActive),
                        values,
                        0.0,
                        note,
                    ));
                    continue;
                }
                let steady_rpm = is_steady(times, &rpm, w0, w1, self.steady_rpm_band);
                let steady_load = is_steady(times, &load, w0, w1, load_band);
                if steady_rpm != Some(true)
                    || steady_load != Some(true)
                    || !rpm_at.is_finite()
                    || !load_at.is_finite()
                {
                    events.push(event(Some(RejectReason::Unsteady), values, 0.0, note));
                    continue;
                }
                let window = index_range(times, w0, w1);
                if window.is_empty()
                    || invalid_fraction(&lambda[window.clone()]) > MAX_INVALID_FRACTION
                    || span(&lambda, window.clone()).is_none_or(|s| s <= 0.0)
                {
                    events.push(event(Some(RejectReason::InvalidSamples), values, 0.0, note));
                    continue;
                }
                if axes.0.bin_index(rpm_at).is_none() || axes.1.bin_index(load_at).is_none() {
                    events.push(event(Some(RejectReason::OutOfAxis), values, 0.0, note));
                    continue;
                }

                // Response measurement.
                let pre = index_range(times, t_step - NOISE_WINDOW_S, t_step);
                let sigma = match robust_sigma_diff(&lambda[pre.clone()]) {
                    Some(local)
                        if update_instants(&lambda[pre]).len() >= MIN_LOCAL_NOISE_UPDATES =>
                    {
                        local.min(global_sigma)
                    }
                    _ => global_sigma,
                };
                let threshold = (self.response_k * sigma).max(min_delta);
                let Some(base_lambda) =
                    window_median(times, &lambda, t_step - LAMBDA_BASELINE_S, t_step)
                else {
                    events.push(event(Some(RejectReason::InvalidSamples), values, 0.0, note));
                    continue;
                };
                // More fuel -> lambda / AFR falls. dev is positive in the
                // expected direction.
                let dev = |i: usize| (base_lambda - lambda[i]) * dir;
                let lo = times.partition_point(|&x| x <= t_step);
                let hi = times.partition_point(|&x| x <= w1);
                let post: Vec<usize> = instants
                    .iter()
                    .copied()
                    .filter(|&i| i >= lo && i < hi)
                    .collect();
                let seed = instants.iter().copied().rfind(|&i| i < lo);
                let interval = update_interval.unwrap_or(dt);
                let expected = find_crossing(times, &post, threshold, interval, seed, dev);
                let wrong = find_crossing(times, &post, threshold, interval, seed, |i| -dev(i));
                let crossing = match (expected, wrong) {
                    (Some(e), Some(w)) if w.time < e.time => {
                        events.push(event(Some(RejectReason::WrongDirection), values, 0.0, note));
                        continue;
                    }
                    (None, Some(_)) => {
                        events.push(event(Some(RejectReason::WrongDirection), values, 0.0, note));
                        continue;
                    }
                    (None, None) => {
                        events.push(event(Some(RejectReason::NoResponse), values, 0.0, note));
                        continue;
                    }
                    (Some(e), _) => e,
                };
                let dead_time_ms = (crossing.time - t_step) * 1000.0;
                let magnitude = post
                    .iter()
                    .filter(|&&i| i >= crossing.index)
                    .map(|&i| dev(i))
                    .fold(0.0_f64, f64::max);
                let t63 = find_crossing(times, &post, 0.63 * magnitude, interval, seed, dev)
                    .map(|c| (c.time - t_step) * 1000.0)
                    .unwrap_or(f64::NAN);
                values[0] = dead_time_ms;
                values[1] = t63;
                values[2] = magnitude;
                let quality = ((magnitude / threshold / 3.0).clamp(0.0, 1.0) as f32) * rate_factor;
                note.push_str(&format!(
                    ", threshold {:.4} {}, {} profile",
                    threshold,
                    unit.unit_name(),
                    self.profile.as_str()
                ));
                events.push(event(None, values, quality, note));
            }
            events
        });

        let mut report = RunReport::from_events(log_name, &events);
        report.warnings = warnings;
        report.computation_time_ms = elapsed;
        Ok((events, report))
    }

    fn params(&self) -> Vec<TableParam> {
        vec![
            TableParam {
                key: "profile",
                label: "Gating profile",
                tooltip: "Strict for trim-bump logs (RPM ±200, 600 ms spacing); relaxed for driving logs (RPM ±400, 400 ms).",
                kind: TableParamKind::Choice(Profile::CHOICES),
            },
            TableParam {
                key: "min_step_pct",
                label: "Min PW step (%)",
                tooltip: "Minimum pulse-width change relative to the pre-step median.",
                kind: TableParamKind::Float {
                    min: 1.0,
                    max: 100.0,
                    speed: 0.5,
                },
            },
            TableParam {
                key: "step_window_ms",
                label: "Step window (ms)",
                tooltip: "The step must complete within this time.",
                kind: TableParamKind::Float {
                    min: 20.0,
                    max: 1000.0,
                    speed: 5.0,
                },
            },
            TableParam {
                key: "min_event_spacing_ms",
                label: "Event spacing (ms)",
                tooltip: "Steps closer together than this are rejected as overlapping.",
                kind: TableParamKind::Float {
                    min: 100.0,
                    max: 5000.0,
                    speed: 10.0,
                },
            },
            TableParam {
                key: "response_k",
                label: "Response k (× noise σ)",
                tooltip: "Response threshold as a multiple of the sensor noise.",
                kind: TableParamKind::Float {
                    min: 1.0,
                    max: 10.0,
                    speed: 0.1,
                },
            },
            TableParam {
                key: "min_response_delta",
                label: "Min response (λ)",
                tooltip: "Floor for the response threshold in lambda units (scaled ×14.7 for AFR channels).",
                kind: TableParamKind::Float {
                    min: 0.001,
                    max: 0.2,
                    speed: 0.001,
                },
            },
            TableParam {
                key: "response_timeout_ms",
                label: "Response timeout (ms)",
                tooltip: "Reject the event if the sensor has not responded within this time.",
                kind: TableParamKind::Float {
                    min: 200.0,
                    max: 5000.0,
                    speed: 10.0,
                },
            },
            TableParam {
                key: "pw_floor",
                label: "PW floor",
                tooltip: "Pulse width below this (channel units) is treated as fuel cut.",
                kind: TableParamKind::Float {
                    min: 0.0,
                    max: 100.0,
                    speed: 0.1,
                },
            },
            TableParam {
                key: "injector_deadtime_ms",
                label: "Injector dead time (ms)",
                tooltip: "Subtracted before the relative step test when the PW channel includes dead time.",
                kind: TableParamKind::Float {
                    min: 0.0,
                    max: 5.0,
                    speed: 0.01,
                },
            },
            TableParam {
                key: "steady_rpm_band",
                label: "Steady RPM ±",
                tooltip: "RPM half-band the event window must stay within.",
                kind: TableParamKind::Float {
                    min: 25.0,
                    max: 2000.0,
                    speed: 5.0,
                },
            },
            TableParam {
                key: "steady_map_band",
                label: "Steady MAP ± (kPa)",
                tooltip: "MAP half-band the event window must stay within.",
                kind: TableParamKind::Float {
                    min: 1.0,
                    max: 100.0,
                    speed: 0.5,
                },
            },
            TableParam {
                key: "steady_tps_band",
                label: "Steady TPS ± (%)",
                tooltip: "TPS half-band the event window must stay within.",
                kind: TableParamKind::Float {
                    min: 1.0,
                    max: 50.0,
                    speed: 0.5,
                },
            },
            TableParam {
                key: "closed_loop_band",
                label: "Closed-loop movement",
                tooltip: "Correction-channel movement around the step that rejects the event.",
                kind: TableParamKind::Float {
                    min: 0.1,
                    max: 50.0,
                    speed: 0.1,
                },
            },
            TableParam {
                key: "min_coolant_temp",
                label: "Min coolant temp",
                tooltip: "Events below this coolant temperature are rejected (channel units).",
                kind: TableParamKind::Float {
                    min: -40.0,
                    max: 400.0,
                    speed: 1.0,
                },
            },
        ]
    }

    fn get_config(&self) -> AnalyzerConfig {
        let mut p = HashMap::new();
        p.insert("profile".into(), self.profile.as_str().to_string());
        p.insert("min_step_pct".into(), self.min_step_pct.to_string());
        p.insert("step_window_ms".into(), self.step_window_ms.to_string());
        p.insert(
            "min_event_spacing_ms".into(),
            self.min_event_spacing_ms.to_string(),
        );
        p.insert("response_k".into(), self.response_k.to_string());
        p.insert(
            "min_response_delta".into(),
            self.min_response_delta.to_string(),
        );
        p.insert(
            "response_timeout_ms".into(),
            self.response_timeout_ms.to_string(),
        );
        p.insert("pw_floor".into(), self.pw_floor.to_string());
        p.insert(
            "injector_deadtime_ms".into(),
            self.injector_deadtime_ms.to_string(),
        );
        p.insert("steady_rpm_band".into(), self.steady_rpm_band.to_string());
        p.insert("steady_map_band".into(), self.steady_map_band.to_string());
        p.insert("steady_tps_band".into(), self.steady_tps_band.to_string());
        p.insert("closed_loop_band".into(), self.closed_loop_band.to_string());
        p.insert("min_coolant_temp".into(), self.min_coolant_temp.to_string());
        AnalyzerConfig {
            id: ID.to_string(),
            name: self.name().to_string(),
            parameters: p,
        }
    }

    fn set_config(&mut self, config: &AnalyzerConfig) {
        if let Some(p) = config
            .parameters
            .get("profile")
            .and_then(|s| Profile::parse(s))
            && p != self.profile
        {
            self.apply_profile(p);
        }
        self.min_step_pct = param_f64(config, "min_step_pct", self.min_step_pct);
        self.step_window_ms = param_f64(config, "step_window_ms", self.step_window_ms);
        self.min_event_spacing_ms =
            param_f64(config, "min_event_spacing_ms", self.min_event_spacing_ms);
        self.response_k = param_f64(config, "response_k", self.response_k);
        self.min_response_delta = param_f64(config, "min_response_delta", self.min_response_delta);
        self.response_timeout_ms =
            param_f64(config, "response_timeout_ms", self.response_timeout_ms);
        self.pw_floor = param_f64(config, "pw_floor", self.pw_floor);
        self.injector_deadtime_ms =
            param_f64(config, "injector_deadtime_ms", self.injector_deadtime_ms);
        self.steady_rpm_band = param_f64(config, "steady_rpm_band", self.steady_rpm_band);
        self.steady_map_band = param_f64(config, "steady_map_band", self.steady_map_band);
        self.steady_tps_band = param_f64(config, "steady_tps_band", self.steady_tps_band);
        self.closed_loop_band = param_f64(config, "closed_loop_band", self.closed_loop_band);
        self.min_coolant_temp = param_f64(config, "min_coolant_temp", self.min_coolant_temp);
    }

    fn clone_box(&self) -> Box<dyn TableAnalyzer> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::super::synthetic::{SyntheticLog, Xorshift};
    use super::*;

    fn mapping() -> ChannelMapping {
        let mut m = ChannelMapping::default();
        m.set(ChannelRole::Rpm, Some("RPM".into()));
        m.set(ChannelRole::Map, Some("MAP".into()));
        m.set(ChannelRole::PulseWidth, Some("PW".into()));
        m.set(ChannelRole::Lambda, Some("Lambda".into()));
        m.load_kind = LoadKind::Map;
        m
    }

    fn axes() -> (AxisSpec, AxisSpec) {
        (
            AxisSpec::new("RPM", "", vec![1000.0, 2000.0, 3000.0, 4000.0]),
            AxisSpec::new("MAP", "kPa", vec![20.0, 60.0, 100.0]),
        )
    }

    /// A log with steady RPM/MAP whose PW toggles between two levels every
    /// 3 s (alternating rising / falling 10 % steps), each followed by a
    /// first-order lambda move to the new level after `delay_s`. The lambda
    /// response depth is `depth` (0.05 for a 10 % PW step at stoich).
    fn step_log_depth(
        rate_hz: f64,
        update_hz: f64,
        delay_s: f64,
        tau_s: f64,
        sigma: f64,
        steps: usize,
        depth: f64,
    ) -> SyntheticLog {
        let duration = 3.0 * steps as f64 + 2.0;
        let mut log = SyntheticLog::new(rate_hz, duration);
        let n = log.times.len();
        let mut rng = Xorshift::new(42);
        let mut pw = vec![2.0; n];
        let mut lambda_true = vec![1.0; n];
        let level = |s: usize| if s.is_multiple_of(2) { 2.2 } else { 2.0 };
        let lambda_level = |s: usize| {
            if s.is_multiple_of(2) {
                1.0 - depth
            } else {
                1.0
            }
        };
        for i in 0..n {
            let t = log.times[i];
            // Most recent step at or before t.
            let s = ((t - 2.0) / 3.0).floor();
            if s < 0.0 {
                continue;
            }
            let s = (s as usize).min(steps - 1);
            let t0 = 2.0 + 3.0 * s as f64;
            pw[i] = level(s);
            let prev = if s == 0 { 1.0 } else { lambda_level(s - 1) };
            let x = t - t0 - delay_s;
            lambda_true[i] = if x < 0.0 {
                prev
            } else {
                prev + (lambda_level(s) - prev) * (1.0 - (-x / tau_s).exp())
            };
        }
        let lambda = log.sample_and_hold(&lambda_true, update_hz, sigma, &mut rng);
        log.add("RPM", vec![2500.0; n]);
        log.add("MAP", vec![50.0; n]);
        log.add("PW", pw);
        log.add("Lambda", lambda);
        log
    }

    fn step_log(
        rate_hz: f64,
        update_hz: f64,
        delay_s: f64,
        tau_s: f64,
        sigma: f64,
        steps: usize,
    ) -> SyntheticLog {
        step_log_depth(rate_hz, update_hz, delay_s, tau_s, sigma, steps, 0.05)
    }

    fn run(log: &SyntheticLog, gen_: &LambdaDelayGenerator) -> (Vec<TableEvent>, RunReport) {
        gen_.analyze(
            &log.log,
            "synthetic",
            &mapping(),
            &axes(),
            &GeneratorContext::default(),
        )
        .expect("analysis runs")
    }

    #[test]
    fn recovers_known_delay_across_rates_and_noise() {
        let gen_ = LambdaDelayGenerator::default();
        for &delay in &[0.06, 0.12, 0.25, 0.5] {
            for &tau in &[0.03, 0.08] {
                for &rate in &[10.0, 20.0, 50.0, 100.0] {
                    for &(update, sigma) in &[
                        (rate, 0.0),
                        (rate, 0.005),
                        (10.0_f64.min(rate), 0.005),
                        (rate, 0.02),
                    ] {
                        // A 3 sigma threshold needs a response well above
                        // the noise; real widebands sit near 0.005 lambda.
                        let depth = if sigma > 0.01 { 0.15 } else { 0.05 };
                        let log = step_log_depth(rate, update, delay, tau, sigma, 6, depth);
                        let (events, report) = run(&log, &gen_);
                        assert!(
                            report.accepted >= 4,
                            "rate {rate} update {update} sigma {sigma} delay {delay} tau {tau}: {}",
                            report.summary()
                        );
                        let interval = 1.0 / update.min(rate);
                        // The dead time is the first crossing of the noise
                        // threshold, so on a first-order response it sits a
                        // little after the true onset (bias) and jitters with
                        // the noise over the local slope.
                        let thr = (3.0 * sigma).max(0.005);
                        let bias = tau * (1.0 / (1.0 - thr / depth)).ln();
                        let jitter = 3.0 * sigma * tau / (depth * (1.0 - thr / depth));
                        let tol = (0.5 * interval).max(0.01) + bias + jitter;
                        for e in events.iter().filter(|e| e.accepted()) {
                            let err = (e.value(0) / 1000.0 - delay).abs();
                            assert!(
                                err <= tol,
                                "rate {rate} update {update} sigma {sigma} delay {delay} tau {tau}: measured {} ms, expected {} ms (tol {} ms)",
                                e.value(0),
                                delay * 1000.0,
                                tol * 1000.0
                            );
                            assert!(e.value(1).is_nan() || e.value(1) >= e.value(0));
                            assert!(e.value(2) > 0.0);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn afr_input_matches_lambda_input() {
        let gen_ = LambdaDelayGenerator::default();
        let log = step_log(50.0, 50.0, 0.12, 0.05, 0.003, 4);
        let (lambda_events, _) = run(&log, &gen_);
        let mut afr_log = log.clone();
        afr_log.scale("Lambda", 14.7);
        let (afr_events, _) = run(&afr_log, &gen_);
        assert_eq!(lambda_events.len(), afr_events.len());
        for (a, b) in lambda_events.iter().zip(afr_events.iter()) {
            assert_eq!(a.reject, b.reject);
            if a.accepted() {
                assert!(
                    (a.value(0) - b.value(0)).abs() < 1.0,
                    "{} vs {}",
                    a.value(0),
                    b.value(0)
                );
            }
        }
    }

    #[test]
    fn refuses_logs_slower_than_4hz() {
        let log = step_log(3.0, 3.0, 0.12, 0.05, 0.0, 2);
        let err = LambdaDelayGenerator::default()
            .analyze(
                &log.log,
                "slow",
                &mapping(),
                &axes(),
                &GeneratorContext::default(),
            )
            .unwrap_err();
        assert!(matches!(err, AnalysisError::InvalidParameter(_)));
    }

    #[test]
    fn slow_lambda_update_emits_warning() {
        let log = step_log(100.0, 10.0, 0.12, 0.05, 0.0, 2);
        let (_, report) = run(&log, &LambdaDelayGenerator::default());
        assert!(
            report.warnings.iter().any(|w| w.contains("10 Hz")),
            "{:?}",
            report.warnings
        );
    }

    #[test]
    fn rpm_ramp_is_unsteady() {
        let mut log = step_log(50.0, 50.0, 0.12, 0.05, 0.0, 3);
        let n = log.times.len();
        let ramp: Vec<f64> = (0..n).map(|i| 1500.0 + i as f64 * 10.0).collect();
        log.replace("RPM", ramp);
        let (_, report) = run(&log, &LambdaDelayGenerator::default());
        assert_eq!(report.accepted, 0);
        assert!(
            report
                .rejected
                .get(&RejectReason::Unsteady)
                .copied()
                .unwrap_or(0)
                >= 3,
            "{}",
            report.summary()
        );
    }

    #[test]
    fn flat_lambda_is_invalid_and_no_response_is_reported() {
        let mut log = step_log(50.0, 50.0, 0.12, 0.05, 0.0, 3);
        let n = log.times.len();
        log.replace("Lambda", vec![1.0; n]);
        let (_, report) = run(&log, &LambdaDelayGenerator::default());
        assert_eq!(report.accepted, 0);
        assert_eq!(
            report.rejected.get(&RejectReason::InvalidSamples).copied(),
            Some(3),
            "{}",
            report.summary()
        );

        // Sensor that moves (noise) but never responds to the step.
        let mut log = step_log(50.0, 50.0, 0.12, 0.05, 0.0, 3);
        let mut rng = Xorshift::new(7);
        let noisy: Vec<f64> = (0..n).map(|_| 1.0 + rng.normal() * 0.001).collect();
        log.replace("Lambda", noisy);
        let (_, report) = run(&log, &LambdaDelayGenerator::default());
        assert_eq!(report.accepted, 0);
        assert!(
            report.rejected.contains_key(&RejectReason::NoResponse),
            "{}",
            report.summary()
        );
    }

    #[test]
    fn sentinels_reject_as_invalid_samples() {
        let mut log = step_log(50.0, 50.0, 0.12, 0.05, 0.003, 3);
        let mut lambda = log.column("Lambda");
        // Blank out 30 % of the samples around the second step with Haltech's sentinel.
        for (i, v) in lambda.iter_mut().enumerate() {
            let t = log.times[i];
            if (4.5..6.5).contains(&t) && i % 3 == 0 {
                *v = -2147483637.0;
            }
        }
        log.replace("Lambda", lambda);
        let (events, _) = run(&log, &LambdaDelayGenerator::default());
        let second = events.iter().find(|e| (e.time - 5.0).abs() < 0.1).unwrap();
        assert_eq!(second.reject, Some(RejectReason::InvalidSamples));
        assert!(events.iter().filter(|e| e.accepted()).count() >= 2);
    }

    #[test]
    fn reversed_direction_is_rejected() {
        // Lambda goes *lean* on a rising PW step: wrong direction.
        let mut log = step_log(50.0, 50.0, 0.12, 0.05, 0.0, 2);
        let lambda: Vec<f64> = log.column("Lambda").iter().map(|v| 2.0 - v).collect();
        log.replace("Lambda", lambda);
        let (_, report) = run(&log, &LambdaDelayGenerator::default());
        assert_eq!(report.accepted, 0);
        assert_eq!(
            report.rejected.get(&RejectReason::WrongDirection).copied(),
            Some(2),
            "{}",
            report.summary()
        );
    }

    #[test]
    fn dead_time_adjusted_step_passes_only_with_deadtime_param() {
        // 2.0 ms PW with 1.0 ms dead time: a 7 % bump in the effective 1.0 ms
        // part reads as 3.5 % on the raw channel.
        let mut log = step_log(50.0, 50.0, 0.12, 0.05, 0.0, 3);
        let pw: Vec<f64> = log
            .column("PW")
            .iter()
            .map(|v| 1.0 + (v - 2.0) * 0.7 + 1.0)
            .collect();
        log.replace("PW", pw);
        let (_, report) = run(&log, &LambdaDelayGenerator::default());
        assert_eq!(report.candidates, 0);
        let mut gen_ = LambdaDelayGenerator::default();
        let mut cfg = gen_.get_config();
        cfg.parameters
            .insert("injector_deadtime_ms".into(), "1.0".into());
        gen_.set_config(&cfg);
        let (_, report) = run(&log, &gen_);
        assert!(report.accepted >= 2, "{}", report.summary());
    }

    #[test]
    fn overlapping_steps_reject_and_relaxed_profile_widens_spacing() {
        let mut log = step_log(50.0, 50.0, 0.12, 0.05, 0.0, 2);
        // Add a second step 0.5 s after the first.
        let mut pw = log.column("PW");
        for (i, v) in pw.iter_mut().enumerate() {
            let t = log.times[i];
            if (2.5..5.0).contains(&t) {
                *v = 2.6;
            }
        }
        log.replace("PW", pw);
        let (_, strict) = run(&log, &LambdaDelayGenerator::default());
        assert!(
            strict.rejected.contains_key(&RejectReason::Overlap),
            "{}",
            strict.summary()
        );
        let mut gen_ = LambdaDelayGenerator::default();
        gen_.apply_profile(Profile::Relaxed);
        assert_eq!(gen_.min_event_spacing_ms, 400.0);
        assert_eq!(gen_.steady_rpm_band, 400.0);
        let (_, relaxed) = run(&log, &gen_);
        assert!(
            relaxed
                .rejected
                .get(&RejectReason::Overlap)
                .copied()
                .unwrap_or(0)
                < strict.rejected[&RejectReason::Overlap]
        );
    }

    #[test]
    fn optional_gates_reject() {
        let base = step_log(50.0, 50.0, 0.12, 0.05, 0.0, 3);
        let n = base.times.len();
        let flag: Vec<f64> = (0..n)
            .map(|i| {
                if (4.5..6.0).contains(&base.times[i]) {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();

        let mut log = base.clone();
        log.add("DFCO", flag.clone());
        let mut m = mapping();
        m.set(ChannelRole::FuelCut, Some("DFCO".into()));
        let (events, _) = LambdaDelayGenerator::default()
            .analyze(&log.log, "s", &m, &axes(), &GeneratorContext::default())
            .unwrap();
        assert_eq!(events[1].reject, Some(RejectReason::FuelCut));
        assert!(events[0].accepted());

        let mut log = base.clone();
        log.add("Clutch", flag.clone());
        let mut m = mapping();
        m.set(ChannelRole::Clutch, Some("Clutch".into()));
        let (events, _) = LambdaDelayGenerator::default()
            .analyze(&log.log, "s", &m, &axes(), &GeneratorContext::default())
            .unwrap();
        assert_eq!(events[1].reject, Some(RejectReason::Clutch));

        let mut log = base.clone();
        log.add("CLT", vec![40.0; n]);
        let mut m = mapping();
        m.set(ChannelRole::CoolantTemp, Some("CLT".into()));
        let (_, report) = LambdaDelayGenerator::default()
            .analyze(&log.log, "s", &m, &axes(), &GeneratorContext::default())
            .unwrap();
        assert_eq!(
            report.rejected.get(&RejectReason::ColdEngine).copied(),
            Some(3)
        );

        let mut log = base.clone();
        let stft: Vec<f64> = flag.iter().map(|f| f * 5.0).collect();
        log.add("STFT", stft);
        let mut m = mapping();
        m.set(ChannelRole::ClosedLoopState, Some("STFT".into()));
        let (events, report) = LambdaDelayGenerator::default()
            .analyze(&log.log, "s", &m, &axes(), &GeneratorContext::default())
            .unwrap();
        assert!(!report.warnings.iter().any(|w| w.contains("open loop")));
        // The trim steps 0.5 s before the 5.0 s event, outside the ±150 ms
        // window, so that event still passes; a move at the step rejects it.
        assert!(events[1].accepted(), "{:?}", events[1].reject);
        let mut log = base.clone();
        let stft: Vec<f64> = (0..n)
            .map(|i| if base.times[i] >= 5.05 { 5.0 } else { 0.0 })
            .collect();
        log.add("STFT", stft);
        let (events, _) = LambdaDelayGenerator::default()
            .analyze(&log.log, "s", &m, &axes(), &GeneratorContext::default())
            .unwrap();
        assert_eq!(events[1].reject, Some(RejectReason::ClosedLoopActive));
    }

    #[test]
    fn out_of_axis_and_low_pw() {
        let log = step_log(50.0, 50.0, 0.12, 0.05, 0.0, 2);
        let narrow = (
            AxisSpec::new("RPM", "", vec![3000.0, 4000.0]),
            AxisSpec::new("MAP", "kPa", vec![20.0, 100.0]),
        );
        let (_, report) = LambdaDelayGenerator::default()
            .analyze(
                &log.log,
                "s",
                &mapping(),
                &narrow,
                &GeneratorContext::default(),
            )
            .unwrap();
        assert_eq!(
            report.rejected.get(&RejectReason::OutOfAxis).copied(),
            Some(2)
        );

        let gen_ = LambdaDelayGenerator {
            pw_floor: 5.0,
            ..Default::default()
        };
        let (_, report) = run(&log, &gen_);
        assert_eq!(report.rejected.get(&RejectReason::LowPw).copied(), Some(2));
    }

    #[test]
    fn ragged_log_is_an_error_not_a_panic() {
        let mut log = step_log(20.0, 20.0, 0.12, 0.05, 0.0, 2);
        log.log.data[3].pop();
        let err = LambdaDelayGenerator::default()
            .analyze(
                &log.log,
                "ragged",
                &mapping(),
                &axes(),
                &GeneratorContext::default(),
            )
            .unwrap_err();
        assert!(matches!(err, AnalysisError::ComputationError(_)), "{err}");
    }

    #[test]
    fn config_round_trip() {
        let mut gen_ = LambdaDelayGenerator::default();
        let mut cfg = gen_.get_config();
        cfg.parameters.insert("min_step_pct".into(), "12".into());
        cfg.parameters.insert("profile".into(), "relaxed".into());
        cfg.parameters.insert("response_k".into(), "garbage".into());
        gen_.set_config(&cfg);
        assert_eq!(gen_.min_step_pct, 12.0);
        assert_eq!(gen_.profile, Profile::Relaxed);
        assert_eq!(gen_.response_k, 3.0);
        assert_eq!(gen_.get_config().parameters["profile"], "relaxed");
        for p in gen_.params() {
            assert!(
                cfg.parameters.contains_key(p.key),
                "param {} missing from config",
                p.key
            );
        }
    }

    #[test]
    fn default_axes_follow_data() {
        let log = step_log(20.0, 20.0, 0.12, 0.05, 0.0, 1);
        let (x, y) = LambdaDelayGenerator::default().default_axes(&log.log, &mapping());
        assert!(x.is_valid() && y.is_valid());
        assert_eq!(x.label, "RPM");
        assert_eq!(y.unit, "kPa");
    }
}
