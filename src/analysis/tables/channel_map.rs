//! Channel roles and auto-suggested channel mapping for the table generators.
//!
//! A generator does not ask for channel *names*; it asks for channel *roles*
//! (engine speed, load, injector pulse width, ...). The user maps a log's
//! channels onto those roles once, and [`suggest_mapping`] pre-fills that
//! mapping by scoring every channel in three tiers:
//!
//! 1. the existing normalization system resolves the name to the role's
//!    canonical name (score 100),
//! 2. OpenECU Alliance spec metadata puts the channel in a category/unit
//!    consistent with the role (score 60),
//! 3. name heuristics such as `on time`, `pulse width`, `dfco` (score 40),
//!
//! and then vetoes candidates whose data is implausible for the role (an
//! "RPM" channel whose median is 3 is not engine speed). Names containing
//! `overall` / `avg` / `average` lose 10 points so a per-bank sensor wins a
//! tie: averaging sensors with different transport delays smears the very
//! rise time the lambda delay generator measures.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::stats::median;
use crate::adapters::ChannelCategory;
use crate::normalize::{get_spec_metadata, normalize_channel_name_with_custom};
use crate::parsers::types::Log;

/// Semantic role a mapped channel plays in a generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum ChannelRole {
    Rpm,
    /// Manifold absolute pressure (kPa). Load axis for lambda delay and the
    /// fallback tip-in trigger for accel enrichment.
    Map,
    /// Throttle position (%). Load axis alternative and the primary tip-in
    /// source for accel enrichment.
    Tps,
    /// Injector pulse width (ms / us) or duty cycle (%). Steps are relative,
    /// so the unit does not matter.
    PulseWidth,
    /// Wideband lambda or AFR; the unit is auto-detected from the data.
    Lambda,
    /// Commanded lambda / AFR target.
    LambdaTarget,
    /// Native throttle rate-of-change channel (Haltech `Throttle Position
    /// Derivative`, Speeduino `TPS DOT`). Preferred over a computed derivative
    /// because the ECU computes it at tick rate rather than log rate.
    TpsRate,
    /// Deceleration fuel cut flag.
    FuelCut,
    /// Closed-loop O2 correction / state. Movement here during an event means
    /// the ECU, not the tuner, changed the fuelling.
    ClosedLoopState,
    CoolantTemp,
    /// Clutch switch state.
    Clutch,
    /// ECU acceleration-enrichment activity (Haltech `Transient Throttle Load
    /// Derivative`, rusEFI `Fuel: TPS AE Active`, Speeduino `Accel Enrich`).
    AeActive,
}

