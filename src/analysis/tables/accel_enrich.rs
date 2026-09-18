//! Acceleration enrichment table generator (issue #3).
//!
//! Physical model: on tip-in, airflow rises faster than the fuel film
//! delivers and the mixture spikes lean for 100 ms to 1 s. Per RPM × tip-in
//! rate cell the tuner wants the depth and duration of that excursion and a
//! starting-point correction.
//!
//! Algorithm, per log:
//!
//! 1. Throttle rate: the native derivative channel when mapped (with its
//!    scale detected against a computed derivative, Haltech logs ×10), else
//!    the derivative of a median-filtered TPS, else MAP rate as a fallback.
//! 2. Tip-in events: runs of at least two samples above the rate threshold;
//!    runs closer than `merge_gap_ms` merge into the larger event.
//! 3. Lambda delay compensation: the AFR window is shifted by the matching
//!    cell of a lambda-delay table from the same session when that cell is
//!    Medium/High confidence, else by `assumed_delay_ms`.
//! 4. Excursion: signed relative deviation from the reference (mapped target
//!    channel, else pre-event baseline). Peak, duration above the lean band,
//!    and area until recovery.
//! 5. Suggested correction: the steady-flow fuel deficit at the peak,
//!    `peak deviation × 100 %`, clamped; **a starting point, not a final
//!    value**. When an ECU AE-activity role is mapped the excursion is the
//!    residual on top of what the ECU already added, so the correction is
//!    *additional* and multiplies the ECU's current value.
//! 6. Bin by RPM × peak rate. Median per cell.

use std::collections::HashMap;

use super::channel_map::{ChannelMapping, ChannelRole, RoleSpec};
use super::events::{
    any_in_window, find_rate_runs, integrate, invalid_fraction, mask_invalid, merge_runs,
    window_median,
};
use super::stats::{index_range, median, median_interval, update_instants};
use super::{
    AxisSpec, Confidence, GeneratorContext, MeasureSpec, RejectReason, RunReport, TableAnalyzer,
    TableEvent, TableParam, TableParamKind, mapped_column, param_f64, required_column,
};
use crate::analysis::afr::{FuelMixtureUnit, STOICH_AFR_GASOLINE, detect_fuel_mixture_unit};
use crate::analysis::filters::median_filter;
use crate::analysis::statistics::time_derivative;
use crate::analysis::{AnalysisError, AnalyzerConfig, timed_analyze};
use crate::parsers::types::Log;

pub const ID: &str = "accel_enrich";

/// Slowest log rate the generator will work with (4 Hz).
const MAX_SAMPLE_INTERVAL_S: f64 = 0.25;
/// Baseline window before the tip-in for RPM / lambda medians.
const BASELINE_S: f64 = 0.25;
/// Time the deviation must stay inside the recovery band to end the event.
const RECOVERY_HOLD_S: f64 = 0.1;
const MAX_INVALID_FRACTION: f64 = 0.10;
/// Geometric rate axis for TPS rate (%/s).
const TPS_RATE_EDGES: [f64; 8] = [25.0, 50.0, 100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0];
/// Geometric rate axis for MAP rate (kPa/s).
const MAP_RATE_EDGES: [f64; 7] = [100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0];

/// Whether the table's suggestions add to, or replace, the ECU's AE value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorrectionKind {
    /// No ECU AE-activity role mapped: the excursion is the whole deficit.
    Absolute,
    /// An AE-activity role is mapped: the excursion is the residual on top
    /// of the ECU's current enrichment, so multiply the ECU value.
    Additional,
}

impl CorrectionKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Absolute => "absolute",
            Self::Additional => "additional to current AE",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccelEnrichGenerator {
    /// Tip-in trigger on throttle rate (%/s).
    pub tps_rate_threshold: f64,
    /// Tip-in trigger on MAP rate (kPa/s) when no throttle channel is mapped.
    pub map_rate_threshold: f64,
    /// Lambda delay used when no session delay table covers the cell.
    pub assumed_delay_ms: f64,
    /// Relative deviation counted as lean/rich for the duration measure.
    pub lean_band: f64,
    /// Relative deviation inside which the event is considered recovered.
    pub recovery_band: f64,
    /// AFR window length after the (delay-shifted) tip-in.
    pub max_event_ms: f64,
    /// RPM drop over the window that marks a gear shift (a rise is the
    /// engine responding to the tip-in).
    pub max_rpm_change_pct: f64,
    /// Clamp on the suggested correction.
    pub correction_clamp_pct: f64,
    /// Accel time the area-based suggestion is spread over.
    pub area_over_ms: f64,
    /// Scale applied to a native rate channel; 0 = detect automatically.
    pub tps_rate_scale: f64,
    /// Rate runs closer than this merge into one event.
    pub merge_gap_ms: f64,
    /// Minimum coolant temperature when a coolant role is mapped.
    pub min_coolant_temp: f64,
}

impl Default for AccelEnrichGenerator {
    fn default() -> Self {
        Self {
            tps_rate_threshold: 50.0,
            map_rate_threshold: 400.0,
            assumed_delay_ms: 120.0,
            lean_band: 0.02,
            recovery_band: 0.01,
            max_event_ms: 2000.0,
            max_rpm_change_pct: 25.0,
            correction_clamp_pct: 50.0,
            area_over_ms: 300.0,
            tps_rate_scale: 0.0,
            merge_gap_ms: 300.0,
            min_coolant_temp: 60.0,
        }
    }
}

/// Which signal triggered tip-in detection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RateSource {
    NativeTpsRate,
    ComputedTpsRate,
    ComputedMapRate,
}

