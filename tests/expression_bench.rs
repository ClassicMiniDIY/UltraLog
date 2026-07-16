//! Benchmark for computed-channel formula evaluation over a large log.
//!
//! Ignored by default because it parses an 84 MB example log. Run with:
//!
//! ```bash
//! cargo test --release --test expression_bench -- --ignored --nocapture
//! ```

use std::time::Instant;
use ultralog::expression::{
    build_channel_bindings, compute_all_channel_statistics, evaluate_all_records,
    evaluate_all_records_with_stats, extract_channel_references,
};
use ultralog::parsers::haltech::Haltech;
use ultralog::parsers::types::Parseable;

const LOG_PATH: &str = "exampleLogs/haltech/2025-03-06_0937pm_Logs658to874.csv";

#[test]
#[ignore = "benchmark: parses an 84 MB log; run explicitly with --ignored"]
fn bench_evaluate_large_log() {
    let content = std::fs::read_to_string(LOG_PATH)
        .unwrap_or_else(|e| panic!("failed to read '{LOG_PATH}': {e}"));
    let log = Haltech.parse(&content).expect("should parse Haltech log");
    let available_channels: Vec<String> = log.channels.iter().map(|c| c.name()).collect();
    let num_records = log.data.len();
    eprintln!(
        "parsed {num_records} records, {} channels",
        available_channels.len()
    );

    let formulas = [
        ("simple", "RPM * 2 + 1", false),
        ("index shift", "RPM - RPM[-1]", false),
        (
            "time shift",
            "(\"Manifold Pressure\" - \"Manifold Pressure\"@-0.1s) * 10",
            false,
        ),
        ("functions", "sqrt(abs(RPM)) + sin(RPM / 1000)", false),
        ("z-score", "(RPM - _mean_RPM) / _stdev_RPM", true),
    ];

    let statistics = compute_all_channel_statistics(&available_channels, &log.data);

    for (label, formula, needs_stats) in formulas {
        let refs = extract_channel_references(formula);
        let bindings = build_channel_bindings(&refs, &available_channels)
            .unwrap_or_else(|e| panic!("bindings for '{formula}': {e}"));

        let mut best = f64::INFINITY;
        let mut len = 0;
        for _ in 0..3 {
            let start = Instant::now();
            let result = if needs_stats {
                evaluate_all_records_with_stats(
                    formula,
                    &bindings,
                    &log.data,
                    &log.times,
                    Some(&statistics),
                )
            } else {
                evaluate_all_records(formula, &bindings, &log.data, &log.times)
            }
            .unwrap_or_else(|e| panic!("evaluate '{formula}': {e}"));
            let elapsed = start.elapsed().as_secs_f64();
            best = best.min(elapsed);
            len = result.len();
        }
        assert_eq!(len, num_records);
        eprintln!(
            "{label:<12} {formula:<55} best of 3: {:>8.1} ms ({:>6.0} ns/record)",
            best * 1000.0,
            best * 1e9 / num_records as f64
        );
    }
}
