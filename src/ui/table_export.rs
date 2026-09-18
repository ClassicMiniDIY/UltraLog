//! PNG and PDF export for generated tuning tables (lambda delay, accel
//! enrichment).
//!
//! The renderers are free functions over a [`TableAccumulator`] so they are
//! unit-testable without an `UltraLogApp`. Cell values, blanking of
//! low-confidence cells, and delay-unit conversion all go through
//! `analysis::tables::export::cell_value`, the same path CSV export uses, so
//! the four exports never disagree.
//!
//! PDF draws values and axis labels with Helvetica. PNG follows the histogram
//! precedent: this crate has no font rasterizer, so PNG is cells and grid
//! lines only. CSV and clipboard remain the numeric exports.

use std::path::Path;

use ::image::{Rgba, RgbaImage};
use printpdf::*;
use rust_i18n::t;

use crate::analysis::tables::binning::Confidence;
use crate::analysis::tables::export::{ExportOptions, cell_value, fmt_edge};
use crate::analysis::tables::{GeneratorKind, TableAccumulator};
use crate::analytics;
use crate::app::UltraLogApp;
use crate::colormap::{Colormap, sample};
use crate::ui::export::{draw_line, push_closed_line, push_filled_rect, push_text};

/// Colour for a cell's normalized value.
fn cell_rgb(t: f64) -> [u8; 3] {
    let c = sample(Colormap::Viridis, t as f32);
    [c.r(), c.g(), c.b()]
}

