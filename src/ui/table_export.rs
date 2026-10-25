//! ExportSheet models for generated tuning tables (lambda delay, accel enrichment).
//!
//! [`table_sheet`] is a free function over a [`TableAccumulator`] so it is unit-testable without
//! an `UltraLogApp`. Cell values, blanking of low-confidence cells, and delay-unit conversion all
//! go through `analysis::tables::export::cell_value`, the same path CSV export uses, so PNG, PDF,
//! CSV and clipboard never disagree. The sheet itself (layout, export theme, fonts) is drawn by
//! `ui::export_sheet`.

use crate::analysis::tables::binning::Confidence;
use crate::analysis::tables::export::{ExportOptions, cell_value, fmt_edge};
use crate::analysis::tables::{GeneratorKind, TableAccumulator};
use crate::ui::export::{ExportResult, group_thousands};
use crate::ui::export_sheet::{self, Body, GridModel, Sheet};
use crate::ui::heatmap::MapKind;

/// `(value, decimals)` per cell after export options; `None` renders blank.
type ResolvedCells = Vec<Vec<Option<(f64, usize)>>>;

/// Resolve every cell to `(value, decimals)` after export options, plus the
/// value range used for colouring.
fn resolve_cells(
    acc: &TableAccumulator,
    measure_idx: usize,
    opts: &ExportOptions,
) -> ExportResult<(ResolvedCells, f64, f64)> {
    let measure = acc
        .measures
        .get(measure_idx)
        .ok_or("Table has no measures")?;
    let grid = acc.grid(measure_idx);
    if grid.rows() == 0 || grid.cols() == 0 {
        return Err("Table has no cells".into());
    }
    let delay_table = acc.generator == GeneratorKind::LambdaDelay;
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    let cells: ResolvedCells = (0..grid.rows())
        .map(|r| {
            (0..grid.cols())
                .map(|c| {
                    let v = cell_value(&grid, r, c, measure, opts, delay_table);
                    if let Some((x, _)) = v {
                        lo = lo.min(x);
                        hi = hi.max(x);
                    }
                    v
                })
                .collect()
        })
        .collect();
    if !lo.is_finite() {
        lo = 0.0;
        hi = 1.0;
    }
    Ok((cells, lo, hi))
}

fn normalize(v: f64, lo: f64, hi: f64) -> f64 {
    if (hi - lo).abs() < f64::EPSILON {
        0.5
    } else {
        ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
    }
}

/// Edge labels at their positions along the axis (edges sit on the cell boundaries).
fn edge_ticks(edges: &[f64]) -> Vec<(f32, String)> {
    let n = edges.len().saturating_sub(1).max(1) as f32;
    edges
        .iter()
        .enumerate()
        .map(|(i, &e)| (i as f32 / n, fmt_edge(e)))
        .collect()
}

