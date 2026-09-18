//! CSV and clipboard (TSV) rendering of generated tables.
//!
//! The CSV carries the value grid, the sample-count grid and the MAD grid
//! under `#`-prefixed comment headers so downstream judgement is possible;
//! empty cells are written blank, never as `0`. The clipboard form is the
//! bare value grid with axis headers, which is what tuning-software grids
//! accept on paste.

use super::binning::{Confidence, TableGrid};
use super::{MeasureSpec, TableAccumulator};

/// Output unit for a lambda-delay table. ECUs disagree: MS3 / AEM Infinity
/// take engine cycles, MoTeC M1 / Link / Emerald take ignition events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DelayUnit {
    #[default]
    Milliseconds,
    EngineCycles,
    IgnitionEvents,
}

impl DelayUnit {
    pub const ALL: [DelayUnit; 3] = [
        DelayUnit::Milliseconds,
        DelayUnit::EngineCycles,
        DelayUnit::IgnitionEvents,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Milliseconds => "ms",
            Self::EngineCycles => "engine cycles",
            Self::IgnitionEvents => "ignition events",
        }
    }

    /// Convert a millisecond delay at `rpm` for an engine with `cylinders`.
    /// One four-stroke engine cycle is two revolutions, i.e. `120000 / rpm` ms.
    pub fn convert(self, ms: f64, rpm: f64, cylinders: u32) -> f64 {
        match self {
            Self::Milliseconds => ms,
            Self::EngineCycles => ms * rpm / 120_000.0,
            Self::IgnitionEvents => ms * rpm / 120_000.0 * cylinders as f64 / 2.0,
        }
    }

    pub fn decimals(self) -> usize {
        match self {
            Self::Milliseconds => 0,
            Self::EngineCycles | Self::IgnitionEvents => 2,
        }
    }
}

/// How cell values are transformed on the way out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExportOptions {
    /// Leave `Low` confidence cells blank.
    pub exclude_low: bool,
    /// Delay unit conversion (lambda delay tables only).
    pub delay_unit: DelayUnit,
    pub cylinders: u32,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            exclude_low: true,
            delay_unit: DelayUnit::Milliseconds,
            cylinders: 4,
        }
    }
}

fn fmt_number(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return String::new();
    }
    format!("{v:.decimals$}")
}

fn fmt_edge(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
    }
}

fn header_row(grid: &TableGrid, sep: &str) -> String {
    let mut cols = vec![format!(
        "{} \\ {}",
        grid.y_axis.header(),
        grid.x_axis.header()
    )];
    cols.extend((0..grid.cols()).map(|c| fmt_edge(grid.x_axis.lower_edge(c))));
    cols.join(sep)
}

/// Value of one cell after options: `None` renders blank.
fn cell_value(
    grid: &TableGrid,
    row: usize,
    col: usize,
    measure: &MeasureSpec,
    opts: &ExportOptions,
    delay_table: bool,
) -> Option<(f64, usize)> {
    let cell = grid.cell(row, col)?;
    if cell.is_empty() || (opts.exclude_low && cell.confidence == Confidence::Low) {
        return None;
    }
    if delay_table && measure.unit == "ms" && opts.delay_unit != DelayUnit::Milliseconds {
        let rpm = grid.x_axis.center(col);
        Some((
            opts.delay_unit.convert(cell.median, rpm, opts.cylinders),
            opts.delay_unit.decimals(),
        ))
    } else {
        Some((cell.median, measure.decimals))
    }
}

fn value_grid(
    grid: &TableGrid,
    measure: &MeasureSpec,
    opts: &ExportOptions,
    delay_table: bool,
    sep: &str,
) -> String {
    let mut out = header_row(grid, sep);
    out.push('\n');
    for r in 0..grid.rows() {
        let mut cols = vec![fmt_edge(grid.y_axis.lower_edge(r))];
        for c in 0..grid.cols() {
            cols.push(match cell_value(grid, r, c, measure, opts, delay_table) {
                Some((v, d)) => fmt_number(v, d),
                None => String::new(),
            });
        }
        out.push_str(&cols.join(sep));
        out.push('\n');
    }
    out
}