impl ChannelRole {
    pub fn label(self) -> &'static str {
        match self {
            Self::Rpm => "Engine speed",
            Self::Map => "Manifold pressure",
            Self::Tps => "Throttle position",
            Self::PulseWidth => "Injector pulse width",
            Self::Lambda => "Lambda / AFR",
            Self::LambdaTarget => "Lambda / AFR target",
            Self::TpsRate => "Throttle rate",
            Self::FuelCut => "Fuel cut flag",
            Self::ClosedLoopState => "Closed-loop correction",
            Self::CoolantTemp => "Coolant temperature",
            Self::Clutch => "Clutch state",
            Self::AeActive => "ECU accel enrichment",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Self::Rpm => "Engine RPM.",
            Self::Map => "Manifold absolute pressure in kPa.",
            Self::Tps => "Throttle position in percent.",
            Self::PulseWidth => {
                "Injector on-time (ms or us) or duty cycle (%). Only relative steps are used."
            }
            Self::Lambda => {
                "Wideband lambda (~1.0) or AFR (~14.7). Prefer a single sensor over an averaged channel."
            }
            Self::LambdaTarget => {
                "Commanded lambda or AFR. Optional: excursions are measured against it instead of the pre-event baseline."
            }
            Self::TpsRate => {
                "Native throttle derivative in %/s if the ECU logs one. Optional: computed from throttle position otherwise."
            }
            Self::FuelCut => {
                "Deceleration fuel cut / DFCO flag. Optional: events during fuel cut are rejected."
            }
            Self::ClosedLoopState => {
                "Short-term trim, EGO correction or O2 control state. Optional: events where it moves are rejected."
            }
            Self::CoolantTemp => "Coolant temperature. Optional: cold-engine events are rejected.",
            Self::Clutch => "Clutch switch. Optional: events with the clutch in are rejected.",
            Self::AeActive => {
                "ECU accel-enrichment activity. Optional: suggestions become 'additional to' the ECU's current value."
            }
        }
    }

    /// Canonical normalized names that identify this role outright.
    fn canonical_names(self) -> &'static [&'static str] {
        match self {
            Self::Rpm => &["RPM"],
            Self::Map => &["MAP"],
            Self::Tps => &["TPS"],
            Self::PulseWidth => &["Pulse Width", "Duty Cycle"],
            Self::Lambda => &[
                "AFR",
                "AFR Channel 1",
                "AFR Channel 2",
                "Lambda",
                "Lambda 1",
                "Lambda 2",
                "O2",
            ],
            Self::LambdaTarget => &["AFR Target", "Lambda Target"],
            Self::TpsRate => &["TPS Rate"],
            Self::FuelCut => &[],
            Self::ClosedLoopState => &[],
            Self::CoolantTemp => &["Coolant Temp"],
            Self::Clutch => &[],
            Self::AeActive => &[],
        }
    }

    /// Lower-case substrings that identify the role more specifically than
    /// [`name_hints`](Self::name_hints); they score 50 so `dfcoActive` beats
    /// `DFCO: Timing retard` and `Short Term Fuel Trim` beats `O2 Control
    /// Bank 1 Output`.
    fn strong_hints(self) -> &'static [&'static str] {
        match self {
            Self::LambdaTarget => &["target lambda", "lambda target", "afr target", "target afr"],
            Self::TpsRate => &["tps dot", "tpsdot", "throttle position derivative"],
            Self::FuelCut => &["dfcoactive", "decel cut state", "dfco"],
            Self::ClosedLoopState => &["short term fuel trim", "stft", "gego", "total correction"],
            Self::Clutch => &["clutch state", "clutch switch"],
            Self::AeActive => &[
                "transient throttle load derivative",
                "tps ae active",
                "accel enrich",
            ],
            _ => &[],
        }
    }

    /// Lower-case substrings that suggest this role by name alone.
    fn name_hints(self) -> &'static [&'static str] {
        match self {
            Self::Rpm => &["engine speed", "rpm"],
            Self::Map => &["manifold pressure", "map"],
            Self::Tps => &["throttle position", "tps", "throttle pos"],
            Self::PulseWidth => &[
                "pulse width",
                "pulsewidth",
                "on time",
                "injection time",
                "inj pw",
                "effective pw",
                "actual pw",
                "inj duration",
                "duty cycle",
                "base pw",
            ],
            Self::Lambda => &["lambda", "wideband", "afr", "air/fuel", "air fuel", "o2"],
            Self::LambdaTarget => &["target"],
            Self::TpsRate => &[
                "tps dot",
                "tpsdot",
                "throttle position derivative",
                "tps delta",
                "tps rate",
            ],
            Self::FuelCut => &["dfco", "decel cut", "fuel cut", "overrun cut", "decel fuel"],
            Self::ClosedLoopState => &[
                "o2 control",
                "short term fuel trim",
                "stft",
                "gego",
                "ego correction",
                "total correction",
                "closed loop",
                "lambda correction",
                "o2 correction",
            ],
            Self::CoolantTemp => &["coolant", "clt", "engine temp"],
            Self::Clutch => &["clutch"],
            Self::AeActive => &[
                "transient throttle load derivative",
                "transient throttle enrichment load derivative",
                "tps ae active",
                "accel enrich",
                "acceleration enrichment",
                "ae active",
            ],
        }
    }

    /// Lower-case substrings that disqualify a name for this role even when a
    /// hint matched (e.g. `Target Lambda` must not become the lambda sensor).
    fn name_vetoes(self) -> &'static [&'static str] {
        match self {
            Self::Rpm => &[
                "target",
                "limit",
                "error",
                "derivative",
                "driveshaft",
                "rpm/s",
                "accel",
                "max",
                "min",
                "idle",
            ],
            Self::Map => &[
                "derivative",
                "target",
                "predicted",
                "fallback",
                "raw",
                "mapxrpm",
                "sensor seems",
                "error",
                "max",
                "min",
                "baro",
                "delta",
            ],
            Self::Tps => &[
                "derivative",
                "dot",
                "delta",
                "target",
                "error",
                "raw",
                "adc",
                "accumulator",
                "split",
                "status",
                "cal",
                "pedal",
                "rate",
                "2",
                "sub",
                "ae",
            ],
            Self::PulseWidth => &[
                "dead time",
                "adder",
                "growth",
                "staging",
                "target",
                "duty cycle _",
                "peak",
                "add fuel",
                "pw2",
                "pw3",
                "pw4",
                "highest",
                "short pulse",
            ],
            Self::Lambda => &[
                "target",
                "status",
                "error",
                "correction",
                "control",
                "raw",
                "time since",
                "heater",
                "trim",
                "delay",
                "good",
                "ready",
                "protect",
            ],
            Self::LambdaTarget => &[
                "error", "airmass", "airflow", "boost", "idle", "rpm", "cam", "angle", "position",
                "gear", "ratio",
            ],
            Self::TpsRate => &["max", "enrichment"],
            Self::FuelCut => &["retard", "timing"],
            Self::ClosedLoopState => &["long term", "ltft", "target", "error", "boost", "idle"],
            Self::CoolantTemp => &["gauge", "raw", "target", "error", "output"],
            Self::Clutch => &[],
            Self::AeActive => &[
                "max",
                "reset",
                "below threshold",
                "too short",
                "fractional",
                "peak",
                "start load",
                "add fuel",
            ],
        }
    }

    /// Spec categories consistent with the role, and whether that alone is
    /// enough (it is not for ambiguous categories like `Fuel`).
    fn spec_categories(self) -> &'static [ChannelCategory] {
        match self {
            Self::Rpm => &[ChannelCategory::Engine],
            Self::Map => &[ChannelCategory::Pressure],
            Self::Tps => &[ChannelCategory::DriverInput, ChannelCategory::Position],
            Self::PulseWidth => &[ChannelCategory::Fuel],
            Self::Lambda => &[ChannelCategory::Fuel],
            Self::LambdaTarget => &[ChannelCategory::Fuel],
            Self::TpsRate => &[ChannelCategory::DriverInput],
            Self::CoolantTemp => &[ChannelCategory::Temperature],
            Self::ClosedLoopState => &[ChannelCategory::Correction],
            Self::FuelCut | Self::Clutch | Self::AeActive => &[],
        }
    }

    /// Whether the median of the channel data is plausible for the role.
    /// `None` when the role has no plausibility band.
    fn plausible(self, med: f64) -> Option<bool> {
        match self {
            Self::Rpm => Some((300.0..=12_000.0).contains(&med)),
            // Haltech logs gauge pressure (negative at vacuum); accept it.
            Self::Map => Some((-100.0..=400.0).contains(&med) && med != 0.0),
            Self::Tps => Some((0.0..=100.0).contains(&med)),
            Self::PulseWidth => Some(med > 0.0 && med < 100_000.0),
            Self::Lambda | Self::LambdaTarget => {
                Some((0.5..=1.6).contains(&med) || (7.0..=24.0).contains(&med))
            }
            Self::CoolantTemp => Some((-40.0..=400.0).contains(&med)),
            _ => None,
        }
    }
}