/// Black or white text for a Viridis background (WCAG relative luminance).
fn text_on(rgb: [u8; 3]) -> bool {
    let lin = |c: u8| {
        let v = c as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let lum = 0.2126 * lin(rgb[0]) + 0.7152 * lin(rgb[1]) + 0.0722 * lin(rgb[2]);
    lum > 0.4
}

/// `(value, decimals)` per cell after export options; `None` renders blank.
type ResolvedCells = Vec<Vec<Option<(f64, usize)>>>;

/// Resolve every cell to `(value, decimals)` after export options, plus the
/// value range used for colouring.
fn resolve_cells(
    acc: &TableAccumulator,
    measure_idx: usize,
    opts: &ExportOptions,
) -> Result<(ResolvedCells, f64, f64), Box<dyn std::error::Error>> {
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

/// Write the table as a 1920x1080 PNG: Viridis cells, grid lines, no text.
pub fn render_table_png(
    acc: &TableAccumulator,
    measure_idx: usize,
    opts: &ExportOptions,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let (cells, lo, hi) = resolve_cells(acc, measure_idx, opts)?;
    let grid = acc.grid(measure_idx);
    let (rows, cols) = (grid.rows(), grid.cols());

    let width = 1920u32;
    let height = 1080u32;
    let margin = 80u32;
    let left = margin;
    let right = width - margin;
    let top = margin;
    let bottom = height - margin;
    let cell_w = (right - left) as f64 / cols as f64;
    let cell_h = (bottom - top) as f64 / rows as f64;

    let mut img = RgbaImage::new(width, height);
    for px in img.pixels_mut() {
        *px = Rgba([30, 30, 30, 255]);
    }

    for (r, row) in cells.iter().enumerate() {
        // Highest Y bin at the top, the way ECU tables are laid out.
        let draw_row = rows - 1 - r;
        let y0 = top as f64 + draw_row as f64 * cell_h;
        for (c, cell) in row.iter().enumerate() {
            let x0 = left as f64 + c as f64 * cell_w;
            let rgb = match cell {
                Some((v, _)) => cell_rgb(normalize(*v, lo, hi)),
                None => [36, 36, 36],
            };
            let px = Rgba([rgb[0], rgb[1], rgb[2], 255]);
            let x_end = (x0 + cell_w).min(right as f64) as u32;
            let y_end = (y0 + cell_h).min(bottom as f64) as u32;
            for y in y0 as u32..y_end {
                for x in x0 as u32..x_end {
                    img.put_pixel(x, y, px);
                }
            }
        }
    }

    let line = Rgba([60, 60, 60, 255]);
    for c in 0..=cols {
        let x = (left as f64 + c as f64 * cell_w).min(right as f64) as u32;
        draw_line(&mut img, x, top, x, bottom, line);
    }
    for r in 0..=rows {
        let y = (top as f64 + r as f64 * cell_h).min(bottom as f64) as u32;
        draw_line(&mut img, left, y, right, y, line);
    }
    let border = Rgba([120, 120, 120, 255]);
    draw_line(&mut img, left, top, right, top, border);
    draw_line(&mut img, left, bottom, right, bottom, border);
    draw_line(&mut img, left, top, left, bottom, border);
    draw_line(&mut img, right, top, right, bottom, border);

    img.save(path)?;
    Ok(())
}

/// Write the table as an A4-landscape PDF with values and axis labels.
pub fn render_table_pdf(
    acc: &TableAccumulator,
    measure_idx: usize,
    opts: &ExportOptions,
    title: &str,
    generated: &str,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let (cells, lo, hi) = resolve_cells(acc, measure_idx, opts)?;
    let grid = acc.grid(measure_idx);
    let (rows, cols) = (grid.rows(), grid.cols());
    let measure = &acc.measures[measure_idx];

    let font_bold = PdfFontHandle::Builtin(BuiltinFont::HelveticaBold);
    let font_regular = PdfFontHandle::Builtin(BuiltinFont::Helvetica);
    let mut ops: Vec<Op> = Vec::new();

    // A4 landscape, mm.
    let margin: f64 = 15.0;
    let label_w: f64 = 22.0;
    let label_h: f64 = 8.0;
    let chart_left = margin + label_w;
    let chart_right: f64 = 297.0 - margin;
    let chart_top: f64 = 210.0 - margin - 26.0;
    let chart_bottom = margin + label_h + 6.0;
    let cell_w = (chart_right - chart_left) / cols as f64;
    let cell_h = (chart_top - chart_bottom) / rows as f64;

    let black = Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None));
    let white = Color::Rgb(Rgb::new(1.0, 1.0, 1.0, None));

    ops.push(Op::SetFillColor { col: black.clone() });
    push_text(
        &mut ops,
        title,
        16.0,
        Mm(margin as f32),
        Mm(200.0),
        &font_bold,
    );
    let unit = if acc.generator == GeneratorKind::LambdaDelay && measure.unit == "ms" {
        opts.delay_unit.label().to_string()
    } else {
        measure.unit.to_string()
    };
    let logs: Vec<&str> = acc.logs.iter().map(|l| l.name.as_str()).collect();
    let subtitle = format!(
        "{} ({}) | {} | Logs: {}",
        measure.label,
        unit,
        generated,
        logs.join(", ")
    );
    push_text(
        &mut ops,
        &subtitle,
        9.0,
        Mm(margin as f32),
        Mm(193.0),
        &font_regular,
    );
    let (filled, high) = grid.coverage();
    let coverage = format!(
        "Rows: {} | Columns: {} | {}/{} cells filled, {} high confidence{}",
        grid.y_axis.header(),
        grid.x_axis.header(),
        filled,
        rows * cols,
        high,
        if opts.exclude_low {
            " | low-confidence cells blank"
        } else {
            ""
        }
    );
    push_text(
        &mut ops,
        &coverage,
        8.0,
        Mm(margin as f32),
        Mm(188.0),
        &font_regular,
    );

    // Cells.
    let value_size = (cell_h as f32 * 1.6).clamp(5.0, 11.0);
    let count_size = (value_size * 0.6).max(4.0);
    for (r, row) in cells.iter().enumerate() {
        let y = chart_bottom + r as f64 * cell_h;
        for (c, cell) in row.iter().enumerate() {
            let x = chart_left + c as f64 * cell_w;
            let Some((v, decimals)) = cell else {
                ops.push(Op::SetFillColor {
                    col: Color::Rgb(Rgb::new(0.93, 0.93, 0.93, None)),
                });
                push_filled_rect(&mut ops, x as f32, y as f32, cell_w as f32, cell_h as f32);
                continue;
            };
            let rgb = cell_rgb(normalize(*v, lo, hi));
            ops.push(Op::SetFillColor {
                col: Color::Rgb(Rgb::new(
                    rgb[0] as f32 / 255.0,
                    rgb[1] as f32 / 255.0,
                    rgb[2] as f32 / 255.0,
                    None,
                )),
            });
            push_filled_rect(&mut ops, x as f32, y as f32, cell_w as f32, cell_h as f32);

            let fg = if text_on(rgb) {
                black.clone()
            } else {
                white.clone()
            };
            ops.push(Op::SetFillColor { col: fg });
            let text = format!("{v:.decimals$}");
            // Helvetica averages ~0.5em per glyph; centre approximately.
            let text_w_mm = text.len() as f64 * value_size as f64 * 0.5 * 0.3528;
            push_text(
                &mut ops,
                &text,
                value_size,
                Mm((x + (cell_w - text_w_mm) / 2.0).max(x + 0.5) as f32),
                Mm((y + cell_h / 2.0 - value_size as f64 * 0.12) as f32),
                &font_regular,
            );
            if let Some(stats) = grid.cell(r, c) {
                // Built-in Helvetica maps `~` to an arrow glyph, so the
                // markers stay in plain ASCII: `*` medium, `?` low.
                let mark = match stats.confidence {
                    Confidence::High | Confidence::Empty => "",
                    Confidence::Medium => "*",
                    Confidence::Low => "?",
                };
                push_text(
                    &mut ops,
                    &format!("{}{}", stats.count, mark),
                    count_size,
                    Mm((x + 0.6) as f32),
                    Mm((y + cell_h - count_size as f64 * 0.42) as f32),
                    &font_regular,
                );
            }
        }
    }

    // Grid lines.
    ops.push(Op::SetOutlineColor {
        col: Color::Rgb(Rgb::new(0.5, 0.5, 0.5, None)),
    });
    ops.push(Op::SetOutlineThickness { pt: Pt(0.25) });
    for c in 0..=cols {
        let x = chart_left + c as f64 * cell_w;
        push_closed_line(
            &mut ops,
            &[
                (x as f32, chart_bottom as f32),
                (x as f32, chart_top as f32),
            ],
        );
    }
    for r in 0..=rows {
        let y = chart_bottom + r as f64 * cell_h;
        push_closed_line(
            &mut ops,
            &[
                (chart_left as f32, y as f32),
                (chart_right as f32, y as f32),
            ],
        );
    }

    // Axis labels: lower edges, X along the bottom, Y down the left.
    ops.push(Op::SetFillColor { col: black });
    let axis_size = (cell_w as f32 * 0.9).clamp(5.0, 8.0);
    for c in 0..cols {
        let x = chart_left + c as f64 * cell_w;
        push_text(
            &mut ops,
            &fmt_edge(grid.x_axis.lower_edge(c)),
            axis_size,
            Mm((x + 0.6) as f32),
            Mm((chart_bottom - 4.0) as f32),
            &font_regular,
        );
    }
    for r in 0..rows {
        let y = chart_bottom + r as f64 * cell_h;
        push_text(
            &mut ops,
            &fmt_edge(grid.y_axis.lower_edge(r)),
            axis_size,
            Mm(margin as f32),
            Mm((y + cell_h / 2.0 - 1.0) as f32),
            &font_regular,
        );
    }
    push_text(
        &mut ops,
        &grid.x_axis.header(),
        8.0,
        Mm(chart_left as f32),
        Mm((chart_bottom - 10.0) as f32),
        &font_bold,
    );
    push_text(
        &mut ops,
        &grid.y_axis.header(),
        8.0,
        Mm(margin as f32),
        Mm((chart_top + 2.0) as f32),
        &font_bold,
    );

    let page = PdfPage::new(Mm(297.0), Mm(210.0), ops);
    let mut doc = PdfDocument::new(title);
    doc.with_pages(vec![page]);
    let mut warnings = Vec::new();
    let bytes = doc.save(&PdfSaveOptions::default(), &mut warnings);
    std::fs::write(path, bytes)?;
    Ok(())
}

