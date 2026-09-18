//! Table generators: mine events out of one or more logs into a 2-D tuning
//! table (RPM × load, RPM × throttle rate, ...).
//!
//! The existing [`Analyzer`](super::Analyzer) trait returns one value per log
//! timestamp, which a table does not fit, so generators implement the sibling
//! [`TableAnalyzer`] trait. Every generator produces a list of
//! [`TableEvent`]s, accepted or rejected with a [`RejectReason`], plus a
//! [`RunReport`]; a [`TableAccumulator`] folds the events of several logs
//! into one grid and re-bins on demand for whichever measure the user wants
//! to look at.
//!
//! Cell values are medians with MAD spread and a [`Confidence`] tier. Empty
//! and low-confidence cells are never interpolated: a fabricated number in a
//! tuning table is worse than a gap.

pub mod accel_enrich;
pub mod binning;
pub mod channel_map;
pub mod events;
pub mod export;
pub mod lambda_delay;
pub mod stats;
pub mod synthetic;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use binning::{AxisSpec, CellStats, Confidence, ConfidenceRule, TableGrid};
pub use channel_map::{
    ChannelMapping, ChannelRole, LoadKind, RoleSpec, Suggestion, suggest_mapping,
};

use super::{AnalysisError, AnalyzerConfig};
use crate::parsers::types::Log;

/// Why a detected event was not used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RejectReason {
    /// RPM or load moved too much across the measurement window.
    Unsteady,
    /// Another event started too close for the responses to be attributed.
    Overlap,
    /// The sensor never moved past the response threshold in time.
    NoResponse,
    /// The sensor moved past the threshold the wrong way first.
    WrongDirection,
    /// Fuel cut was active around the event.
    FuelCut,
    /// Pulse width below the floor (decel / fuel-cut region).
    LowPw,
    /// The closed-loop correction moved during the event.
    ClosedLoopActive,
    /// Coolant below the minimum temperature.
    ColdEngine,
    /// Clutch was in.
    Clutch,
    /// Too many masked / sentinel samples, or a flat sensor.
    InvalidSamples,
    /// The event's RPM / load fell outside the table axes.
    OutOfAxis,
    /// RPM collapsed or jumped mid-window (gear change).
    GearShift,
    /// The ECU's accel-enrichment activity did not match the table kind.
    AeKindMismatch,
}

impl RejectReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::Unsteady => "unsteady",
            Self::Overlap => "overlap",
            Self::NoResponse => "no response",
            Self::WrongDirection => "wrong direction",
            Self::FuelCut => "fuel cut",
            Self::LowPw => "low pulse width",
            Self::ClosedLoopActive => "closed loop active",
            Self::ColdEngine => "cold engine",
            Self::Clutch => "clutch",
            Self::InvalidSamples => "invalid samples",
            Self::OutOfAxis => "out of axis",
            Self::GearShift => "gear shift",
            Self::AeKindMismatch => "AE kind mismatch",
        }
    }
}

/// A measure a generator records per event; index-aligned with
/// [`TableEvent::values`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeasureSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub decimals: usize,
}

/// One detected event, accepted or rejected.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TableEvent {
    /// Per-load nonce of the log the event came from (never a file index,
    /// which shifts when tabs close, and never a bare file name).
    pub log_id: u64,
    pub log_name: String,
    /// Event start in log time (seconds).
    pub time: f64,
    /// RPM at the event (X axis).
    pub rpm: f64,
    /// Y-axis value: load for lambda delay, peak throttle rate for accel
    /// enrichment.
    pub axis_value: f64,
    /// Measured values, aligned with the generator's [`MeasureSpec`] list.
    /// `NaN` where a measure could not be taken.
    pub values: Vec<f64>,
    /// 0..1 per-event quality (informational; cells use plain medians).
    pub quality: f32,
    pub reject: Option<RejectReason>,
    /// Free-form diagnostics for the inspector (e.g. which delay was used).
    pub note: String,
}

impl TableEvent {
    pub fn accepted(&self) -> bool {
        self.reject.is_none()
    }