/// Which channel supplies the load axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum LoadKind {
    #[default]
    Map,
    Tps,
}

impl LoadKind {
    pub fn role(self) -> ChannelRole {
        match self {
            Self::Map => ChannelRole::Map,
            Self::Tps => ChannelRole::Tps,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Map => "MAP",
            Self::Tps => "TPS",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            Self::Map => "kPa",
            Self::Tps => "%",
        }
    }
}

/// A role a generator wants mapped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleSpec {
    pub role: ChannelRole,
    pub required: bool,
}

impl RoleSpec {
    pub const fn required(role: ChannelRole) -> Self {
        Self {
            role,
            required: true,
        }
    }

    pub const fn optional(role: ChannelRole) -> Self {
        Self {
            role,
            required: false,
        }
    }
}

/// The user's channel assignment for one run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChannelMapping {
    /// Role -> raw channel name as it appears in the log.
    pub assignments: HashMap<ChannelRole, String>,
    pub load_kind: LoadKind,
}

impl ChannelMapping {
    pub fn get(&self, role: ChannelRole) -> Option<&str> {
        self.assignments.get(&role).map(String::as_str)
    }

    pub fn set(&mut self, role: ChannelRole, name: Option<String>) {
        match name {
            Some(n) if !n.is_empty() => {
                self.assignments.insert(role, n);
            }
            _ => {
                self.assignments.remove(&role);
            }
        }
    }