impl UltraLogApp {
    /// Export the active table tool's table as PNG.
    pub fn export_table_png(&mut self) {
        let Some(kind) = self.active_tool.generator_kind() else {
            return;
        };
        if self.table_generator.table(kind).is_none() {
            self.show_toast_error(&t!("table_gen.no_events"));
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .set_file_name(format!("ultralog_{}.png", kind.id()))
            .save_file()
        else {
            return;
        };
        let result = {
            let Some(acc) = self.table_generator.table(kind) else {
                return;
            };
            let measure_idx = self.table_generator.measure_index(kind);
            render_table_png(acc, measure_idx, &self.table_generator.export, &path)
        };
        match result {
            Ok(()) => {
                analytics::track_export(&format!("{}_png", kind.id()));
                self.show_toast_success(&t!("toast.table_exported_png"));
            }
            Err(e) => self.show_toast_error(&t!("toast.export_failed", error = e.to_string())),
        }
    }

    /// Export the active table tool's table as PDF.
    pub fn export_table_pdf(&mut self) {
        let Some(kind) = self.active_tool.generator_kind() else {
            return;
        };
        if self.table_generator.table(kind).is_none() {
            self.show_toast_error(&t!("table_gen.no_events"));
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF Document", &["pdf"])
            .set_file_name(format!("ultralog_{}.pdf", kind.id()))
            .save_file()
        else {
            return;
        };
        let result = {
            let Some(acc) = self.table_generator.table(kind) else {
                return;
            };
            let measure_idx = self.table_generator.measure_index(kind);
            let title = format!("UltraLog {}", self.active_tool.name());
            let generated = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
            render_table_pdf(
                acc,
                measure_idx,
                &self.table_generator.export,
                &title,
                &generated,
                &path,
            )
        };
        match result {
            Ok(()) => {
                analytics::track_export(&format!("{}_pdf", kind.id()));
                self.show_toast_success(&t!("toast.table_exported_pdf"));
            }
            Err(e) => self.show_toast_error(&t!("toast.export_failed", error = e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::tables::export::DelayUnit;
    use crate::analysis::tables::{AxisSpec, MeasureSpec, RunReport, TableEvent};

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

    #[test]
    fn png_writes_a_valid_image() {
        let acc = synthetic_accumulator();
        let dir = std::env::temp_dir().join(format!("ultralog_table_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("table.png");
        render_table_png(&acc, 0, &ExportOptions::default(), &path).unwrap();
        let img = ::image::open(&path).unwrap();
        assert_eq!(img.width(), 1920);
        assert_eq!(img.height(), 1080);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn pdf_writes_a_document_with_values() {
        let acc = synthetic_accumulator();
        let dir = std::env::temp_dir().join(format!("ultralog_table_pdf_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("table.pdf");
        render_table_pdf(
            &acc,
            0,
            &ExportOptions::default(),
            "UltraLog Lambda Delay",
            "2026-09-18 10:00",
            &path,
        )
        .unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"%PDF"));
        assert!(bytes.len() > 1000);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn png_draws_the_highest_row_at_the_top() {
        // Row 0 (MAP 30-50) holds every event's low value; row 1 is empty.
        // On screen the highest Y bin is at the top, so the PNG must put
        // row 0's Viridis fill in the bottom band and dark grey in the top.
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
        let dir = std::env::temp_dir().join(format!("ultralog_row_order_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rows.png");
        render_table_png(&acc, 0, &ExportOptions::default(), &path).unwrap();
        let img = ::image::open(&path).unwrap().to_rgba8();
        let top = img.get_pixel(960, 80 + 200);
        let bottom = img.get_pixel(960, 1080 - 80 - 200);
        assert_eq!(top.0, [36, 36, 36, 255], "empty row 1 must be at the top");
        assert_ne!(
            bottom.0,
            [36, 36, 36, 255],
            "filled row 0 must be at the bottom"
        );
        std::fs::remove_dir_all(&dir).ok();
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
        let path = std::env::temp_dir().join("ultralog_never_written.png");
        assert!(render_table_png(&acc, 0, &ExportOptions::default(), &path).is_err());
    }

    #[test]
    fn text_colour_flips_on_bright_viridis() {
        assert!(!text_on(cell_rgb(0.0)));
        assert!(text_on(cell_rgb(1.0)));
    }
}