    pub fn value(&self, measure: usize) -> f64 {
        self.values.get(measure).copied().unwrap_or(f64::NAN)
    }
}

/// Outcome of running a generator over one log.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunReport {
    pub log_name: String,
    /// Events detected before gating.
    pub candidates: usize,
    pub accepted: usize,
    pub rejected: BTreeMap<RejectReason, usize>,
    pub warnings: Vec<String>,
    pub computation_time_ms: u64,
}

impl RunReport {
    pub fn total_rejected(&self) -> usize {
        self.rejected.values().sum()
    }

    /// `"41 events found · 39 rejected: 30 unsteady, 9 no response"`.
    pub fn summary(&self) -> String {
        let mut s = format!("{} events found", self.candidates);
        let rejected = self.total_rejected();
        if rejected > 0 {
            let mut parts: Vec<(usize, RejectReason)> =
                self.rejected.iter().map(|(r, n)| (*n, *r)).collect();
            parts.sort_by_key(|p| std::cmp::Reverse(p.0));
            let detail: Vec<String> = parts
                .iter()
                .map(|(n, r)| format!("{} {}", n, r.label()))
                .collect();
            s.push_str(&format!(" · {} rejected: {}", rejected, detail.join(", ")));
        } else if self.candidates > 0 {
            s.push_str(" · none rejected");
        }
        s
    }

    pub fn from_events(log_name: &str, events: &[TableEvent]) -> Self {
        let mut report = Self {
            log_name: log_name.to_string(),
            candidates: events.len(),
            ..Default::default()
        };
        for e in events {
            match e.reject {
                None => report.accepted += 1,
                Some(r) => *report.rejected.entry(r).or_insert(0) += 1,
            }
        }
        report
    }
}

/// A user-tunable parameter, for the UI's parameter grid.
#[derive(Clone, Debug, PartialEq)]
pub struct TableParam {
    pub key: &'static str,
    pub label: &'static str,
    pub tooltip: &'static str,
    pub kind: TableParamKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TableParamKind {
    Float { min: f64, max: f64, speed: f64 },
    Integer { min: i64, max: i64 },
    Choice(&'static [&'static str]),
}

/// Extra inputs a generator may use, beyond the log itself.
#[derive(Clone, Copy, Debug, Default)]
pub struct GeneratorContext<'a> {
    /// A lambda-delay grid (RPM × load, ms) from the same session. The accel
    /// enrichment generator shifts its AFR window by the matching cell.
    pub delay_table: Option<&'a TableGrid>,
}

/// A generator that mines events from a log into table events.
pub trait TableAnalyzer: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;

    /// Channel roles this generator uses (required and optional).
    fn roles(&self) -> Vec<RoleSpec>;

    /// Measures recorded per event. Index 0 is the primary table value.
    fn measures(&self) -> Vec<MeasureSpec>;

    /// Data-driven default axes: `(x = RPM, y)`.
    fn default_axes(&self, log: &Log, mapping: &ChannelMapping) -> (AxisSpec, AxisSpec);

    /// Detect and measure events in one log.
    fn analyze(
        &self,
        log: &Log,
        log_name: &str,
        mapping: &ChannelMapping,
        axes: &(AxisSpec, AxisSpec),
        ctx: &GeneratorContext<'_>,
    ) -> Result<(Vec<TableEvent>, RunReport), AnalysisError>;

    fn params(&self) -> Vec<TableParam>;
    fn get_config(&self) -> AnalyzerConfig;
    fn set_config(&mut self, config: &AnalyzerConfig);
    fn clone_box(&self) -> Box<dyn TableAnalyzer>;
}

impl Clone for Box<dyn TableAnalyzer> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// The generators that ship with UltraLog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum GeneratorKind {
    #[default]
    LambdaDelay,
    AccelEnrich,
}

impl GeneratorKind {
    pub const ALL: [GeneratorKind; 2] = [GeneratorKind::LambdaDelay, GeneratorKind::AccelEnrich];