fn aux_grid(grid: &TableGrid, sep: &str, f: impl Fn(&super::CellStats) -> String) -> String {
    let mut out = header_row(grid, sep);
    out.push('\n');
    for r in 0..grid.rows() {
        let mut cols = vec![fmt_edge(grid.y_axis.lower_edge(r))];
        for c in 0..grid.cols() {
            cols.push(grid.cell(r, c).map(&f).unwrap_or_default());
        }
        out.push_str(&cols.join(sep));
        out.push('\n');
    }
    out
}

/// Full CSV: value grid, count grid, MAD grid, with `#` comment headers.
pub fn to_csv(
    acc: &TableAccumulator,
    measure_index: usize,
    opts: &ExportOptions,
    generated: &str,
) -> String {
    let measure = acc
        .measures
        .get(measure_index)
        .copied()
        .unwrap_or(MeasureSpec {
            key: "value",
            label: "Value",
            unit: "",
            decimals: 2,
        });
    let grid = acc.grid(measure_index);
    let delay_table = acc.generator == super::GeneratorKind::LambdaDelay;
    let unit = if delay_table && measure.unit == "ms" {
        opts.delay_unit.label().to_string()
    } else {
        measure.unit.to_string()
    };
    let mut out = String::new();
    out.push_str(&format!(
        "# UltraLog {} - {} ({}), generated {}\n",
        acc.generator.create().name(),
        measure.label,
        unit,
        generated
    ));
    let logs: Vec<&str> = acc.logs.iter().map(|l| l.name.as_str()).collect();
    out.push_str(&format!("# Logs: {}\n", logs.join(", ")));
    out.push_str(&format!(
        "# Rows: {}, Columns: {} (lower cell edges)\n",
        grid.y_axis.header(),
        grid.x_axis.header()
    ));
    let (filled, high) = grid.coverage();
    out.push_str(&format!(
        "# {} accepted events, {}/{} cells filled, {} high confidence{}\n",
        acc.accepted_count(),
        filled,
        grid.rows() * grid.cols(),
        high,
        if opts.exclude_low {
            ", low-confidence cells left blank"
        } else {
            ""
        }
    ));
    if delay_table && opts.delay_unit != DelayUnit::Milliseconds {
        out.push_str(&format!(
            "# Delay converted at the cell's centre RPM for {} cylinders\n",
            opts.cylinders
        ));
    }
    out.push_str(&value_grid(&grid, &measure, opts, delay_table, ","));
    out.push_str("# Sample counts\n");
    out.push_str(&aux_grid(&grid, ",", |c| c.count.to_string()));
    out.push_str("# Median absolute deviation\n");
    out.push_str(&aux_grid(&grid, ",", |c| {
        if c.is_empty() {
            String::new()
        } else {
            fmt_number(c.mad, measure.decimals.max(1))
        }
    }));
    out.push_str("# Confidence\n");
    out.push_str(&aux_grid(&grid, ",", |c| c.confidence.label().to_string()));
    out
}

/// Bare tab-separated value grid with axis headers, for pasting into a
/// tuning-software table.
pub fn to_clipboard_tsv(
    acc: &TableAccumulator,
    measure_index: usize,
    opts: &ExportOptions,
) -> String {
    let measure = acc
        .measures
        .get(measure_index)
        .copied()
        .unwrap_or(MeasureSpec {
            key: "value",
            label: "Value",
            unit: "",
            decimals: 2,
        });
    let grid = acc.grid(measure_index);
    let delay_table = acc.generator == super::GeneratorKind::LambdaDelay;
    value_grid(&grid, &measure, opts, delay_table, "\t")
}

#[cfg(test)]
mod tests {
    use super::super::{AxisSpec, GeneratorKind, RunReport, TableEvent};
    use super::*;

