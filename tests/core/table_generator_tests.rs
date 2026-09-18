//! Table generator tests against real example logs.
//!
//! Synthetic ground-truth tests live next to the generators in
//! `src/analysis/tables/`. These tests assert plausibility envelopes and
//! auto-suggestion behaviour on the shipped fixtures:
//!
//! - MegaSquirt `2026-04-12_12.49.36.mlg` (15 Hz, 668 s, engine running):
//!   `PW`, `Lambda`, `TPS DOT`, `Accel Enrich` - the primary real-log fixture
//!   for both generators and the low-rate warning path.
//! - Haltech `2025-07-18_0215pm_Log1118.csv` (50 Hz throttle-blip log):
//!   auto-suggestion regression, sentinel masking, rejection breakdown, and
//!   tip-in detection against the ECU's own transient-throttle channel.
//! - rusEFI `rusefilog.mlg` (100 Hz, no wideband): tip-in detection against
//!   `Fuel: TPS AE Active`, and the flat-lambda warning.

use std::collections::HashMap;

use ultralog::analysis::tables::events::find_rate_runs;
use ultralog::analysis::tables::export::{ExportOptions, to_clipboard_tsv, to_csv};
use ultralog::analysis::tables::lambda_delay::{LambdaDelayGenerator, Profile};
use ultralog::analysis::tables::{
    ChannelRole, GeneratorContext, GeneratorKind, RejectReason, TableAccumulator, TableAnalyzer,
    suggest_mapping,
};
use ultralog::parsers::types::Log;
use ultralog::parsers::{Haltech, Parseable, Speeduino};

use crate::common::{example_file_exists, example_files, read_example_binary, read_example_file};

const MEGASQUIRT_MLG: &str = "exampleLogs/megasquirt/2026-04-12_12.49.36.mlg";

fn megasquirt() -> Log {
    Speeduino::parse_binary(&read_example_binary(MEGASQUIRT_MLG)).expect("MegaSquirt MLG parses")
}

fn haltech_small() -> Log {
    Haltech
        .parse(&read_example_file(example_files::HALTECH_SMALL))
        .expect("Haltech CSV parses")
}

fn rusefi() -> Log {
    Speeduino::parse_binary(&read_example_binary(example_files::RUSEFI_MLG))
        .expect("rusEFI MLG parses")
}

fn column(log: &Log, name: &str) -> Vec<f64> {
    let idx = log
        .channels
        .iter()
        .position(|c| c.name() == name)
        .unwrap_or_else(|| panic!("channel {name} missing"));
    log.get_channel_data(idx)
}

fn mapped(map: &HashMap<ChannelRole, String>, role: ChannelRole) -> &str {
    map.get(&role).map(String::as_str).unwrap_or("(none)")
}

// ---------------------------------------------------------------------------
// Auto-suggestion
// ---------------------------------------------------------------------------

#[test]
fn haltech_auto_suggestion_picks_the_documented_channels() {
    let log = haltech_small();
    let g = GeneratorKind::LambdaDelay.create();
    let s = suggest_mapping(&log, &g.roles(), None);
    let m = &s.mapping.assignments;
    assert_eq!(mapped(m, ChannelRole::Rpm), "RPM");
    assert_eq!(mapped(m, ChannelRole::Map), "Manifold Pressure");
    assert_eq!(mapped(m, ChannelRole::Tps), "Throttle Position");
    assert_eq!(mapped(m, ChannelRole::PulseWidth), "Injector 1 On Time");
    // A single sensor beats the averaged channel.
    assert_eq!(mapped(m, ChannelRole::Lambda), "Wideband O2 1");
    assert_eq!(mapped(m, ChannelRole::FuelCut), "Decel Cut State");
    assert_eq!(
        mapped(m, ChannelRole::ClosedLoopState),
        "O2 Control Bank 1 Short Term Fuel Trim"
    );
    assert_eq!(mapped(m, ChannelRole::CoolantTemp), "Coolant Temperature");
    assert!(mapped(m, ChannelRole::Clutch).starts_with("Clutch"));
    // `Clutch State` vs `Clutch Switch Input State` is a genuine tie.
    assert!(
        s.ambiguous.contains(&ChannelRole::Clutch),
        "{:?}",
        s.ambiguous
    );
    assert!(!s.ambiguous.contains(&ChannelRole::Lambda));

    let g = GeneratorKind::AccelEnrich.create();
    let s = suggest_mapping(&log, &g.roles(), None);
    let m = &s.mapping.assignments;
    assert_eq!(
        mapped(m, ChannelRole::TpsRate),
        "Throttle Position Derivative"
    );
    assert_eq!(mapped(m, ChannelRole::LambdaTarget), "Target Lambda");
    assert_eq!(
        mapped(m, ChannelRole::AeActive),
        "Transient Throttle Load Derivative"
    );
}