    pub fn id(self) -> &'static str {
        match self {
            Self::LambdaDelay => lambda_delay::ID,
            Self::AccelEnrich => accel_enrich::ID,
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    pub fn create(self) -> Box<dyn TableAnalyzer> {
        match self {
            Self::LambdaDelay => Box::new(lambda_delay::LambdaDelayGenerator::default()),
            Self::AccelEnrich => Box::new(accel_enrich::AccelEnrichGenerator::default()),
        }
    }
}

/// One log folded into an accumulator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccumulatedLog {
    pub id: u64,
    pub name: String,
    pub report: RunReport,
}

/// Events from one or more logs, re-binnable into a grid per measure.
#[derive(Clone, Debug, PartialEq)]
pub struct TableAccumulator {
    pub generator: GeneratorKind,
    pub axes: (AxisSpec, AxisSpec),
    pub measures: Vec<MeasureSpec>,
    pub events: Vec<TableEvent>,
    pub logs: Vec<AccumulatedLog>,
    pub rule: ConfidenceRule,
}

impl TableAccumulator {
    pub fn new(
        generator: GeneratorKind,
        axes: (AxisSpec, AxisSpec),
        measures: Vec<MeasureSpec>,
    ) -> Self {
        Self {
            generator,
            axes,
            measures,
            events: Vec::new(),
            logs: Vec::new(),
            rule: ConfidenceRule::default(),
        }
    }

    /// Fold one log's events in. A log with the same id replaces its earlier
    /// contribution.
    pub fn add_log(&mut self, id: u64, name: &str, events: Vec<TableEvent>, report: RunReport) {
        self.remove_log(id);
        self.events.extend(events);
        self.logs.push(AccumulatedLog {
            id,
            name: name.to_string(),
            report,
        });
    }

    pub fn remove_log(&mut self, id: u64) {
        self.events.retain(|e| e.log_id != id);
        self.logs.retain(|l| l.id != id);
    }

    pub fn contains_log(&self, id: u64) -> bool {
        self.logs.iter().any(|l| l.id == id)
    }