impl RateSource {
    fn axis(self) -> (&'static str, &'static str, &'static [f64]) {
        match self {
            Self::NativeTpsRate | Self::ComputedTpsRate => ("TPS rate", "%/s", &TPS_RATE_EDGES),
            Self::ComputedMapRate => ("MAP rate", "kPa/s", &MAP_RATE_EDGES),
        }
    }
}

/// Detect the scale of a native rate channel against a computed derivative:
/// the median ratio over samples where the computed rate is clearly above
/// `threshold`. Haltech's `Throttle Position Derivative` is ×10.
pub fn detect_rate_scale(native: &[f64], computed: &[f64], threshold: f64) -> f64 {
    let ratios: Vec<f64> = native
        .iter()
        .zip(computed)
        .filter(|(n, c)| n.is_finite() && c.is_finite() && c.abs() > threshold && n.abs() > 1e-6)
        .map(|(n, c)| n / c)
        .collect();
    match median(&ratios) {
        Some(r) if ratios.len() >= 5 && r > 0.0 => {
            // Snap to a decade so 9.6 reads as ×10.
            let decade = 10f64.powf(r.log10().round());
            1.0 / decade
        }
        _ => 1.0,
    }
}

/// Resolve the rate signal and its threshold / axis from the mapping.
fn rate_signal(
    gen_: &AccelEnrichGenerator,
    log: &Log,
    mapping: &ChannelMapping,
) -> Result<(Vec<f64>, f64, RateSource, f64), AnalysisError> {
    let times = &log.times;
    let tps = mapped_column(log, mapping, ChannelRole::Tps)?
        .map(|v| mask_invalid(&v, Some((-5.0, 105.0))));
    let computed_tps_rate = tps
        .as_ref()
        .map(|t| time_derivative(&median_filter(t, 3), times));
    if let Some(native) = mapped_column(log, mapping, ChannelRole::TpsRate)? {
        let native = mask_invalid(&native, None);
        let scale = if gen_.tps_rate_scale > 0.0 {
            gen_.tps_rate_scale
        } else if let Some(c) = &computed_tps_rate {
            detect_rate_scale(&native, c, gen_.tps_rate_threshold)
        } else {
            1.0
        };
        let rate: Vec<f64> = native.iter().map(|v| v * scale).collect();
        return Ok((
            rate,
            gen_.tps_rate_threshold,
            RateSource::NativeTpsRate,
            scale,
        ));
    }
    if let Some(rate) = computed_tps_rate {
        return Ok((
            rate,
            gen_.tps_rate_threshold,
            RateSource::ComputedTpsRate,
            1.0,
        ));
    }
    if let Some(map) = mapped_column(log, mapping, ChannelRole::Map)? {
        let map = mask_invalid(&map, Some((-110.0, 600.0)));
        let rate = time_derivative(&median_filter(&map, 3), times);
        return Ok((
            rate,
            gen_.map_rate_threshold,
            RateSource::ComputedMapRate,
            1.0,
        ));
    }
    Err(AnalysisError::MissingChannel(
        "Throttle position, throttle rate or manifold pressure".to_string(),
    ))
}