    pub fn is_mapped(&self, role: ChannelRole) -> bool {
        self.assignments.contains_key(&role)
    }

    /// Roles from `specs` that are required but unmapped.
    pub fn missing_required(&self, specs: &[RoleSpec]) -> Vec<ChannelRole> {
        specs
            .iter()
            .filter(|s| s.required && !self.is_mapped(s.role))
            .map(|s| s.role)
            .collect()
    }
}

/// One scored candidate for a role.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub channel: String,
    pub score: i32,
}

/// Result of [`suggest_mapping`].
#[derive(Clone, Debug, Default)]
pub struct Suggestion {
    pub mapping: ChannelMapping,
    /// Roles where the top two candidates were within 10 points of each
    /// other; the UI flags these for the user to confirm.
    pub ambiguous: Vec<ChannelRole>,
    /// All candidates with a positive score, best first, per role.
    pub candidates: HashMap<ChannelRole, Vec<Candidate>>,
}

fn name_score(role: ChannelRole, raw_name: &str, custom: Option<&HashMap<String, String>>) -> i32 {
    let lower = raw_name.to_lowercase();
    if role.name_vetoes().iter().any(|v| lower.contains(v)) {
        return 0;
    }
    let normalized = normalize_channel_name_with_custom(raw_name, custom);
    let mut score = 0;
    if role
        .canonical_names()
        .iter()
        .any(|c| c.eq_ignore_ascii_case(&normalized))
    {
        // Duty cycle works for step detection but a true pulse width is the
        // better channel when both are logged.
        score = if normalized.eq_ignore_ascii_case("Duty Cycle") {
            90
        } else {
            100
        };
    } else if role.strong_hints().iter().any(|h| lower.contains(h)) {
        score = 50;
    } else if let Some(meta) = get_spec_metadata(raw_name)
        && role.spec_categories().contains(&meta.category)
        && role.name_hints().iter().any(|h| lower.contains(h))
    {
        score = 60;
    } else if role.name_hints().iter().any(|h| lower.contains(h)) {
        score = 40;
    }
    if score > 0
        && (lower.contains("overall") || lower.contains("avg") || lower.contains("average"))
    {
        score -= 10;
    }
    score
}