#[test]
fn megasquirt_auto_suggestion() {
    let log = megasquirt();
    let g = GeneratorKind::LambdaDelay.create();
    let s = suggest_mapping(&log, &g.roles(), None);
    let m = &s.mapping.assignments;
    assert_eq!(mapped(m, ChannelRole::Rpm), "RPM");
    assert_eq!(mapped(m, ChannelRole::Map), "MAP");
    assert_eq!(mapped(m, ChannelRole::PulseWidth), "PW");
    assert!(matches!(mapped(m, ChannelRole::Lambda), "AFR" | "Lambda"));
    // AFR and Lambda are both logged; the user is asked to confirm.
    assert!(s.ambiguous.contains(&ChannelRole::Lambda));
    assert_eq!(mapped(m, ChannelRole::FuelCut), "DFCO");
    assert_eq!(mapped(m, ChannelRole::ClosedLoopState), "Gego");

    let g = GeneratorKind::AccelEnrich.create();
    let s = suggest_mapping(&log, &g.roles(), None);
    let m = &s.mapping.assignments;
    assert_eq!(mapped(m, ChannelRole::TpsRate), "TPS DOT");
    assert_eq!(mapped(m, ChannelRole::LambdaTarget), "Lambda Target");
    assert_eq!(mapped(m, ChannelRole::AeActive), "Accel Enrich");
}

#[test]
fn rusefi_auto_suggestion_vetoes_flat_lambda_and_flag_channels() {
    let log = rusefi();
    let g = GeneratorKind::AccelEnrich.create();
    let s = suggest_mapping(&log, &g.roles(), None);
    let m = &s.mapping.assignments;
    assert_eq!(mapped(m, ChannelRole::Rpm), "RPM");
    assert_eq!(mapped(m, ChannelRole::Map), "MAP");
    assert_eq!(mapped(m, ChannelRole::Tps), "TPS");
    assert_eq!(mapped(m, ChannelRole::AeActive), "Fuel: TPS AE Active");
    assert_eq!(mapped(m, ChannelRole::CoolantTemp), "CLT");
    // `Lambda` is all zeros in this log and `lambdaCurrentlyGood` is a flag:
    // neither may be picked as the wideband.
    assert_ne!(mapped(m, ChannelRole::Lambda), "lambdaCurrentlyGood");
    assert_ne!(mapped(m, ChannelRole::Lambda), "Lambda");
    let g = GeneratorKind::LambdaDelay.create();
    let s = suggest_mapping(&log, &g.roles(), None);
    let m = &s.mapping.assignments;
    assert_eq!(
        mapped(m, ChannelRole::PulseWidth),
        "Fuel: Last inj pulse width"
    );
    assert_eq!(mapped(m, ChannelRole::FuelCut), "dfcoActive");
}

// ---------------------------------------------------------------------------
// Lambda delay
// ---------------------------------------------------------------------------