impl TableAnalyzer for AccelEnrichGenerator {
    fn id(&self) -> &'static str {
        ID
    }

    fn name(&self) -> &'static str {
        "Acceleration Enrichment Table"
    }

    fn description(&self) -> &'static str {
        "Detects tip-in events, measures the lean/rich excursion against target after the lambda delay, \
         and suggests a starting enrichment correction binned by RPM and throttle rate."
    }

    fn roles(&self) -> Vec<RoleSpec> {
        vec![
            RoleSpec::required(ChannelRole::Rpm),
            RoleSpec::optional(ChannelRole::Tps),
            RoleSpec::optional(ChannelRole::TpsRate),
            RoleSpec::optional(ChannelRole::Map),
            RoleSpec::required(ChannelRole::Lambda),
            RoleSpec::optional(ChannelRole::LambdaTarget),
            RoleSpec::optional(ChannelRole::AeActive),
            RoleSpec::optional(ChannelRole::Clutch),
            RoleSpec::optional(ChannelRole::CoolantTemp),
        ]
    }

    fn measures(&self) -> Vec<MeasureSpec> {
        vec![
            MeasureSpec {
                key: "correction_pct",
                label: "Suggested correction",
                unit: "%",
                decimals: 1,
            },
            MeasureSpec {
                key: "depth_lambda",
                label: "Excursion depth",
                unit: "λ",
                decimals: 3,
            },
            MeasureSpec {
                key: "duration_ms",
                label: "Excursion duration",
                unit: "ms",
                decimals: 0,
            },
            MeasureSpec {
                key: "area_pct",
                label: "Area-based correction",
                unit: "%",
                decimals: 1,
            },
            MeasureSpec {
                key: "area_lambda_s",
                label: "Excursion area",
                unit: "λ·s",
                decimals: 4,
            },
            MeasureSpec {
                key: "delay_used_ms",
                label: "Delay compensation",
                unit: "ms",
                decimals: 0,
            },
        ]
    }

    fn default_axes(&self, log: &Log, mapping: &ChannelMapping) -> (AxisSpec, AxisSpec) {
        let rpm = mapped_column(log, mapping, ChannelRole::Rpm)
            .ok()
            .flatten()
            .map(|v| mask_invalid(&v, Some((0.0, 20_000.0))))
            .unwrap_or_default();
        let fallback_rpm: Vec<f64> = (1..=16).map(|i| i as f64 * 500.0).collect();
        let x = AxisSpec::from_data("RPM", "", &rpm, 500.0, &fallback_rpm);
        let source = match rate_signal(self, log, mapping) {
            Ok((_, _, s, _)) => s,
            Err(_) => RateSource::ComputedTpsRate,
        };
        let (label, unit, edges) = source.axis();
        (x, AxisSpec::new(label, unit, edges.to_vec()))
    }

    fn analyze(
        &self,
        log: &Log,
        log_name: &str,
        mapping: &ChannelMapping,
        axes: &(AxisSpec, AxisSpec),
        ctx: &GeneratorContext<'_>,
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
                "log rate is {:.1} Hz; accel enrichment needs at least 4 Hz",
                1.0 / dt
            )));
        }

        let rpm = mask_invalid(
            &required_column(log, mapping, ChannelRole::Rpm)?,
            Some((0.0, 20_000.0)),
        );
        let (rate, threshold, source, scale) = rate_signal(self, log, mapping)?;
        let lambda_raw = mask_invalid(&required_column(log, mapping, ChannelRole::Lambda)?, None);
        let unit = detect_fuel_mixture_unit(
            &lambda_raw
                .iter()
                .copied()
                .filter(|v| v.is_finite())
                .collect::<Vec<_>>(),
        );
        let to_lambda = |v: f64, u: FuelMixtureUnit| match u {
            FuelMixtureUnit::Lambda => v,
            FuelMixtureUnit::Afr => v / STOICH_AFR_GASOLINE,
        };
        let lambda: Vec<f64> = mask_invalid(
            &lambda_raw,
            Some(match unit {
                FuelMixtureUnit::Lambda => (0.4, 2.0),
                FuelMixtureUnit::Afr => (5.0, 30.0),
            }),
        )
        .iter()
        .map(|&v| to_lambda(v, unit))
        .collect();
        let target: Option<Vec<f64>> = match mapped_column(log, mapping, ChannelRole::LambdaTarget)?
        {
            Some(t) => {
                let t = mask_invalid(&t, None);
                let tu = detect_fuel_mixture_unit(
                    &t.iter()
                        .copied()
                        .filter(|v| v.is_finite())
                        .collect::<Vec<_>>(),
                );
                Some(
                    mask_invalid(
                        &t,
                        Some(match tu {
                            FuelMixtureUnit::Lambda => (0.4, 2.0),
                            FuelMixtureUnit::Afr => (5.0, 30.0),
                        }),
                    )
                    .iter()
                    .map(|&v| to_lambda(v, tu))
                    .collect(),
                )
            }
            None => None,
        };
        let ae_active =
            mapped_column(log, mapping, ChannelRole::AeActive)?.map(|v| mask_invalid(&v, None));
        // Speeduino / MegaSquirt log AE as a percentage that idles at 100.
        let ae_threshold = ae_active.as_ref().and_then(|a| median(a)).map_or(0.5, |m| {
            if (90.0..=110.0).contains(&m) {
                m + 0.5
            } else {
                0.5
            }
        });
        let kind = if ae_active.is_some() {
            CorrectionKind::Additional
        } else {
            CorrectionKind::Absolute
        };
        let clutch = mapped_column(log, mapping, ChannelRole::Clutch)?;
        let coolant =
            mapped_column(log, mapping, ChannelRole::CoolantTemp)?.map(|v| mask_invalid(&v, None));
        let load_for_delay = match mapped_column(log, mapping, ChannelRole::Map)? {
            Some(m) => Some(mask_invalid(&m, Some((-110.0, 600.0)))),
            None => mapped_column(log, mapping, ChannelRole::Tps)?
                .map(|t| mask_invalid(&t, Some((-5.0, 105.0)))),
        };

        let mut warnings = Vec::new();
        warnings.push(format!("Lambda channel detected as {}", unit.unit_name()));
        match source {
            RateSource::NativeTpsRate => warnings.push(format!(
                "Using native throttle rate channel (scale ×{scale})"
            )),
            RateSource::ComputedTpsRate => {
                warnings.push("Throttle rate computed from throttle position".to_string())
            }
            RateSource::ComputedMapRate => warnings.push(
                "No throttle channel mapped; using MAP rate as the tip-in trigger".to_string(),
            ),
        }
        if target.is_none() {
            warnings.push(
                "No lambda target mapped; excursions are measured against the pre-event baseline"
                    .to_string(),
            );
        }
        if ctx.delay_table.is_none() {
            warnings.push(format!(
                "No lambda delay table in session; using assumed delay of {:.0} ms",
                self.assumed_delay_ms
            ));
        }
        warnings.push(format!("Correction kind: {}", kind.label()));

        let instants = update_instants(&lambda);
        let max_event_s = self.max_event_ms / 1000.0;
        let min_run = 2usize;

        let (events, elapsed) = timed_analyze(|| {
            let runs = merge_runs(
                times,
                &find_rate_runs(&rate, threshold, min_run),
                self.merge_gap_ms / 1000.0,
            );
            let mut events = Vec::with_capacity(runs.len());
            for run in runs {
                let t_start = times[run.start];
                let rpm_at = window_median(times, &rpm, t_start - BASELINE_S, t_start + dt)
                    .unwrap_or(f64::NAN);
                let mut values = vec![f64::NAN; 6];
                let mut note = String::new();
                let make = |reject: Option<RejectReason>,
                            values: Vec<f64>,
                            quality: f32,
                            note: String| TableEvent {
                    log_id: 0,
                    log_name: log_name.to_string(),
                    time: t_start,
                    rpm: rpm_at,
                    axis_value: run.peak,
                    values,
                    quality,
                    reject,
                    note,
                };

                // Delay compensation.
                let mut delay_s = self.assumed_delay_ms / 1000.0;
                let mut delay_from_table = false;
                if let (Some(grid), Some(load)) = (ctx.delay_table, &load_for_delay)
                    && let Some(load_at) =
                        window_median(times, load, t_start - BASELINE_S, t_start + dt)
                    && let (Some(c), Some(r)) = (
                        grid.x_axis.bin_index(rpm_at),
                        grid.y_axis.bin_index(load_at),
                    )
                    && let Some(cell) = grid.cell(r, c)
                    && matches!(cell.confidence, Confidence::Medium | Confidence::High)
                {
                    delay_s = cell.median / 1000.0;
                    delay_from_table = true;
                }
                values[5] = delay_s * 1000.0;
                note.push_str(if delay_from_table {
                    "delay from table"
                } else {
                    "assumed delay"
                });
                let w0 = t_start + delay_s;
                let w1 = w0 + max_event_s;
                let window = index_range(times, w0, w1);

                if !rpm_at.is_finite() || window.is_empty() {
                    events.push(make(Some(RejectReason::InvalidSamples), values, 0.0, note));
                    continue;
                }
                if let Some(cl) = &clutch
                    && any_in_window(times, cl, t_start - BASELINE_S, w1, |v| v > 0.5)
                {
                    events.push(make(Some(RejectReason::Clutch), values, 0.0, note));
                    continue;
                }
                if let Some(ct) = &coolant
                    && window_median(times, ct, t_start - BASELINE_S, w1)
                        .is_some_and(|c| c < self.min_coolant_temp)
                {
                    events.push(make(Some(RejectReason::ColdEngine), values, 0.0, note));
                    continue;
                }
                // RPM rising after a tip-in is the engine responding; an RPM
                // *drop* of more than the limit mid-window is an upshift.
                let rpm_window = index_range(times, t_start, w1);
                let rpm_min = rpm[rpm_window.start.min(rpm.len())..rpm_window.end.min(rpm.len())]
                    .iter()
                    .copied()
                    .filter(|v| v.is_finite())
                    .fold(f64::INFINITY, f64::min);
                if !rpm_min.is_finite()
                    || rpm_min < rpm_at * (1.0 - self.max_rpm_change_pct / 100.0)
                {
                    events.push(make(Some(RejectReason::GearShift), values, 0.0, note));
                    continue;
                }
                if invalid_fraction(&lambda[window.clone()]) > MAX_INVALID_FRACTION {
                    events.push(make(Some(RejectReason::InvalidSamples), values, 0.0, note));
                    continue;
                }
                if let Some(ae) = &ae_active {
                    let active = any_in_window(times, ae, t_start, w1, |v| v > ae_threshold);
                    if !active {
                        events.push(make(Some(RejectReason::AeKindMismatch), values, 0.0, note));
                        continue;
                    }
                    note.push_str(", ECU AE active");
                }
                if axes.0.bin_index(rpm_at).is_none() || axes.1.bin_index(run.peak).is_none() {
                    events.push(make(Some(RejectReason::OutOfAxis), values, 0.0, note));
                    continue;
                }

                // Reference and deviation.
                let baseline = window_median(times, &lambda, t_start - BASELINE_S, t_start);
                let reference = |i: usize| -> f64 {
                    match &target {
                        Some(t) if t[i].is_finite() => t[i],
                        _ => baseline.unwrap_or(f64::NAN),
                    }
                };
                let dev = |i: usize| -> f64 {
                    let r = reference(i);
                    if lambda[i].is_finite() && r.is_finite() && r > 0.0 {
                        lambda[i] / r - 1.0
                    } else {
                        f64::NAN
                    }
                };
                let post: Vec<usize> = instants
                    .iter()
                    .copied()
                    .filter(|i| window.contains(i))
                    .collect();
                let Some(&peak_i) =
                    post.iter()
                        .filter(|&&i| dev(i).is_finite())
                        .max_by(|&&a, &&b| {
                            dev(a)
                                .abs()
                                .partial_cmp(&dev(b).abs())
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                else {
                    events.push(make(Some(RejectReason::InvalidSamples), values, 0.0, note));
                    continue;
                };
                let peak_dev = dev(peak_i);
                let sign = if peak_dev >= 0.0 { 1.0 } else { -1.0 };
                let depth = lambda[peak_i] - reference(peak_i);

                // Duration above the lean band in the peak's direction.
                let mut duration = 0.0;
                for i in window.clone() {
                    let d = dev(i);
                    if d.is_finite() && d * sign > self.lean_band && i + 1 < times.len() {
                        duration += times[i + 1] - times[i];
                    }
                }

                // Area from the first sample outside the recovery band until
                // the deviation stays inside it for RECOVERY_HOLD_S.
                let first = window
                    .clone()
                    .find(|&i| dev(i).is_finite() && dev(i).abs() > self.recovery_band);
                let area = match first {
                    Some(start) => {
                        let mut end = window.end - 1;
                        let mut inside_since: Option<f64> = None;
                        for (i, &t) in times.iter().enumerate().take(window.end).skip(peak_i) {
                            let d = dev(i);
                            if d.is_finite() && d.abs() <= self.recovery_band {
                                let since = *inside_since.get_or_insert(t);
                                if t - since >= RECOVERY_HOLD_S {
                                    end = i;
                                    break;
                                }
                            } else {
                                inside_since = None;
                            }
                        }
                        integrate(times, start, end, dev)
                    }
                    None => 0.0,
                };

                let clamp = self.correction_clamp_pct;
                let correction = (peak_dev * 100.0).clamp(-clamp, clamp);
                let area_pct = (area / (self.area_over_ms / 1000.0) * 100.0).clamp(-clamp, clamp);
                values[0] = correction;
                values[1] = depth;
                values[2] = duration * 1000.0;
                values[3] = area_pct;
                values[4] = area;
                let quality = if delay_from_table { 1.0 } else { 0.7 };
                note.push_str(&format!(", peak rate {:.0}, {}", run.peak, kind.label()));
                events.push(make(None, values, quality, note));
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
                key: "tps_rate_threshold",
                label: "TPS rate trigger (%/s)",
                tooltip: "Throttle rate that starts a tip-in event.",
                kind: TableParamKind::Float {
                    min: 5.0,
                    max: 2000.0,
                    speed: 1.0,
                },
            },
            TableParam {
                key: "map_rate_threshold",
                label: "MAP rate trigger (kPa/s)",
                tooltip: "Fallback trigger when no throttle channel is mapped.",
                kind: TableParamKind::Float {
                    min: 20.0,
                    max: 10000.0,
                    speed: 10.0,
                },
            },
            TableParam {
                key: "assumed_delay_ms",
                label: "Assumed lambda delay (ms)",
                tooltip: "Used when no lambda-delay table covers the cell.",
                kind: TableParamKind::Float {
                    min: 0.0,
                    max: 2000.0,
                    speed: 5.0,
                },
            },
            TableParam {
                key: "lean_band",
                label: "Lean band (λ)",
                tooltip: "Relative deviation counted as an excursion for the duration measure.",
                kind: TableParamKind::Float {
                    min: 0.001,
                    max: 0.5,
                    speed: 0.001,
                },
            },
            TableParam {
                key: "recovery_band",
                label: "Recovery band (λ)",
                tooltip: "Deviation inside which the event is considered recovered.",
                kind: TableParamKind::Float {
                    min: 0.001,
                    max: 0.5,
                    speed: 0.001,
                },
            },
            TableParam {
                key: "max_event_ms",
                label: "Window (ms)",
                tooltip: "AFR window length after the delay-shifted tip-in.",
                kind: TableParamKind::Float {
                    min: 200.0,
                    max: 5000.0,
                    speed: 10.0,
                },
            },
            TableParam {
                key: "max_rpm_change_pct",
                label: "Gear-shift RPM change (%)",
                tooltip: "RPM drop over the window that rejects the event as a gear shift.",
                kind: TableParamKind::Float {
                    min: 5.0,
                    max: 100.0,
                    speed: 1.0,
                },
            },
            TableParam {
                key: "correction_clamp_pct",
                label: "Correction clamp (%)",
                tooltip: "Suggestions are clamped to ± this value.",
                kind: TableParamKind::Float {
                    min: 5.0,
                    max: 200.0,
                    speed: 1.0,
                },
            },
            TableParam {
                key: "area_over_ms",
                label: "Area spread over (ms)",
                tooltip: "Accel time the area-based suggestion is spread over (ECU accel time).",
                kind: TableParamKind::Float {
                    min: 50.0,
                    max: 2000.0,
                    speed: 10.0,
                },
            },
            TableParam {
                key: "tps_rate_scale",
                label: "Native rate scale (0 = auto)",
                tooltip: "Multiplier for a native throttle-rate channel; 0 detects it (Haltech logs ×10).",
                kind: TableParamKind::Float {
                    min: 0.0,
                    max: 100.0,
                    speed: 0.01,
                },
            },
            TableParam {
                key: "merge_gap_ms",
                label: "Merge gap (ms)",
                tooltip: "Tip-ins closer than this merge into one event.",
                kind: TableParamKind::Float {
                    min: 0.0,
                    max: 2000.0,
                    speed: 10.0,
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
        p.insert(
            "tps_rate_threshold".into(),
            self.tps_rate_threshold.to_string(),
        );
        p.insert(
            "map_rate_threshold".into(),
            self.map_rate_threshold.to_string(),
        );
        p.insert("assumed_delay_ms".into(), self.assumed_delay_ms.to_string());
        p.insert("lean_band".into(), self.lean_band.to_string());
        p.insert("recovery_band".into(), self.recovery_band.to_string());
        p.insert("max_event_ms".into(), self.max_event_ms.to_string());
        p.insert(
            "max_rpm_change_pct".into(),
            self.max_rpm_change_pct.to_string(),
        );
        p.insert(
            "correction_clamp_pct".into(),
            self.correction_clamp_pct.to_string(),
        );
        p.insert("area_over_ms".into(), self.area_over_ms.to_string());
        p.insert("tps_rate_scale".into(), self.tps_rate_scale.to_string());
        p.insert("merge_gap_ms".into(), self.merge_gap_ms.to_string());
        p.insert("min_coolant_temp".into(), self.min_coolant_temp.to_string());
        AnalyzerConfig {
            id: ID.to_string(),
            name: self.name().to_string(),
            parameters: p,
        }
    }

    fn set_config(&mut self, config: &AnalyzerConfig) {
        self.tps_rate_threshold = param_f64(config, "tps_rate_threshold", self.tps_rate_threshold);
        self.map_rate_threshold = param_f64(config, "map_rate_threshold", self.map_rate_threshold);
        self.assumed_delay_ms = param_f64(config, "assumed_delay_ms", self.assumed_delay_ms);
        self.lean_band = param_f64(config, "lean_band", self.lean_band);
        self.recovery_band = param_f64(config, "recovery_band", self.recovery_band);
        self.max_event_ms = param_f64(config, "max_event_ms", self.max_event_ms);
        self.max_rpm_change_pct = param_f64(config, "max_rpm_change_pct", self.max_rpm_change_pct);
        self.correction_clamp_pct =
            param_f64(config, "correction_clamp_pct", self.correction_clamp_pct);
        self.area_over_ms = param_f64(config, "area_over_ms", self.area_over_ms);
        self.tps_rate_scale = param_f64(config, "tps_rate_scale", self.tps_rate_scale);
        self.merge_gap_ms = param_f64(config, "merge_gap_ms", self.merge_gap_ms);
        self.min_coolant_temp = param_f64(config, "min_coolant_temp", self.min_coolant_temp);
    }

    fn clone_box(&self) -> Box<dyn TableAnalyzer> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::super::binning::{ConfidenceRule, TableGrid};
    use super::super::synthetic::{SyntheticLog, Xorshift};
    use super::*;

    fn mapping() -> ChannelMapping {
        let mut m = ChannelMapping::default();
        m.set(ChannelRole::Rpm, Some("RPM".into()));
        m.set(ChannelRole::Tps, Some("TPS".into()));
        m.set(ChannelRole::Map, Some("MAP".into()));
        m.set(ChannelRole::Lambda, Some("Lambda".into()));
        m
    }

    fn axes() -> (AxisSpec, AxisSpec) {
        (
            AxisSpec::new("RPM", "", vec![1000.0, 2000.0, 3000.0, 4000.0]),
            AxisSpec::new("TPS rate", "%/s", TPS_RATE_EDGES.to_vec()),
        )
    }

    /// Tip-ins every 4 s: TPS ramps 5 -> 5 + amplitude over `ramp_s`, and
    /// lambda shows a first-order excursion of `depth` (signed, relative)
    /// starting `delay_s` after the ramp start and recovering with `tau_s`.
    #[allow(clippy::too_many_arguments)]
    fn tipin_log(
        rate_hz: f64,
        ramp_s: f64,
        amplitude: f64,
        delay_s: f64,
        depth: f64,
        tau_s: f64,
        sigma: f64,
        n_events: usize,
    ) -> SyntheticLog {
        let duration = 4.0 * n_events as f64 + 2.0;
        let mut log = SyntheticLog::new(rate_hz, duration);
        let n = log.times.len();
        let mut rng = Xorshift::new(9);
        let mut tps = vec![5.0; n];
        let mut lambda_true = vec![1.0; n];
        for e in 0..n_events {
            let t0 = 2.0 + 4.0 * e as f64;
            for i in 0..n {
                let t = log.times[i];
                if t >= t0 && t < t0 + ramp_s {
                    tps[i] = 5.0 + amplitude * (t - t0) / ramp_s;
                } else if t >= t0 + ramp_s && t < t0 + 2.5 {
                    tps[i] = 5.0 + amplitude;
                }
                if t >= t0 + delay_s {
                    let x = t - t0 - delay_s;
                    // Rise over 0.1 s, then decay.
                    let shape = if x < 0.1 {
                        x / 0.1
                    } else {
                        (-(x - 0.1) / tau_s).exp()
                    };
                    if shape > 1e-3 {
                        lambda_true[i] = 1.0 + depth * shape;
                    }
                }
            }
        }
        let lambda = log.sample_and_hold(&lambda_true, rate_hz, sigma, &mut rng);
        log.add("RPM", vec![2500.0; n]);
        log.add("TPS", tps);
        log.add("MAP", vec![50.0; n]);
        log.add("Lambda", lambda);
        log
    }

    fn run(
        log: &SyntheticLog,
        gen_: &AccelEnrichGenerator,
        ctx: &GeneratorContext<'_>,
    ) -> (Vec<TableEvent>, RunReport) {
        gen_.analyze(&log.log, "synthetic", &mapping(), &axes(), ctx)
            .expect("analysis runs")
    }

    #[test]
    fn recovers_depth_and_sign_and_bins_by_peak_rate() {
        let gen_ = AccelEnrichGenerator {
            assumed_delay_ms: 100.0,
            ..Default::default()
        };
        for &(depth, rate) in &[(0.08, 50.0), (-0.06, 100.0), (0.12, 20.0)] {
            // 40 % over 0.2 s = 200 %/s peak.
            let log = tipin_log(rate, 0.2, 40.0, 0.1, depth, 0.3, 0.002, 4);
            let (events, report) = run(&log, &gen_, &GeneratorContext::default());
            assert_eq!(report.accepted, 4, "{}", report.summary());
            for e in events.iter().filter(|e| e.accepted()) {
                assert!(
                    (e.value(1) - depth).abs() < 0.01,
                    "depth {} vs {}",
                    e.value(1),
                    depth
                );
                assert!(
                    (e.value(0) - depth * 100.0).abs() < 1.0,
                    "correction {}",
                    e.value(0)
                );
                assert!(e.value(2) > 100.0, "duration {}", e.value(2));
                assert_eq!(e.value(0).signum(), depth.signum());
                assert_eq!(e.value(5), 100.0);
                assert!(
                    (e.axis_value - 200.0).abs() < 60.0,
                    "peak rate {}",
                    e.axis_value
                );
                assert_eq!(axes().1.bin_index(e.axis_value), Some(3));
                assert_eq!(e.value(4).signum(), depth.signum());
            }
        }
    }

    #[test]
    fn afr_input_matches_lambda_input() {
        let gen_ = AccelEnrichGenerator::default();
        let log = tipin_log(50.0, 0.2, 40.0, 0.12, 0.08, 0.3, 0.0, 3);
        let (a, _) = run(&log, &gen_, &GeneratorContext::default());
        let mut afr = log.clone();
        afr.scale("Lambda", 14.7);
        let (b, _) = run(&afr, &gen_, &GeneratorContext::default());
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.reject, y.reject);
            assert!((x.value(0) - y.value(0)).abs() < 0.2);
        }
    }

    #[test]
    fn overlapping_ramps_merge_into_one_event() {
        let mut log = tipin_log(50.0, 0.2, 40.0, 0.1, 0.08, 0.3, 0.0, 2);
        // Add a second stab 0.25 s after the first ramp starts.
        let mut tps = log.column("TPS");
        for (i, v) in tps.iter_mut().enumerate() {
            let t = log.times[i];
            if (2.25..2.35).contains(&t) {
                *v += 30.0 * (t - 2.25) / 0.1;
            } else if (2.35..2.5).contains(&t) {
                *v += 30.0;
            }
        }
        log.replace("TPS", tps);
        let (events, _) = run(
            &log,
            &AccelEnrichGenerator::default(),
            &GeneratorContext::default(),
        );
        let near_first: Vec<&TableEvent> = events
            .iter()
            .filter(|e| (e.time - 2.0).abs() < 0.5)
            .collect();
        assert_eq!(
            near_first.len(),
            1,
            "{:?}",
            events.iter().map(|e| e.time).collect::<Vec<_>>()
        );
        // The larger (300 %/s) peak wins.
        assert!(near_first[0].axis_value > 250.0);
    }

    #[test]
    fn rpm_collapse_is_a_gear_shift() {
        let mut log = tipin_log(50.0, 0.2, 40.0, 0.1, 0.08, 0.3, 0.0, 3);
        let rpm: Vec<f64> = log
            .times
            .iter()
            .map(|&t| {
                if (6.2..7.0).contains(&t) {
                    1500.0
                } else {
                    2500.0
                }
            })
            .collect();
        log.replace("RPM", rpm);
        let (events, _) = run(
            &log,
            &AccelEnrichGenerator::default(),
            &GeneratorContext::default(),
        );
        assert!(events[0].accepted());
        assert_eq!(events[1].reject, Some(RejectReason::GearShift));
    }

    #[test]
    fn delay_compensation_prefers_a_confident_table_cell() {
        let log = tipin_log(50.0, 0.2, 40.0, 0.25, 0.08, 0.3, 0.0, 3);
        let gen_ = AccelEnrichGenerator {
            assumed_delay_ms: 100.0,
            ..Default::default()
        };
        // A delay grid with a High cell at (2500 rpm, 50 kPa) = 250 ms and a
        // Low cell elsewhere.
        let x = AxisSpec::new("RPM", "", vec![1000.0, 2000.0, 3000.0, 4000.0]);
        let y = AxisSpec::new("MAP", "kPa", vec![0.0, 100.0]);
        let mut points = Vec::new();
        for i in 0..10 {
            points.push((2500.0, 50.0, 250.0 + i as f64));
        }
        points.push((3500.0, 50.0, 900.0));
        let grid = TableGrid::build(x, y, points, ConfidenceRule::default());
        let ctx = GeneratorContext {
            delay_table: Some(&grid),
        };
        let (events, _) = run(&log, &gen_, &ctx);
        for e in events.iter().filter(|e| e.accepted()) {
            assert!(
                (e.value(5) - 254.5).abs() < 1.0,
                "delay used {}",
                e.value(5)
            );
            assert!(e.note.contains("delay from table"));
            assert!((e.value(1) - 0.08).abs() < 0.01);
        }
        // Move the engine to the Low cell: assumed delay applies.
        let mut log2 = log.clone();
        let n = log2.times.len();
        log2.replace("RPM", vec![3500.0; n]);
        let (events, _) = run(&log2, &gen_, &ctx);
        for e in events.iter().filter(|e| e.accepted()) {
            assert_eq!(e.value(5), 100.0);
            assert!(e.note.contains("assumed delay"));
        }
    }

    #[test]
    fn native_rate_channel_scale_is_detected() {
        let log = tipin_log(50.0, 0.2, 40.0, 0.1, 0.08, 0.3, 0.0, 3);
        let computed = time_derivative(&median_filter(&log.column("TPS"), 3), &log.times);
        let native: Vec<f64> = computed.iter().map(|v| v * 10.0).collect();
        assert!((detect_rate_scale(&native, &computed, 50.0) - 0.1).abs() < 1e-9);
        assert_eq!(detect_rate_scale(&computed, &computed, 50.0), 1.0);
        assert_eq!(detect_rate_scale(&[0.0; 10], &[0.0; 10], 50.0), 1.0);

        let mut log = log;
        log.add("TPS DOT", native);
        let mut m = mapping();
        m.set(ChannelRole::TpsRate, Some("TPS DOT".into()));
        let gen_ = AccelEnrichGenerator::default();
        let (events, report) = gen_
            .analyze(&log.log, "s", &m, &axes(), &GeneratorContext::default())
            .unwrap();
        assert_eq!(report.accepted, 3, "{}", report.summary());
        assert!(
            report.warnings.iter().any(|w| w.contains("×0.1")),
            "{:?}",
            report.warnings
        );
        for e in &events {
            assert!((e.axis_value - 200.0).abs() < 60.0);
        }
    }

    #[test]
    fn map_rate_fallback_relabels_axis() {
        let mut log = tipin_log(50.0, 0.2, 40.0, 0.1, 0.08, 0.3, 0.0, 2);
        // MAP follows TPS: 50 -> 90 kPa over the ramp (200 kPa/s), below the
        // default 400 kPa/s trigger, so lower the threshold.
        let map: Vec<f64> = log.column("TPS").iter().map(|t| 45.0 + t).collect();
        log.replace("MAP", map);
        let mut m = mapping();
        m.set(ChannelRole::Tps, None);
        let gen_ = AccelEnrichGenerator {
            map_rate_threshold: 100.0,
            ..Default::default()
        };
        let (_, y) = gen_.default_axes(&log.log, &m);
        assert_eq!(y.unit, "kPa/s");
        let axes = (axes().0, y);
        let (_, report) = gen_
            .analyze(&log.log, "s", &m, &axes, &GeneratorContext::default())
            .unwrap();
        assert_eq!(report.accepted, 2, "{}", report.summary());
        assert!(report.warnings.iter().any(|w| w.contains("MAP rate")));
    }

    #[test]
    fn ae_activity_role_switches_kind_and_rejects_mismatch() {
        let log = tipin_log(50.0, 0.2, 40.0, 0.1, 0.08, 0.3, 0.0, 3);
        let n = log.times.len();
        // Speeduino-style percentage that idles at 100 and is active only
        // around the second event.
        let ae: Vec<f64> = log
            .times
            .iter()
            .map(|&t| {
                if (6.0..6.5).contains(&t) {
                    130.0
                } else {
                    100.0
                }
            })
            .collect();
        let mut log = log;
        log.add("Accel Enrich", ae);
        assert_eq!(n, log.times.len());
        let mut m = mapping();
        m.set(ChannelRole::AeActive, Some("Accel Enrich".into()));
        let (events, report) = AccelEnrichGenerator::default()
            .analyze(&log.log, "s", &m, &axes(), &GeneratorContext::default())
            .unwrap();
        assert!(report.warnings.iter().any(|w| w.contains("additional")));
        assert_eq!(events[0].reject, Some(RejectReason::AeKindMismatch));
        assert!(events[1].accepted());
        assert!(events[1].note.contains("ECU AE active"));
        assert_eq!(events[2].reject, Some(RejectReason::AeKindMismatch));
    }

    #[test]
    fn target_channel_is_used_as_reference() {
        let log = tipin_log(50.0, 0.2, 40.0, 0.1, 0.08, 0.3, 0.0, 2);
        let n = log.times.len();
        let mut log = log;
        // Target sits 5 % rich of the baseline: excursion vs target is deeper.
        log.add("Target", vec![0.95; n]);
        let mut m = mapping();
        m.set(ChannelRole::LambdaTarget, Some("Target".into()));
        let (events, _) = AccelEnrichGenerator::default()
            .analyze(&log.log, "s", &m, &axes(), &GeneratorContext::default())
            .unwrap();
        for e in events.iter().filter(|e| e.accepted()) {
            assert!((e.value(1) - 0.13).abs() < 0.01, "depth {}", e.value(1));
        }
    }

    #[test]
    fn clutch_cold_and_out_of_axis_gates() {
        let base = tipin_log(50.0, 0.2, 40.0, 0.1, 0.08, 0.3, 0.0, 2);
        let n = base.times.len();
        let mut log = base.clone();
        log.add(
            "Clutch",
            base.times
                .iter()
                .map(|&t| if t > 5.0 { 1.0 } else { 0.0 })
                .collect(),
        );
        let mut m = mapping();
        m.set(ChannelRole::Clutch, Some("Clutch".into()));
        let (events, _) = AccelEnrichGenerator::default()
            .analyze(&log.log, "s", &m, &axes(), &GeneratorContext::default())
            .unwrap();
        assert!(events[0].accepted());
        assert_eq!(events[1].reject, Some(RejectReason::Clutch));

        let mut log = base.clone();
        log.add("CLT", vec![30.0; n]);
        let mut m = mapping();
        m.set(ChannelRole::CoolantTemp, Some("CLT".into()));
        let (_, report) = AccelEnrichGenerator::default()
            .analyze(&log.log, "s", &m, &axes(), &GeneratorContext::default())
            .unwrap();
        assert_eq!(
            report.rejected.get(&RejectReason::ColdEngine).copied(),
            Some(2)
        );

        let narrow = (AxisSpec::new("RPM", "", vec![5000.0, 6000.0]), axes().1);
        let (_, report) = AccelEnrichGenerator::default()
            .analyze(
                &base.log,
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
    }

    #[test]
    fn missing_trigger_channel_is_an_error() {
        let log = tipin_log(50.0, 0.2, 40.0, 0.1, 0.08, 0.3, 0.0, 1);
        let mut m = mapping();
        m.set(ChannelRole::Tps, None);
        m.set(ChannelRole::Map, None);
        let err = AccelEnrichGenerator::default()
            .analyze(&log.log, "s", &m, &axes(), &GeneratorContext::default())
            .unwrap_err();
        assert!(matches!(err, AnalysisError::MissingChannel(_)));
    }

    #[test]
    fn config_round_trip() {
        let mut gen_ = AccelEnrichGenerator::default();
        let cfg = gen_.get_config();
        for p in gen_.params() {
            assert!(
                cfg.parameters.contains_key(p.key),
                "param {} missing",
                p.key
            );
        }
        let mut cfg = cfg;
        cfg.parameters
            .insert("assumed_delay_ms".into(), "200".into());
        gen_.set_config(&cfg);
        assert_eq!(gen_.assumed_delay_ms, 200.0);
    }
}
