# Tuning Table Generators — Lambda Delay & Acceleration Enrichment

- **Date:** 2026-07-16
- **Status:** Draft
- **Issues:** [#4 Lambda delay table generator](https://github.com/ClassicMiniDIY/UltraLog/issues/4), [#3 Acceleration enrichment table generator](https://github.com/ClassicMiniDIY/UltraLog/issues/3)

## Summary

Two requested features mine loaded logs to auto-generate tuning tables:

1. **Lambda delay table (#4)** — measure the time between an injector pulse width change and the corresponding lambda/AFR response, binned by RPM × load. Tuners currently do this by hand across multiple logs; the result feeds the ECU's closed-loop O2 control delay table.
2. **Acceleration enrichment table (#3)** — detect tip-in events, measure the transient lean excursion versus target, and suggest an enrichment correction binned by RPM × TPS rate-of-change.

The two features share roughly 80% of their machinery: channel-role mapping, event detection over time series, 2D binning into an RPM × load grid, a table/heatmap results view with per-cell statistics, CSV export, and multi-log accumulation. This doc designs that shared framework once, with the two features as pluggable analyzers on top of it.

## Goals

- One **table-generation framework** in `src/analysis/` that both analyzers (and future ones — VE table verification, ignition scatter, etc.) plug into.
- **Channel mapping with auto-suggestion** driven by the existing normalization system (`src/normalize.rs`), editable by the user when auto-detection is wrong or ambiguous.
- **Per-cell sample counts and confidence** so sparse cells are visibly untrustworthy rather than silently wrong.
- **Multi-log accumulation** — several capture sessions fill one table before export.
- **CSV export** (and tab-separated clipboard copy) suitable for pasting into NSP, TunerStudio, EMU Black software, etc.
- Validate against `exampleLogs/haltech/` and `exampleLogs/speeduino/speeduino.mlg`.

## Non-Goals

- Writing tables back to an ECU or generating vendor-native calibration files. Output is CSV/clipboard only.
- Real-time analysis during log streaming.
- Automatic fuel-map (VE) correction. That is a separate, larger feature; this framework is a prerequisite for it.

## Background — existing infrastructure this builds on

| Piece | Location | What we reuse |
| --- | --- | --- |
| Analyzer trait + registry | `src/analysis/mod.rs` | `AnalysisError`, `LogDataAccess`, `timed_analyze`, registration/discovery patterns. The existing `Analyzer` trait returns one value per timestamp — tables don't fit it, so table generators get a sibling trait (below), not a shoehorn. |
| AFR/lambda unit handling | `src/analysis/afr.rs` | `FuelMixtureUnit`, `detect_fuel_mixture_unit()` — auto-detects whether a channel logs lambda (~1.0) or AFR (~14.7) and converts. Both analyzers need this. |
| Field normalization | `src/normalize.rs` | `normalize_channel_name_with_custom()` and the built-in map (RPM, MAP, TPS, `Pulse Width`, `Duty Cycle`, AFR/Lambda variants, `AFR Target`) power channel auto-suggestion. |
| 2D binning + heatmap render | `src/ui/scatter_plot.rs` | Precedent for the grid histogram (`HEATMAP_BINS`, per-cell hit counting) and the painted heatmap with hover/click cell inspection and legend. |
| Analysis panel UI | `src/ui/analysis_panel.rs` | `ParamDef`/`ParamType` (including `ParamType::Channel` dropdowns), category tabs, config get/set round-trip — the table-generator dialog follows the same conventions. |
| Tools panel entry point | `src/ui/tools_panel.rs` | Analyzers surface through the tools side panel; table generators get a section here. |
| Rate-of-change | `src/analysis/statistics.rs` (`RateOfChangeAnalyzer`) | Derivative computation for PW-dot / TPS-dot when the ECU doesn't log a derivative channel natively. |

## Architecture

### Module layout

```text
src/analysis/
├── mod.rs              # + register table generators, re-export tables module
├── tables/
│   ├── mod.rs          # TableAnalyzer trait, TableResult, TableGeneratorRegistry
│   ├── channel_map.rs  # ChannelRole, ChannelMapping, auto-suggestion
│   ├── binning.rs      # AxisSpec, TableGrid, CellStats, accumulation & merge
│   ├── events.rs       # derivative helpers, step/tip-in detection primitives
│   ├── lambda_delay.rs # LambdaDelayGenerator (#4)
│   └── accel_enrich.rs # AccelEnrichGenerator (#3)
src/ui/
└── table_generator.rs  # dialog (mapping + params), results view, CSV export
```

### Core trait

The existing `Analyzer` trait produces a `Vec<f64>` aligned to log timestamps. Table generators produce a 2D grid plus per-event diagnostics, so they get a parallel trait rather than an awkward encoding:

```rust
/// A generator that mines events from one or more logs into a 2D table.
pub trait TableAnalyzer: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn description(&self) -> &str;

    /// Channel roles this generator needs (required + optional).
    fn channel_roles(&self) -> Vec<RoleSpec>;

    /// Default axis specs (user-editable before running).
    fn default_axes(&self, log: Option<&Log>) -> (AxisSpec, AxisSpec);

    /// Detect events in one log and fold them into the accumulator.
    /// Called once per log; accumulation across logs happens in `TableAccumulator`.
    fn analyze(
        &self,
        log: &Log,
        mapping: &ChannelMapping,
        axes: &(AxisSpec, AxisSpec),
        acc: &mut TableAccumulator,
    ) -> Result<AnalysisRunReport, AnalysisError>;

    fn get_config(&self) -> AnalyzerConfig;   // reuse existing config type
    fn set_config(&mut self, config: &AnalyzerConfig);
    fn clone_box(&self) -> Box<dyn TableAnalyzer>;
}
```

`AnalyzerConfig` (string-keyed parameters) is reused verbatim so parameter persistence and the panel's parameter widgets carry over.

### Data structures

```rust
/// Semantic role a mapped channel plays in the analysis.
pub enum ChannelRole {
    Rpm,
    Load,        // MAP or TPS — user picks which flavor; affects axis label/units
    PulseWidth,  // injector PW (ms/us) or duty cycle (%)
    Lambda,      // wideband lambda or AFR (auto-detected via FuelMixtureUnit)
    LambdaTarget,// optional; enables excursion-vs-target instead of vs-baseline
    TpsDot,      // optional; native derivative channel (Haltech/Speeduino log these)
}

pub struct RoleSpec {
    pub role: ChannelRole,
    pub required: bool,
    pub label: String,       // i18n key
    pub hint: String,        // tooltip: what kinds of channels qualify
}

/// User's channel assignment for one run. Persisted per ECU type.
pub struct ChannelMapping {
    pub assignments: HashMap<ChannelRole, String>, // role -> raw channel name in log
    pub load_kind: LoadKind,                       // Map | Tps
}

/// One table axis: explicit breakpoints (uneven spacing allowed, like real ECU tables).
pub struct AxisSpec {
    pub label: String,          // "RPM", "MAP (kPa)", "TPS δ (%/s)"
    pub breakpoints: Vec<f64>,  // cell edges; N+1 edges -> N bins
}

/// Everything measured for one detected event (kept for the inspector view).
pub struct TableEvent {
    pub time: f64,          // event start in log time
    pub log_name: String,   // provenance for multi-log accumulation
    pub rpm: f64,
    pub load: f64,
    pub value: f64,         // delay in ms (lambda delay) or correction % (AE)
    pub quality: f32,       // 0..1 per-event quality (see analyzers below)
}

pub struct CellStats {
    pub samples: Vec<f64>,      // raw per-event values (needed for median/MAD merge)
    pub median: f64,
    pub mad: f64,               // median absolute deviation — robust spread
    pub count: usize,
    pub confidence: Confidence, // High | Medium | Low | Empty (derived, see below)
}

/// The accumulating table. Lives in app state; logs are folded in one at a time.
pub struct TableAccumulator {
    pub generator_id: String,
    pub axes: (AxisSpec, AxisSpec),
    pub cells: Vec<Vec<CellStats>>,   // [load_bin][rpm_bin]
    pub events: Vec<TableEvent>,      // all events, for inspection/undo of a log
    pub logs_included: Vec<String>,
}

pub struct AnalysisRunReport {
    pub events_detected: usize,
    pub events_rejected: usize,       // failed quality gates
    pub warnings: Vec<String>,        // e.g. "sample rate 5 Hz is coarse for delay measurement"
}
```

Storing raw samples per cell (not just running stats) is deliberate: medians and MADs can't be merged incrementally, and it enables "remove log X from the table" by re-folding the remaining events. Memory is trivial — even an aggressive session yields a few thousand events.

Axis defaults are derived from data when a log is loaded: 1st–99th percentile of the mapped RPM/load channels, rounded to tuner-friendly steps (RPM: 250/500; MAP: 10 kPa; TPS: 10%; TPS-dot: geometric 25/50/100/200/400/800 %/s). Users can edit breakpoints as a comma-separated list before running, matching how ECU software presents axes.

### Channel mapping & auto-suggestion

The mapping dialog runs once per (generator, ECU type) and is persisted (JSON via eframe storage, like `computed_library`):

1. For each `RoleSpec`, score every channel in the loaded log:
   - `normalize_channel_name_with_custom(name, custom_mappings)` equals the role's canonical target (`"RPM"`, `"MAP"`/`"TPS"`, `"Pulse Width"`/`"Duty Cycle"`, `"Lambda 1"`/`"AFR"`/`"AFR Channel 1"`, `"AFR Target"`) → strong match.
   - OpenECU Alliance spec metadata (`normalize::get_spec_metadata`) category/unit consistent with the role → medium match.
   - Substring heuristics as last resort (`"injector"`, `"pulse width"`, `"on time"`, `"derivative"`, `"dot"`).
2. Best match pre-fills the dropdown; ambiguity (multiple strong matches, e.g. Haltech's `Wideband O2 1` and `Wideband O2 Overall`) is flagged with a ⚠ so the user confirms.
3. Every role is an editable dropdown over all channels (same widget as `ParamType::Channel` in the analysis panel).

Multi-log accumulation re-runs auto-suggestion per log (channel sets can differ between captures) but keeps the user's explicit overrides when the same channel name exists.

### Event detection primitives (`events.rs`)

Shared by both analyzers:

- `smooth_median3(values)` — 3-sample median pre-filter to kill single-sample spikes before differentiation.
- `derivative(values, times)` — central difference, units/second. Used when no native derivative channel is mapped. When the ECU logs one (Haltech `Throttle Position Derivative`, Speeduino `TPS DOT`), the native channel is preferred — it's computed at ECU tick rate, not log rate.
- `noise_sigma(values, window)` — robust noise estimate (1.4826 × MAD of first differences) used to scale response thresholds so they adapt to sensor noise instead of using fixed magic numbers.
- `find_crossing_interpolated(times, values, threshold, from_idx)` — first threshold crossing with linear interpolation between samples. Critical for lambda delay: at a 20 Hz log rate one sample is 50 ms, a large fraction of a typical 80–300 ms delay; interpolation recovers sub-sample timing.
- `steady_state(values, window, tolerance)` — gate that rejects events where RPM/load is still moving (both analyzers need quasi-steady operating point for the bin assignment to be meaningful).

Sample-rate awareness: each run computes the log's median sample interval. Below 10 Hz, lambda-delay results get a warning and per-event `quality` is derated; below 4 Hz the run refuses with an explanatory error (`AnalysisError::InvalidParameter`).

## Lambda delay analyzer (#4)

**Physical model:** a step increase in injector PW enriches the charge; the wideband reads it after transport delay (exhaust travel) + sensor response time. That total delay, mapped over RPM × load, is what ECU closed-loop control wants. Delay shrinks with RPM/load (higher exhaust velocity), typically 50–500 ms on a small NA engine at low load, down to tens of ms at high load.

### Primary algorithm — step detection

1. **Pre-filter** PW and lambda with `smooth_median3`.
2. **Find PW steps:** compute PW-dot; a step event starts where `|ΔPW| / PW_baseline ≥ min_step_pct` within `step_window_ms` (defaults: 8%, 150 ms). `PW_baseline` = median PW over the 300 ms before the candidate. Both rising and falling steps are used (rising → lambda falls / AFR falls; falling → the reverse); the expected response direction is recorded per event.
3. **Gates:**
   - Steady operating point: RPM within ±200 rpm and load within ±8 kPa (or ±5% TPS) across the measurement window — otherwise the bin assignment is ambiguous; reject.
   - No overlapping step within `min_event_spacing_ms` (default 600 ms) — overlapping responses can't be attributed; reject.
   - PW above a floor (default 1.0 ms) to skip decel-fuel-cut regions; if a DFCO/decel-cut channel exists (Speeduino `DFCO`, Haltech `Decel Cut State`), reject events while it's active.
4. **Measure response:** lambda baseline = median over the 250 ms pre-event. Response threshold = `max(response_k × noise_sigma, min_response_delta)` in the expected direction (defaults: k = 3, min delta = 0.005 λ / 0.1 AFR — converted via `FuelMixtureUnit`). Delay = interpolated first-crossing time − step time. Timeout `response_timeout_ms` (default 1500 ms) → reject.
5. **Per-event quality:** product of factors — response magnitude vs noise, steadiness margin, sample-rate factor. Stored on `TableEvent`; cells can optionally weight the median by quality (off by default; plain median is more explainable).
6. **Bin** by (RPM, load) at the step instant; **median per cell**, MAD as spread.

### Alternative mode — windowed cross-correlation

Exposed as a mode toggle (`method = steps | xcorr`). For logs without crisp PW steps (steady cruise with dither), slide a 3 s window with 50% overlap; within each window with sufficient PW variance (coefficient of variation ≥ 2%), compute normalized cross-correlation between detrended PW and lambda over lags 0–1000 ms; accept the peak lag if peak correlation ≥ 0.6. Each accepted window contributes one event binned at the window's mean RPM/load. This mode ships in Phase 3 (see plan) — step detection covers the primary "do pulls, get table" workflow and is far easier to validate.

### Tunable parameters (all surfaced in the dialog, persisted via `AnalyzerConfig`)

| Parameter | Default | Notes |
| --- | --- | --- |
| `min_step_pct` | 8 % | Minimum PW step relative to baseline |
| `step_window_ms` | 150 | Step must complete within this |
| `min_event_spacing_ms` | 600 | Attribution guard |
| `response_k` | 3.0 | Threshold = k × noise σ |
| `min_response_delta` | 0.005 λ | Floor in lambda units, converted per `FuelMixtureUnit` |
| `response_timeout_ms` | 1500 | Reject if lambda never responds |
| `pw_floor_ms` | 1.0 | Skip fuel-cut regions |
| `steady_rpm_band` | ±200 rpm | Steadiness gate |
| `steady_load_band` | ±8 kPa / ±5 % | Per `LoadKind` |
| `method` | `steps` | `steps` \| `xcorr` (Phase 3) |

Output unit: **milliseconds**, formatted per cell to 0 decimal places.

## Acceleration enrichment analyzer (#3)

**Physical model:** on tip-in, airflow rises faster than fuel film delivers; without AE the mixture spikes lean for 100 ms–1 s. The tuner wants to know, per RPM × tip-in-rate cell: how deep and long the lean excursion is, and roughly how much extra fuel would have flattened it.

### Algorithm

1. **Tip-in detection:** TPS-dot (native channel preferred, else derivative of smoothed TPS) crosses `tps_dot_threshold` (default 50 %/s) and stays above it for ≥ 2 samples. Event magnitude = peak TPS-dot during the ramp. MAP-dot (default 400 kPa/s) is the fallback trigger for logs without TPS — same pipeline, different axis label.
2. **Lambda delay compensation:** the AFR response to the tip-in arrives one lambda-delay later. The AFR window is shifted by a delay estimate before excursion measurement — taken from a completed lambda-delay table for the matching cell when one exists in the session (the two features compose), else the `assumed_delay_ms` parameter (default 120 ms). Without this shift, excursions at high RPM get attributed to the wrong instant and depth is underestimated.
3. **Excursion measurement** over a window from tip-in until AFR recovers to within `recovery_band` of reference for 100 ms, capped at `max_event_ms` (default 2000 ms):
   - Reference = mapped `LambdaTarget` channel when present (Haltech `Target Lambda`, Speeduino `AFR Target`); else the pre-event 250 ms baseline.
   - **Depth:** peak lean deviation, in lambda units.
   - **Duration:** time above `lean_band` (default 0.02 λ over reference).
   - **Area:** integral of deviation over the window (reported in the event inspector; not binned in v1).
4. **Suggested correction:** `correction_pct = (peak_lambda / reference_lambda − 1) × 100`, clamped to 0–50%. This is the steady-flow fuel deficit at the excursion peak — an honest first-order starting point, and the doc/UI labels it as *"suggested starting correction"*, not a final value (wall-wetting dynamics mean the true transient dose differs; tuners iterate from here).
5. **Gates:** clutch/gearshift rejection when a clutch state channel exists (Haltech `Clutch State`); events where RPM changes > 25% during the window are rejected (shift mid-event); overlapping tip-ins merge into the larger event.
6. **Bin** by (RPM at tip-in, peak TPS-dot); **median correction per cell** — matching how ECUs axis their AE tables (Speeduino: TPSdot × correction; Haltech transient throttle: load-dot based).

### Tunable parameters

| Parameter | Default | Notes |
| --- | --- | --- |
| `tps_dot_threshold` | 50 %/s | Tip-in trigger |
| `map_dot_threshold` | 400 kPa/s | Fallback trigger |
| `assumed_delay_ms` | 120 | Used when no lambda-delay table in session |
| `lean_band` | 0.02 λ | Deviation counted as "lean" |
| `recovery_band` | 0.01 λ | Event end condition |
| `max_event_ms` | 2000 | Window cap |
| `max_rpm_change_pct` | 25 % | Gearshift guard |
| `correction_clamp_pct` | 50 % | Sanity clamp on suggestions |

Output unit: **% enrichment**, 1 decimal place. Secondary grids (depth in λ, duration in ms) are selectable views over the same events — the accumulator keeps `TableEvent`s, so re-binning a different measure is free.

## Confidence and sparse cells

Per-cell `Confidence` derives from count and dispersion:

| Level | Rule | Rendering |
| --- | --- | --- |
| Empty | n = 0 | Blank cell, no fill |
| Low | n < `min_samples` (default 3) **or** MAD/median > 0.5 | Value in gray italic + count badge red |
| Medium | n ≥ 3 and MAD/median ≤ 0.5 | Normal value, amber count badge |
| High | n ≥ `good_samples` (default 8) and MAD/median ≤ 0.25 | Normal value, green count badge |

Empty and Low cells are **never interpolated or smoothed in v1** — fabricated numbers in a tuning table are worse than gaps. CSV export writes empty cells as blank (not 0), and the export dialog offers "exclude Low-confidence cells" (on by default). A count grid and a MAD grid export alongside the value grid so downstream judgment is possible.

The results view surfaces a coverage line — "34/91 cells filled, 21 high confidence" — plus the per-log event counts from `AnalysisRunReport`, which tells the user *what kind of driving to log next* (e.g., no events above 4000 rpm → go do high-rpm pulls).

## UI flow

Entry point: a **"Table Generators"** collapsing section in the tools panel (`src/ui/tools_panel.rs`), listing the two generators with a status line when an accumulator is active ("Lambda Delay — 3 logs, 214 events"). Clicking opens the generator window (`src/ui/table_generator.rs`, an `egui::Window` like the analysis panel — no new `ActiveTool` variant; the result is a table, not a chart mode).

The window has three states:

1. **Setup** — channel mapping (auto-suggested dropdowns with ⚠ on ambiguity), axis breakpoint editors, parameter grid (same widget conventions as `analysis_panel.rs` `ParamDef`), and a *Run on current file* button.
2. **Results** — painted heatmap grid (cell fill = value on a color ramp, adapted from `scatter_plot.rs` rendering; cell text = value; corner badge = count with confidence color). Hover shows the cell tooltip (median, MAD, n, contributing logs); click opens an event inspector listing each `TableEvent` (time, log, value, quality) with a *jump to time in chart* action — this makes the tool auditable instead of a black box. Toolbar: **Add current file** (fold another loaded tab into the accumulator), **Remove log…**, **Reset**, measure selector (AE: correction / depth / duration), **Export CSV**, **Copy for paste** (tab-separated, no headers — what tuning-software grids accept).
3. **Empty/error** — no file loaded, or run report with zero events: show the rejection breakdown ("41 steps found, 39 rejected: 30 unsteady, 9 no response") so threshold tuning is guided, not guesswork.

All new strings go through `rust_i18n` `t!()` keys under `table_gen.*`, consistent with the analysis panel.

### CSV format

```csv
# UltraLog Lambda Delay Table (ms), generated 2026-07-16
# Logs: 2025-07-18_0215pm_Log1118.csv, ...
# Rows: MAP (kPa), Columns: RPM
,1000,1500,2000,2500,3000
30,,182,164,151,
40,201,176,158,143,139
...
# Sample counts
,1000,1500,2000,2500,3000
30,0,4,9,12,2
...
```

Value grid, count grid, MAD grid in one file, `#`-prefixed comment separators. Clipboard copy is the bare value grid only.

## Testing strategy

**Unit tests (synthetic signals)** — the core value of this feature is measurement correctness, so events.rs and both analyzers get synthetic-signal tests with known ground truth:

- Square PW step + lambda response delayed by exactly N ms at various sample rates (5/10/20/50 Hz) → recovered delay within half a sample interval (interpolation working).
- Noise-injected variants (σ scaled to real wideband noise) → detection still fires, delay error bounded.
- Steps during RPM sweeps → rejected by the steadiness gate.
- Tip-in ramp + lean excursion of known depth/duration → recovered within tolerance; overlapping tip-ins merge; mid-event RPM collapse (simulated shift) rejected.
- Binning: events on exact breakpoints land deterministically (lower-edge inclusive); median/MAD/confidence math; accumulator merge and per-log removal round-trips.

**Integration tests (example logs)** — `cargo test` fixtures against real files, asserting plausibility envelopes rather than exact values:

- `exampleLogs/haltech/2025-07-18_0215pm_Log1118.csv` — map `RPM`, `Manifold Pressure`, `Injector 1 On Time` (or `Injection Stage 1 Average Injection Time`), `Wideband O2 Overall`, `Target Lambda`. Assert: >0 lambda-delay events detected, all cell medians in 20–800 ms, auto-suggestion picks these channels unaided (this doubles as a normalization regression test — `Wideband O2 Overall` → `AFR` already has coverage in `normalize.rs`).
- Same file for AE: the log contains `Throttle Position Derivative` and the `Transient Throttle *` channels — assert our tip-in events temporally overlap regions where Haltech's own `Transient Throttle Fuel Peak Synchronous Output` is active (the ECU's AE detector is our reference detector).
- `exampleLogs/speeduino/speeduino.mlg` — map `RPM`, `MAP`, `PW`, `AFR`, `AFR Target`, native `TPS DOT`. Same plausibility assertions; additionally cross-check tip-in detection against the logged `Accel Enrich` / `Gammae` channels (events should coincide with `Accel Enrich` > 100%).
- One binary + one CSV format in CI keeps parser-interaction regressions covered; the remaining `exampleLogs/` formats are manual-QA targets.

**Manual QA:** run against a fresh Haltech capture on the actual car; sanity-check the delay table against the known-good hand-derived values that motivated #4.

## Phased implementation plan

**Phase 1 — framework + lambda delay (ships as one PR series):**

1. `src/analysis/tables/` — trait, binning, accumulator, channel mapping + auto-suggestion, events.rs primitives, with unit tests.
2. `lambda_delay.rs` step-detection analyzer + synthetic tests.
3. `src/ui/table_generator.rs` — setup/results/empty states, heatmap grid, event inspector, CSV + clipboard export; tools-panel section; i18n keys.
4. Integration fixtures for haltech + speeduino logs; wiki page draft (`UltraLog.wiki`).

**Phase 2 — acceleration enrichment:**

1. `accel_enrich.rs` — tip-in detection, delay compensation (session lambda-delay table lookup), excursion + correction math, synthetic tests.
2. Measure selector in the results view (correction/depth/duration); Speeduino `Accel Enrich` cross-check fixture.
3. Wiki page.

**Phase 3 — enhancements (each optional, independent):**

- Cross-correlation mode for lambda delay.
- Quality-weighted medians toggle.
- PNG/PDF export of the table view (reuse `src/ui/export.rs` plumbing).
- Persisted per-ECU mapping presets shared between the two generators.

## Open questions

1. **PW channel semantics vary** — Haltech logs per-injector on-time (ms), Speeduino logs PW1 (ms) and duty (%). Step detection is relative, so units don't matter for #4, but the mapping UI should display the detected unit so users pick the right channel. Any ECU that only logs *commanded* fuel including AE compensation will show AE events in the PW trace — acceptable for delay measurement (steps are steps), worth a wiki note.
2. **AE table axis flavors** — Speeduino bins AE by TPSdot only (1D × RPM scaling), Haltech by load-dot. v1 bins RPM × TPS-dot with MAP-dot fallback; if users ask for vendor-exact axis shapes, that's an `AxisSpec` preset, not a redesign.
3. **Where does the accumulator live across app restarts?** v1: session-only (in `UltraLogApp` state). Persisting partial tables to disk is a small follow-up if requested.