    fn acc() -> TableAccumulator {
        let axes = (
            AxisSpec::new("RPM", "", vec![1000.0, 2000.0, 3000.0]),
            AxisSpec::new("MAP", "kPa", vec![0.0, 50.0, 100.0]),
        );
        let measures = GeneratorKind::LambdaDelay.create().measures();
        let mut a = TableAccumulator::new(GeneratorKind::LambdaDelay, axes, measures);
        let mut events = Vec::new();
        for i in 0..8 {
            events.push(TableEvent {
                log_id: 1,
                log_name: "a.csv".into(),
                time: i as f64,
                rpm: 1500.0,
                axis_value: 25.0,
                values: vec![100.0 + i as f64, f64::NAN, 0.02, 10.0],
                quality: 1.0,
                reject: None,
                note: String::new(),
            });
        }
        events.push(TableEvent {
            log_id: 1,
            log_name: "a.csv".into(),
            time: 20.0,
            rpm: 2500.0,
            axis_value: 75.0,
            values: vec![300.0, f64::NAN, 0.02, 10.0],
            quality: 1.0,
            reject: None,
            note: String::new(),
        });
        a.add_log(
            1,
            "a.csv",
            events.clone(),
            RunReport::from_events("a.csv", &events),
        );
        a
    }

    #[test]
    fn delay_unit_conversion() {
        // 100 ms at 3000 rpm = 2.5 cycles = 5 ignition events on a 4-cyl.
        assert_eq!(DelayUnit::Milliseconds.convert(100.0, 3000.0, 4), 100.0);
        assert!((DelayUnit::EngineCycles.convert(100.0, 3000.0, 4) - 2.5).abs() < 1e-9);
        assert!((DelayUnit::IgnitionEvents.convert(100.0, 3000.0, 4) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn csv_has_all_grids_and_blank_low_cells() {
        let a = acc();
        let csv = to_csv(&a, 0, &ExportOptions::default(), "2026-09-18");
        assert!(
            csv.starts_with(
                "# UltraLog Lambda Delay Table - Dead time (ms), generated 2026-09-18\n"
            )
        );
        assert!(csv.contains("# Logs: a.csv\n"));
        assert!(csv.contains("MAP (kPa) \\ RPM,1000,2000\n"));
        // High-confidence cell present, single-sample (Low) cell blank.
        assert!(csv.contains("\n0,104,\n"), "{csv}");
        assert!(csv.contains("\n50,,\n"), "{csv}");
        assert!(csv.contains("# Sample counts\n"));
        assert!(csv.contains("\n0,8,0\n"));
        assert!(csv.contains("\n50,0,1\n"));
        assert!(csv.contains("# Median absolute deviation\n"));
        assert!(csv.contains("# Confidence\n"));
        assert!(csv.contains("high,empty"));

        let all = ExportOptions {
            exclude_low: false,
            ..Default::default()
        };
        let csv = to_csv(&a, 0, &all, "x");
        assert!(csv.contains("\n50,,300\n"), "{csv}");
    }

    #[test]
    fn csv_converts_delay_units() {
        let a = acc();
        let opts = ExportOptions {
            exclude_low: false,
            delay_unit: DelayUnit::EngineCycles,
            cylinders: 4,
        };
        let csv = to_csv(&a, 0, &opts, "x");
        assert!(csv.contains("(engine cycles)"));
        // 103.5 ms at the 1500 rpm cell centre = 1.29 cycles.
        assert!(csv.contains("\n0,1.29,\n"), "{csv}");
        // Non-ms measures are untouched.
        let csv = to_csv(&a, 2, &opts, "x");
        assert!(csv.contains("\n0,0.020,"), "{csv}");
    }

    #[test]
    fn clipboard_is_bare_tsv() {
        let a = acc();
        let tsv = to_clipboard_tsv(&a, 0, &ExportOptions::default());
        assert_eq!(tsv, "MAP (kPa) \\ RPM\t1000\t2000\n0\t104\t\n50\t\t\n");
    }
}