#[test]
fn megasquirt_lambda_delay_driving_log_runs_and_warns() {
    let log = megasquirt();
    let g = LambdaDelayGenerator::default();
    let s = suggest_mapping(&log, &g.roles(), None);
    let axes = g.default_axes(&log, &s.mapping);
    assert!(axes.0.is_valid() && axes.1.is_valid());
    let (events, report) = g
        .analyze(
            &log,
            "megasquirt",
            &s.mapping,
            &axes,
            &GeneratorContext::default(),
        )
        .expect("runs");
    assert_eq!(events.len(), report.candidates);
    assert!(report.candidates > 50, "{}", report.summary());
    // A driving log is mostly unsteady / overlapping: the rejection
    // breakdown is the product here.
    assert!(
        report.rejected.contains_key(&RejectReason::Unsteady),
        "{}",
        report.summary()
    );
    assert!(
        report.rejected.contains_key(&RejectReason::Overlap),
        "{}",
        report.summary()
    );
    assert!(
        report.warnings.iter().any(|w| w.contains("15 Hz")),
        "low-rate warning expected: {:?}",
        report.warnings
    );
    assert!(report.warnings.iter().any(|w| w.contains("AFR")));
    for e in events.iter().filter(|e| e.accepted()) {
        let ms = e.value(0);
        assert!(
            (20.0..=1500.0).contains(&ms),
            "dead time {ms} ms out of envelope"
        );
        assert!(e.rpm > 500.0 && e.rpm < 7000.0);
    }
    // The relaxed profile never rejects more than strict.
    let mut relaxed = LambdaDelayGenerator::default();
    relaxed.apply_profile(Profile::Relaxed);
    let (_, relaxed_report) = relaxed
        .analyze(
            &log,
            "megasquirt",
            &s.mapping,
            &axes,
            &GeneratorContext::default(),
        )
        .expect("runs");
    assert!(relaxed_report.accepted >= report.accepted);
}

#[test]
fn haltech_blip_log_is_all_rejected_with_a_breakdown() {
    let log = haltech_small();
    let g = LambdaDelayGenerator::default();
    let s = suggest_mapping(&log, &g.roles(), None);
    let axes = g.default_axes(&log, &s.mapping);
    let (events, report) = g
        .analyze(
            &log,
            "haltech",
            &s.mapping,
            &axes,
            &GeneratorContext::default(),
        )
        .expect("runs");
    assert!(report.candidates >= 10, "{}", report.summary());
    // RPM 630 -> 3500 in 17 s with constant PW steps: strict gating rejects
    // at least 90 %.
    assert!(
        report.accepted * 10 <= report.candidates,
        "strict should reject >= 90 %: {}",
        report.summary()
    );
    assert!(report.total_rejected() > 0);
    assert!(report.summary().contains("rejected"));
    // Every event, accepted or not, stays in the list.
    assert_eq!(events.len(), report.candidates);
    // The Haltech sentinel rows are masked, not treated as readings: the
    // wideband column carries -2147483.637 samples and nothing panicked or
    // produced an absurd delay.
    let wb = column(&log, "Wideband O2 1");
    assert!(
        wb.iter().any(|v| *v < -1e6),
        "fixture should contain sentinels"
    );
    for e in events.iter().filter(|e| e.accepted()) {
        assert!(e.value(0).is_finite() && e.value(0) < 2000.0);
    }
}

#[test]
fn rusefi_flat_lambda_warns_and_rejects() {
    let log = rusefi();
    let g = LambdaDelayGenerator::default();
    let mut s = suggest_mapping(&log, &g.roles(), None);
    // Force the (all-zero) Lambda channel to exercise the flat-sensor path.
    s.mapping.set(ChannelRole::Lambda, Some("Lambda".into()));
    let axes = g.default_axes(&log, &s.mapping);
    let (_, report) = g
        .analyze(
            &log,
            "rusefi",
            &s.mapping,
            &axes,
            &GeneratorContext::default(),
        )
        .expect("runs");
    assert_eq!(report.accepted, 0);
    assert!(
        report.warnings.iter().any(|w| w.contains("never changes")),
        "{:?}",
        report.warnings
    );
}

// ---------------------------------------------------------------------------
// Acceleration enrichment
// ---------------------------------------------------------------------------