/// The ExportSheet for a table: values, count badges by confidence, edge labels and a
/// ColorScale for the measure.
pub fn table_sheet(
    acc: &TableAccumulator,
    measure_idx: usize,
    opts: &ExportOptions,
    title: &str,
) -> ExportResult<Sheet> {
    let (cells, lo, hi) = resolve_cells(acc, measure_idx, opts)?;
    let grid = acc.grid(measure_idx);
    let (rows, cols) = (grid.rows(), grid.cols());
    let measure = &acc.measures[measure_idx];
    let unit = if acc.generator == GeneratorKind::LambdaDelay && measure.unit == "ms" {
        opts.delay_unit.label().to_string()
    } else {
        measure.unit.to_string()
    };

    // Same map choice as the on-screen table (`render_table_tool_view`): values on both sides of
    // zero use the diverging map centred on zero, magnitudes the sequential map.
    let kind = if lo < 0.0 && hi > 0.0 {
        MapKind::Diverging
    } else {
        MapKind::Sequential
    };
    let span = lo.abs().max(hi.abs()).max(f64::EPSILON);
    let norm = cells
        .iter()
        .map(|row| {
            row.iter()
                .map(|v| {
                    v.map(|(v, _)| match kind {
                        MapKind::Diverging => (0.5 + v / (2.0 * span)).clamp(0.0, 1.0) as f32,
                        MapKind::Sequential => normalize(v, lo, hi) as f32,
                    })
                })
                .collect()
        })
        .collect();
    let labels = cells
        .iter()
        .map(|row| {
            row.iter()
                .map(|v| v.map(|(v, d)| format!("{v:.d$}")))
                .collect()
        })
        .collect();
    let counts = (0..rows)
        .map(|r| {
            (0..cols)
                .map(|c| {
                    let stats = grid.cell(r, c)?;
                    let conf = match stats.confidence {
                        Confidence::High => export_sheet::Confidence::High,
                        Confidence::Medium => export_sheet::Confidence::Medium,
                        Confidence::Low => export_sheet::Confidence::Low,
                        Confidence::Empty => return None,
                    };
                    Some((stats.count, conf))
                })
                .collect()
        })
        .collect();

    let (filled, high) = grid.coverage();
    let logs: Vec<&str> = acc.logs.iter().map(|l| l.name.as_str()).collect();
    let mut coverage = format!(
        "{}/{} cells \u{00B7} {} high confidence",
        group_thousands(filled),
        group_thousands(rows * cols),
        high
    );
    if opts.exclude_low {
        coverage.push_str(" \u{00B7} low blank");
    }
    // The scale uses the cells' decimals, which follow the delay unit (cycles have 2).
    let decimals = cells
        .iter()
        .flatten()
        .flatten()
        .map(|&(_, d)| d)
        .next()
        .unwrap_or(measure.decimals);
    let (scale_min, scale_max) = match kind {
        MapKind::Diverging => (
            format!("\u{2212}{span:.decimals$}"),
            format!("+{span:.decimals$}"),
        ),
        MapKind::Sequential => (format!("{lo:.decimals$}"), format!("{hi:.decimals$}")),
    };
    let scale_title = if unit.is_empty() {
        measure.label.to_string()
    } else {
        format!("{} ({unit})", measure.label)
    };
    Ok(Sheet {
        title: title.to_string(),
        meta: vec![logs.join(", "), coverage],
        body: Body::Grids(vec![GridModel {
            caption: None,
            x_title: grid.x_axis.header(),
            y_title: grid.y_axis.header(),
            x_ticks: edge_ticks(&grid.x_axis.edges),
            y_ticks: edge_ticks(&grid.y_axis.edges),
            norm,
            labels,
            counts: Some(counts),
            kind,
            scale_title,
            scale_min,
            scale_max,
            rich_lean: measure.lean_is_positive(),
            dense: false,
        }]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::tables::export::DelayUnit;
    use crate::analysis::tables::{AxisSpec, MeasureSpec, RunReport, TableEvent};
    use crate::ui::heatmap;
    use crate::ui::theme::{ExportTheme, export_tokens};

    fn synthetic_accumulator() -> TableAccumulator {
        let x = AxisSpec::new("RPM", "rpm", vec![1000.0, 2000.0, 3000.0, 4000.0]);
        let y = AxisSpec::new("MAP", "kPa", vec![30.0, 50.0, 70.0]);
        let measures = vec![MeasureSpec {
            key: "dead_time",
            label: "Dead time",
            unit: "ms",
            decimals: 0,
        }];
        let mut acc = TableAccumulator::new(GeneratorKind::LambdaDelay, (x, y), measures);
        let events: Vec<TableEvent> = (0..12)
            .map(|i| TableEvent {
                log_id: 1,
                log_name: "synthetic.csv".into(),
                time: i as f64,
                rpm: 1500.0 + (i % 3) as f64 * 1000.0,
                axis_value: 40.0 + (i % 2) as f64 * 20.0,
                values: vec![100.0 + i as f64 * 5.0],
                quality: 1.0,
                reject: None,
                note: String::new(),
            })
            .collect();
        let report = RunReport {
            log_name: "synthetic.csv".into(),
            candidates: 12,
            accepted: 12,
            ..Default::default()
        };
        acc.add_log(1, "synthetic.csv", events, report);
        acc
    }

    /// `EXPORT_SHEET_DUMP=<dir> cargo test --lib dump_table_sheet -- --ignored` writes a sample
    /// table sheet for a visual check.
    #[test]
    #[ignore]
    fn dump_table_sheet() {
        let dir = std::env::var("EXPORT_SHEET_DUMP").expect("EXPORT_SHEET_DUMP");
        let rpm: Vec<f64> = (0..=10).map(|i| 1000.0 + i as f64 * 500.0).collect();
        let map: Vec<f64> = (0..=6).map(|i| 20.0 + i as f64 * 15.0).collect();
        let x = AxisSpec::new("RPM", "rpm", rpm);
        let y = AxisSpec::new("MAP", "kPa", map);
        let measures = vec![MeasureSpec {
            key: "dead_time",
            label: "Dead time",
            unit: "ms",
            decimals: 0,
        }];
        let mut acc = TableAccumulator::new(GeneratorKind::LambdaDelay, (x, y), measures);
        let mut events = Vec::new();
        for c in 0..10 {
            for r in 0..6 {
                // Fewer events at high load and high rpm; a corner stays empty.
                let n = (12usize).saturating_sub(c + 2 * r);
                for k in 0..n {
                    events.push(TableEvent {
                        log_id: 1,
                        log_name: "pull.csv".into(),
                        time: (c * 100 + r * 10 + k) as f64,
                        rpm: 1250.0 + c as f64 * 500.0,
                        axis_value: 27.0 + r as f64 * 15.0,
                        values: vec![80.0 + c as f64 * 12.0 - r as f64 * 6.0 + k as f64],
                        quality: 1.0,
                        reject: None,
                        note: String::new(),
                    });
                }
            }
        }
        acc.add_log(1, "pull.csv", events, RunReport::default());
        let sheet = table_sheet(&acc, 0, &ExportOptions::default(), "Lambda Delay").unwrap();
        for theme in [ExportTheme::Paper, ExportTheme::Graphite] {
            let name = format!("{theme:?}").to_lowercase();
            export_sheet::render_png(&sheet, theme)
                .save(format!("{dir}/table-{name}.png"))
                .unwrap();
            std::fs::write(
                format!("{dir}/table-{name}.pdf"),
                export_sheet::render_pdf(&sheet, theme),
            )
            .unwrap();
        }
    }

    #[test]
    fn sheet_renders_as_png_and_pdf() {
        let acc = synthetic_accumulator();
        let sheet = table_sheet(&acc, 0, &ExportOptions::default(), "Lambda Delay").unwrap();
        let img = export_sheet::render_png(&sheet, ExportTheme::Paper);
        assert_eq!((img.width(), img.height()), (1920, 1080));
        let pdf = export_sheet::render_pdf(&sheet, ExportTheme::Graphite);
        assert!(pdf.starts_with(b"%PDF"));
        assert!(pdf.len() > 1000);
    }

    #[test]
    fn png_draws_the_highest_row_at_the_top() {
        // Row 0 (MAP 30-50) holds every event's low value; row 1 is empty.
        // On screen the highest Y bin is at the top, so the PNG must put
        // row 0's map fill in the bottom band and dark grey in the top.
        let x = AxisSpec::new("RPM", "rpm", vec![1000.0, 2000.0]);
        let y = AxisSpec::new("MAP", "kPa", vec![30.0, 50.0, 70.0]);
        let measures = vec![MeasureSpec {
            key: "dead_time",
            label: "Dead time",
            unit: "ms",
            decimals: 0,
        }];
        let mut acc = TableAccumulator::new(GeneratorKind::LambdaDelay, (x, y), measures);
        let events: Vec<TableEvent> = (0..10)
            .map(|i| TableEvent {
                log_id: 1,
                log_name: "s".into(),
                time: i as f64,
                rpm: 1500.0,
                axis_value: 40.0,
                values: vec![100.0],
                quality: 1.0,
                reject: None,
                note: String::new(),
            })
            .collect();
        acc.add_log(1, "s", events, RunReport::default());
        let sheet = table_sheet(&acc, 0, &ExportOptions::default(), "t").unwrap();
        let img = export_sheet::render_png_body(&sheet, ExportTheme::Paper);
        // Body layout: plot (48, 24)-(948, 344) pt at 2 px/pt, two rows of 160 pt. Sample the
        // bottom-left corner of each cell, clear of the value label and count badge.
        let fill = heatmap::fill(&export_tokens(ExportTheme::Paper), MapKind::Sequential, 0.5);
        let px = |x: f32, y: f32| img.get_pixel((x * 2.0) as u32, (y * 2.0) as u32).0;
        let bottom = px(52.0, 340.0);
        let top = px(52.0, 180.0);
        assert_eq!(
            &bottom[..3],
            &[fill.r(), fill.g(), fill.b()],
            "filled row 0 is at the bottom"
        );
        assert_ne!(
            &top[..3],
            &[fill.r(), fill.g(), fill.b()],
            "empty row 1 is at the top"
        );
    }

    #[test]
    fn resolve_cells_honours_export_options() {
        let acc = synthetic_accumulator();
        // Every cell in the fixture has 2 samples -> Low, so blanking low
        // cells empties the table and the colour range falls back to 0..1.
        let blank = ExportOptions {
            exclude_low: true,
            ..ExportOptions::default()
        };
        let (cells, lo, hi) = resolve_cells(&acc, 0, &blank).unwrap();
        assert!(cells.iter().flatten().all(Option::is_none));
        assert_eq!((lo, hi), (0.0, 1.0));

        // Unit conversion goes through the same path CSV uses.
        let ms = ExportOptions {
            exclude_low: false,
            ..ExportOptions::default()
        };
        let cycles = ExportOptions {
            exclude_low: false,
            delay_unit: DelayUnit::EngineCycles,
            ..ExportOptions::default()
        };
        let (a, _, _) = resolve_cells(&acc, 0, &ms).unwrap();
        let (b, _, _) = resolve_cells(&acc, 0, &cycles).unwrap();
        let (va, _) = a[0][0].unwrap();
        let (vb, _) = b[0][0].unwrap();
        // 1000-2000 rpm cell centre is 1500 rpm: cycles = ms * rpm / 120000.
        assert!((vb - va * 1500.0 / 120_000.0).abs() < 1e-9);

        // A measure index past the list is an error, not a panic.
        assert!(resolve_cells(&acc, 5, &ms).is_err());
    }

    #[test]
    fn empty_table_is_an_error_not_a_panic() {
        let x = AxisSpec::new("RPM", "rpm", vec![1000.0, 2000.0]);
        let y = AxisSpec::new("MAP", "kPa", vec![30.0, 50.0]);
        let acc = TableAccumulator::new(GeneratorKind::LambdaDelay, (x, y), vec![]);
        assert!(table_sheet(&acc, 0, &ExportOptions::default(), "t").is_err());
    }

    #[test]
    fn badges_carry_count_and_confidence() {
        let acc = synthetic_accumulator();
        let sheet = table_sheet(&acc, 0, &ExportOptions::default(), "t").unwrap();
        let Body::Grids(grids) = sheet.body else {
            panic!("a table exports as a grid");
        };
        let counts = grids[0].counts.as_ref().unwrap();
        // Every fixture cell holds 2 events, which is Low confidence.
        assert_eq!(counts[0][0], Some((2, export_sheet::Confidence::Low)));
        assert_eq!(grids[0].x_ticks.len(), 4, "one label per edge");
    }
}