/// Score every channel in `log` for each role in `specs` and pick the best.
pub fn suggest_mapping(
    log: &Log,
    specs: &[RoleSpec],
    custom: Option<&HashMap<String, String>>,
) -> Suggestion {
    let mut suggestion = Suggestion::default();
    let names: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    let mut median_cache: HashMap<usize, Option<f64>> = HashMap::new();

    for spec in specs {
        let role = spec.role;
        let mut candidates: Vec<Candidate> = Vec::new();
        for (idx, name) in names.iter().enumerate() {
            let mut score = name_score(role, name, custom);
            if score == 0 {
                continue;
            }
            if role.plausible(0.0).is_some() {
                let med = *median_cache
                    .entry(idx)
                    .or_insert_with(|| median(&log.get_channel_data(idx)));
                match med {
                    Some(m) if role.plausible(m) == Some(true) => {}
                    _ => score = 0,
                }
            }
            if score > 0 {
                candidates.push(Candidate {
                    channel: name.clone(),
                    score,
                });
            }
        }
        candidates.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.channel.cmp(&b.channel))
        });
        if let Some(best) = candidates.first() {
            suggestion
                .mapping
                .assignments
                .insert(role, best.channel.clone());
            if let Some(second) = candidates.get(1)
                && best.score - second.score < 10
            {
                suggestion.ambiguous.push(role);
            }
        }
        if !candidates.is_empty() {
            suggestion.candidates.insert(role, candidates);
        }
    }

    suggestion.mapping.load_kind = if suggestion.mapping.is_mapped(ChannelRole::Map) {
        LoadKind::Map
    } else {
        LoadKind::Tps
    };
    suggestion
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_scores_prefer_canonical_and_penalise_averages() {
        assert_eq!(name_score(ChannelRole::Rpm, "RPM", None), 100);
        assert_eq!(name_score(ChannelRole::Rpm, "Target RPM", None), 0);
        assert_eq!(name_score(ChannelRole::Lambda, "Wideband O2 1", None), 100);
        assert_eq!(
            name_score(ChannelRole::Lambda, "Wideband O2 Overall", None),
            90
        );
        assert_eq!(name_score(ChannelRole::Lambda, "Target Lambda", None), 0);
        assert!(name_score(ChannelRole::PulseWidth, "Injector 1 On Time", None) >= 40);
        assert_eq!(
            name_score(ChannelRole::PulseWidth, "Injection Stage 1 Dead Time", None),
            0
        );
        assert!(name_score(ChannelRole::TpsRate, "TPS DOT", None) >= 40);
        assert_eq!(name_score(ChannelRole::FuelCut, "dfcoActive", None), 50);
        assert_eq!(
            name_score(ChannelRole::FuelCut, "DFCO: Timing retard", None),
            0
        );
        assert_eq!(
            name_score(ChannelRole::FuelCut, "Decel Cut State", None),
            50
        );
        assert_eq!(
            name_score(
                ChannelRole::ClosedLoopState,
                "O2 Control Bank 1 Short Term Fuel Trim",
                None
            ),
            50
        );
        assert_eq!(
            name_score(
                ChannelRole::ClosedLoopState,
                "O2 Control Bank 1 Target",
                None
            ),
            0
        );
        assert_eq!(
            name_score(ChannelRole::Lambda, "lambdaCurrentlyGood", None),
            0
        );
        assert_eq!(name_score(ChannelRole::PulseWidth, "Duty Cycle", None), 90);
        assert!(name_score(ChannelRole::AeActive, "Fuel: TPS AE Active", None) >= 40);
        assert_eq!(
            name_score(ChannelRole::AeActive, "Fuel: TPS AE: reset time", None),
            0
        );
    }

    #[test]
    fn mapping_missing_required() {
        let mut m = ChannelMapping::default();
        let specs = [
            RoleSpec::required(ChannelRole::Rpm),
            RoleSpec::optional(ChannelRole::Clutch),
        ];
        assert_eq!(m.missing_required(&specs), vec![ChannelRole::Rpm]);
        m.set(ChannelRole::Rpm, Some("RPM".into()));
        assert!(m.missing_required(&specs).is_empty());
        m.set(ChannelRole::Rpm, None);
        assert!(!m.is_mapped(ChannelRole::Rpm));
    }
}