#[test]
fn megasquirt_accel_enrich_events_bin_and_export() {
    let log = megasquirt();
    let g = GeneratorKind::AccelEnrich.create();
    let s = suggest_mapping(&log, &g.roles(), None);
    let axes = g.default_axes(&log, &s.mapping);
    assert_eq!(axes.1.unit, "%/s");
    let (events, report) = g
        .analyze(
            &log,
            "megasquirt",
            &s.mapping,
            &axes,
            &GeneratorContext::default(),
        )
        .expect("runs");
    assert!(report.accepted >= 20, "{}", report.summary());
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("native throttle rate"))
    );
    assert!(report.warnings.iter().any(|w| w.contains("additional")));
    for e in events.iter().filter(|e| e.accepted()) {
        assert!(
            e.value(0).abs() <= 50.0,
            "correction {} outside clamp",
            e.value(0)
        );
        assert!(
            e.axis_value >= 50.0,
            "peak rate {} below trigger",
            e.axis_value
        );
        assert!(e.value(2) >= 0.0);
        assert_eq!(
            e.value(5),
            120.0,
            "assumed delay used without a delay table"
        );
        assert!(e.note.contains("ECU AE active"));
    }
    // Events that the ECU did not enrich are rejected as a kind mismatch
    // rather than mixed into an "additional" table.
    assert!(
        report.rejected.contains_key(&RejectReason::AeKindMismatch),
        "{}",
        report.summary()
    );
    // Tip-ins coincide with the ECU's own enrichment: every accepted event
    // has Accel Enrich > 100 % within its window.
    let ae = column(&log, "Accel Enrich");
    for e in events.iter().filter(|e| e.accepted()) {
        let hit = log
            .times
            .iter()
            .zip(&ae)
            .any(|(&t, &v)| t >= e.time && t <= e.time + 2.2 && v > 100.5);
        assert!(hit, "event at {} s has no ECU AE activity", e.time);
    }

    let mut acc = TableAccumulator::new(GeneratorKind::AccelEnrich, axes, g.measures());
    acc.add_log(7, "megasquirt", events, report);
    let grid = acc.grid(0);
    let (filled, _) = grid.coverage();
    assert!(filled >= 5, "coverage {filled}");
    let csv = to_csv(&acc, 0, &ExportOptions::default(), "test");
    assert!(csv.contains("# UltraLog Acceleration Enrichment Table - Suggested correction (%)"));
    assert!(csv.contains("# Sample counts"));
    let tsv = to_clipboard_tsv(&acc, 0, &ExportOptions::default());
    assert!(tsv.starts_with("TPS rate (%/s) \\ RPM\t"));
    assert_eq!(tsv.lines().count(), grid.rows() + 1);
}

#[test]
fn megasquirt_lambda_delay_table_feeds_accel_enrich() {
    // Composition: a lambda-delay grid from the same session shifts the AE
    // window. With a synthetic High-confidence grid every accepted event
    // records "delay from table".
    let log = megasquirt();
    let g = GeneratorKind::AccelEnrich.create();
    let s = suggest_mapping(&log, &g.roles(), None);
    let axes = g.default_axes(&log, &s.mapping);
    let ld = GeneratorKind::LambdaDelay.create();
    let ld_axes = ld.default_axes(&log, &suggest_mapping(&log, &ld.roles(), None).mapping);
    let mut points = Vec::new();
    for c in 0..ld_axes.0.bins() {
        for r in 0..ld_axes.1.bins() {
            for i in 0..8 {
                points.push((ld_axes.0.center(c), ld_axes.1.center(r), 200.0 + i as f64));
            }
        }
    }
    let grid = ultralog::analysis::tables::TableGrid::build(
        ld_axes.0.clone(),
        ld_axes.1.clone(),
        points,
        Default::default(),
    );
    let ctx = GeneratorContext {
        delay_table: Some(&grid),
    };
    let (events, report) = g
        .analyze(&log, "megasquirt", &s.mapping, &axes, &ctx)
        .expect("runs");
    assert!(report.accepted > 0, "{}", report.summary());
    assert!(
        !report
            .warnings
            .iter()
            .any(|w| w.contains("No lambda delay table"))
    );
    for e in events.iter().filter(|e| e.accepted()) {
        assert!(
            (e.value(5) - 203.5).abs() < 1.0,
            "delay used {}",
            e.value(5)
        );
        assert!(e.note.contains("delay from table"));
    }
}