    pub fn reset(&mut self) {
        self.events.clear();
        self.logs.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn accepted(&self) -> impl Iterator<Item = &TableEvent> {
        self.events.iter().filter(|e| e.accepted())
    }

    pub fn accepted_count(&self) -> usize {
        self.accepted().count()
    }

    /// Bin the accepted events for one measure.
    pub fn grid(&self, measure: usize) -> TableGrid {
        TableGrid::build(
            self.axes.0.clone(),
            self.axes.1.clone(),
            self.accepted()
                .map(|e| (e.rpm, e.axis_value, e.value(measure))),
            self.rule,
        )
    }

    /// Accepted events that fall in a given cell.
    pub fn events_in_cell(&self, row: usize, col: usize) -> Vec<&TableEvent> {
        self.accepted()
            .filter(|e| {
                self.axes.0.bin_index(e.rpm) == Some(col)
                    && self.axes.1.bin_index(e.axis_value) == Some(row)
            })
            .collect()
    }
}

/// Fetch a mapped channel's column, checking it is aligned with `times`.
///
/// `Log::get_channel_data` is a `filter_map` that drops rows missing the
/// column, so a ragged log yields a column *shorter* than `times`; any
/// index-based math on that pair would be misaligned, so it is refused here.
pub(crate) fn mapped_column(
    log: &Log,
    mapping: &ChannelMapping,
    role: ChannelRole,
) -> Result<Option<Vec<f64>>, AnalysisError> {
    let Some(name) = mapping.get(role) else {
        return Ok(None);
    };
    let idx = log
        .channels
        .iter()
        .position(|c| c.name() == name)
        .ok_or_else(|| AnalysisError::MissingChannel(name.to_string()))?;
    let data = log.get_channel_data(idx);
    if data.len() != log.times.len() {
        return Err(AnalysisError::ComputationError(format!(
            "channel '{}' has {} samples but the log has {} timestamps (ragged log)",
            name,
            data.len(),
            log.times.len()
        )));
    }
    Ok(Some(data))
}

pub(crate) fn required_column(
    log: &Log,
    mapping: &ChannelMapping,
    role: ChannelRole,
) -> Result<Vec<f64>, AnalysisError> {
    mapped_column(log, mapping, role)?
        .ok_or_else(|| AnalysisError::MissingChannel(role.label().to_string()))
}

/// Parse a float parameter, keeping the current value on a bad string.
pub(crate) fn param_f64(config: &AnalyzerConfig, key: &str, current: f64) -> f64 {
    config
        .parameters
        .get(key)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(log_id: u64, rpm: f64, load: f64, v: f64, reject: Option<RejectReason>) -> TableEvent {
        TableEvent {
            log_id,
            log_name: format!("log{log_id}"),
            time: 0.0,
            rpm,
            axis_value: load,
            values: vec![v],
            quality: 1.0,
            reject,
            note: String::new(),
        }
    }

    fn axes() -> (AxisSpec, AxisSpec) {
        (
            AxisSpec::new("RPM", "", vec![1000.0, 2000.0, 3000.0]),
            AxisSpec::new("MAP", "kPa", vec![0.0, 50.0, 100.0]),
        )
    }

    #[test]
    fn report_summary_orders_by_count() {
        let events = vec![
            event(1, 1500.0, 25.0, 100.0, None),
            event(1, 1500.0, 25.0, 100.0, Some(RejectReason::Unsteady)),
            event(1, 1500.0, 25.0, 100.0, Some(RejectReason::Unsteady)),
            event(1, 1500.0, 25.0, 100.0, Some(RejectReason::NoResponse)),
        ];
        let r = RunReport::from_events("a", &events);
        assert_eq!(r.candidates, 4);
        assert_eq!(r.accepted, 1);
        assert_eq!(
            r.summary(),
            "4 events found · 3 rejected: 2 unsteady, 1 no response"
        );
        let none = RunReport::from_events("a", &events[..1]);
        assert_eq!(none.summary(), "1 events found · none rejected");
    }

    #[test]
    fn accumulate_two_logs_then_remove_one_equals_single_log() {
        let measures = vec![MeasureSpec {
            key: "v",
            label: "v",
            unit: "",
            decimals: 0,
        }];
        let mut acc = TableAccumulator::new(GeneratorKind::LambdaDelay, axes(), measures.clone());
        let a = vec![
            event(1, 1500.0, 25.0, 100.0, None),
            event(1, 1500.0, 25.0, 120.0, None),
        ];
        let b = vec![
            event(2, 1500.0, 25.0, 500.0, None),
            event(2, 2500.0, 75.0, 50.0, Some(RejectReason::Overlap)),
        ];
        acc.add_log(1, "a", a.clone(), RunReport::from_events("a", &a));
        acc.add_log(2, "b", b.clone(), RunReport::from_events("b", &b));
        assert_eq!(acc.accepted_count(), 3);
        assert_eq!(acc.grid(0).cell(0, 0).unwrap().median, 120.0);
        assert_eq!(acc.events_in_cell(0, 0).len(), 3);

        let mut single = TableAccumulator::new(GeneratorKind::LambdaDelay, axes(), measures);
        single.add_log(1, "a", a.clone(), RunReport::from_events("a", &a));
        acc.remove_log(2);
        assert_eq!(acc, single);
        assert!(!acc.contains_log(2));

        // Re-adding the same id replaces rather than duplicates.
        acc.add_log(1, "a", a.clone(), RunReport::from_events("a", &a));
        assert_eq!(acc.accepted_count(), 2);
        acc.reset();
        assert!(acc.is_empty());
    }

    #[test]
    fn generator_kind_round_trip() {
        for k in GeneratorKind::ALL {
            assert_eq!(GeneratorKind::from_id(k.id()), Some(k));
            let g = k.create();
            assert_eq!(g.id(), k.id());
            assert!(!g.measures().is_empty());
            assert!(g.roles().iter().any(|r| r.required));
        }
        assert_eq!(GeneratorKind::from_id("nope"), None);
    }
}