#[test]
fn haltech_tip_in_detection_matches_the_ecu_transient_channel() {
    // The generator's tip-in detector on the native derivative channel must
    // overlap the rows where Haltech's own transient-throttle load
    // derivative is non-zero, and never fire where it stays at zero for a
    // whole second around the event.
    let log = haltech_small();
    let rate = column(&log, "Throttle Position Derivative");
    let ecu = column(&log, "Transient Throttle Load Derivative");
    let runs = find_rate_runs(&rate, 50.0, 2);
    assert!(
        runs.len() >= 3,
        "expected several tip-ins, got {}",
        runs.len()
    );
    let mut covered = 0;
    for run in &runs {
        let lo = run.start.saturating_sub(10);
        let hi = (run.end + 25).min(ecu.len() - 1);
        if ecu[lo..=hi].iter().any(|v| *v > 0.0) {
            covered += 1;
        }
    }
    assert!(
        covered * 10 >= runs.len() * 8,
        "only {covered}/{} detected tip-ins overlap ECU AE activity",
        runs.len()
    );
    // Rows where the ECU applied enrichment: at least 80 % lie within a
    // detected run (allowing the ECU's decay tail after the ramp).
    let active_rows: Vec<usize> = ecu
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > 0.0)
        .map(|(i, _)| i)
        .collect();
    assert!(!active_rows.is_empty());
    let inside = active_rows
        .iter()
        .filter(|&&i| runs.iter().any(|r| i + 5 >= r.start && i <= r.end + 40))
        .count();
    assert!(
        inside * 10 >= active_rows.len() * 8,
        "{inside}/{} ECU-active rows inside detected tip-ins",
        active_rows.len()
    );

    // The full generator runs; the free-revving blip log yields events with
    // the ECU AE role mapped and a rejection breakdown, never a panic.
    let g = GeneratorKind::AccelEnrich.create();
    let s = suggest_mapping(&log, &g.roles(), None);
    let axes = g.default_axes(&log, &s.mapping);
    let (events, report) = g
        .analyze(
            &log,
            "haltech",
            &s.mapping,
            &axes,
            &GeneratorContext::default(),
        )
        .expect("runs");
    assert!(report.candidates >= 3, "{}", report.summary());
    assert_eq!(events.len(), report.candidates);
    assert!(report.warnings.iter().any(|w| w.contains("additional")));
}

#[test]
fn rusefi_tip_ins_coincide_with_tps_ae_active() {
    let log = rusefi();
    let tps = column(&log, "TPS");
    let flag = column(&log, "Fuel: TPS AE Active");
    let rate = ultralog::analysis::statistics::time_derivative(
        &ultralog::analysis::filters::median_filter(&tps, 3),
        &log.times,
    );
    let runs = find_rate_runs(&rate, 50.0, 2);
    assert!(!runs.is_empty());
    let active: Vec<usize> = flag
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > 0.5)
        .map(|(i, _)| i)
        .collect();
    assert!(!active.is_empty(), "fixture should have AE activity");
    // Every ECU-active row sits inside or just after a detected tip-in.
    let inside = active
        .iter()
        .filter(|&&i| runs.iter().any(|r| i + 5 >= r.start && i <= r.end + 30))
        .count();
    assert!(inside * 10 >= active.len() * 8, "{inside}/{}", active.len());
}

#[test]
fn large_haltech_log_runs_quickly_when_present() {
    if !example_file_exists(example_files::HALTECH_LARGE) {
        return;
    }
    let log = Haltech
        .parse(&read_example_file(example_files::HALTECH_LARGE))
        .expect("large Haltech log parses");
    for kind in GeneratorKind::ALL {
        let g = kind.create();
        let s = suggest_mapping(&log, &g.roles(), None);
        let axes = g.default_axes(&log, &s.mapping);
        let (_, report) = g
            .analyze(
                &log,
                "large",
                &s.mapping,
                &axes,
                &GeneratorContext::default(),
            )
            .expect("runs");
        assert!(
            report.computation_time_ms < 5_000,
            "{} took {} ms",
            g.name(),
            report.computation_time_ms
        );
    }
}
