//! The ExportSheet (`design/design-system/4-exports.md`,
//! `design/design-system/components/ExportSheet/README.md`).
//!
//! Every PNG and PDF export is the same sheet: the sunset redline across the top edge, the
//! lockup, the title, the time range and counts right-aligned in mono, the chart or table, its
//! legend or ColorScale, and a footer (ultralog.co · version · date). Exports use an export
//! theme (Paper or Graphite), always the color-blind safe palette, and no intensity effects.
//!
//! Callers build a [`Sheet`] from app state (`ui/export.rs`, `ui/table_export.rs`); this module
//! lays it out once on a [`Canvas`], and the PNG / PDF back ends turn that into files.

pub mod canvas;
pub mod pdf;
pub mod png;
pub mod text;

use std::sync::OnceLock;

use ::image::RgbaImage;
use eframe::egui::{Color32, Pos2, Rect, pos2, vec2};
use rust_i18n::t;

use crate::ui::heatmap::{self, MapKind};
use crate::ui::theme::{self, ExportTheme, SeriesStroke, Tokens, export_tokens};
use canvas::{Canvas, Face, HAlign, flatten};

/// PNG sheet size in points (rendered at 2×: 1920 × 1080 px).
pub const PNG_SIZE: (f32, f32) = (960.0, 540.0);
/// PNG pixels per point.
pub const PNG_SCALE: f32 = 2.0;
/// PDF page: A4 landscape, in points.
pub const PDF_SIZE: (f32, f32) = (842.0, 595.0);

/// One line series, normalized to its own range for display.
pub struct SeriesModel {
    pub name: String,
    pub unit: String,
    /// Colour slot; the export palette is `theme::series(.., true)`.
    pub color_index: usize,
    /// `(time, value 0..=1)` within the sheet's time range.
    pub points: Vec<[f64; 2]>,
}

pub struct ChartModel {
    pub series: Vec<SeriesModel>,
    pub t0: f64,
    pub t1: f64,
}

/// How sure a table cell is (count badge fill style).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

/// Event count and confidence per table cell, `[row][col]`.
pub type CountGrid = Vec<Vec<Option<(usize, Confidence)>>>;

/// A heatmap or table. Row 0 is the lowest Y bin and is drawn at the bottom.
pub struct GridModel {
    /// Shown above the grid when several grids share a sheet.
    pub caption: Option<String>,
    pub x_title: String,
    pub y_title: String,
    /// Axis labels at positions 0..=1 along each axis (0 = left / bottom), already formatted.
    /// Labels that would overlap the previous one are skipped.
    pub x_ticks: Vec<(f32, String)>,
    pub y_ticks: Vec<(f32, String)>,
    /// Colour position 0..=1 per cell; `None` is empty (hatched).
    pub norm: Vec<Vec<Option<f32>>>,
    /// Value text per cell, if it should print one.
    pub labels: Vec<Vec<Option<String>>>,
    /// Event count and confidence per cell (tables only).
    pub counts: Option<CountGrid>,
    pub kind: MapKind,
    pub scale_title: String,
    pub scale_min: String,
    pub scale_max: String,
    /// A density map with many small cells (scatter plots): no cell gaps, labels or hatching,
    /// and empty cells show the plot ground.
    pub dense: bool,
}

/// The file type the ExportDialog writes, remembered in `UserSettings`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExportFormat {
    #[default]
    Pdf,
    Png,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Pdf => "pdf",
            ExportFormat::Png => "png",
        }
    }
}

/// The ExportDialog's remembered choices: the last format, and the last theme per format.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ExportChoice {
    pub format: ExportFormat,
    pub pdf_theme: ExportTheme,
    pub png_theme: ExportTheme,
}

impl ExportChoice {
    /// The theme remembered for the current format.
    pub fn theme(&self) -> ExportTheme {
        match self.format {
            ExportFormat::Pdf => self.pdf_theme,
            ExportFormat::Png => self.png_theme,
        }
    }

    pub fn theme_mut(&mut self) -> &mut ExportTheme {
        match self.format {
            ExportFormat::Pdf => &mut self.pdf_theme,
            ExportFormat::Png => &mut self.png_theme,
        }
    }
}

pub enum Body {
    Chart(ChartModel),
    Grids(Vec<GridModel>),
}

pub struct Sheet {
    pub title: String,
    /// Right-aligned mono lines: time range, record/channel counts.
    pub meta: Vec<String>,
    pub body: Body,
}

impl Sheet {
    fn footer_right() -> String {
        format!(
            "v{} \u{00B7} exported {}",
            env!("CARGO_PKG_VERSION"),
            chrono::Local::now().format("%Y-%m-%d")
        )
    }
}

/// The pre-rasterized lockup for `theme` (dark grounds: Graphite and the About window).
pub(crate) fn lockup(theme: ExportTheme) -> &'static RgbaImage {
    static PAPER: OnceLock<RgbaImage> = OnceLock::new();
    static DARK: OnceLock<RgbaImage> = OnceLock::new();
    let (cell, bytes): (&OnceLock<RgbaImage>, &[u8]) = match theme {
        ExportTheme::Paper => (
            &PAPER,
            include_bytes!("../../../assets/brand/ultralog-lockup-paper.png"),
        ),
        ExportTheme::Graphite => (
            &DARK,
            include_bytes!("../../../assets/brand/ultralog-lockup.png"),
        ),
    };
    cell.get_or_init(|| {
        ::image::load_from_memory(bytes)
            .map(|i| i.to_rgba8())
            .unwrap_or_else(|_| RgbaImage::new(1, 1))
    })
}

/// Render the full sheet as PNG pixels (1920 × 1080).
pub fn render_png(sheet: &Sheet, theme: ExportTheme) -> RgbaImage {
    let (w, h) = PNG_SIZE;
    let mut c = png::PngCanvas::new(w, h, PNG_SCALE);
    draw_sheet(
        &mut c,
        sheet,
        theme,
        Rect::from_min_size(Pos2::ZERO, vec2(w, h)),
        false,
    );
    c.finish()
}

/// Render only the chart or table (no header, legend or footer) for pasting into a document.
pub fn render_png_body(sheet: &Sheet, theme: ExportTheme) -> RgbaImage {
    let (w, h) = (PNG_SIZE.0, PNG_SIZE.1 * 0.7);
    let t = export_tokens(theme);
    let mut c = png::PngCanvas::new(w, h, PNG_SCALE);
    let page = Rect::from_min_size(Pos2::ZERO, vec2(w, h));
    c.fill_rect(page, t.bg_base);
    draw_body(&mut c, &sheet.body, &t, page.shrink(12.0));
    c.finish()
}

/// Render the full sheet as a one-page A4 landscape PDF.
pub fn render_pdf(sheet: &Sheet, theme: ExportTheme) -> Vec<u8> {
    let (w, h) = PDF_SIZE;
    let mut c = pdf::PdfCanvas::new(&sheet.title, w, h);
    draw_sheet(
        &mut c,
        sheet,
        theme,
        Rect::from_min_size(Pos2::ZERO, vec2(w, h)),
        true,
    );
    c.finish()
}

/// Redline colour at `k` (cyan → violet → magenta → orange → gold at 0 / 30 / 55 / 80 / 100 %).
fn redline_at(t: &Tokens, k: f32) -> Color32 {
    let stops = [
        (0.0, t.redline_0),
        (0.30, t.redline_1),
        (0.55, t.redline_2),
        (0.80, t.redline_3),
        (1.0, t.redline_4),
    ];
    for pair in stops.windows(2) {
        let ((k0, c0), (k1, c1)) = (pair[0], pair[1]);
        if k <= k1 {
            return theme::lerp_oklab(c0, c1, ((k - k0) / (k1 - k0)).clamp(0.0, 1.0));
        }
    }
    t.redline_4
}

fn draw_sheet(c: &mut dyn Canvas, sheet: &Sheet, theme: ExportTheme, page: Rect, pdf: bool) {
    let t = export_tokens(theme);
    // Graphite PDFs keep a white margin so a printer never floods the sheet edge to edge.
    let area = if pdf && theme == ExportTheme::Graphite {
        c.fill_rect(page, export_tokens(ExportTheme::Paper).bg_base);
        page.shrink(18.0)
    } else {
        page
    };
    c.fill_rect(area, t.bg_base);
    c.gradient_rect(
        Rect::from_min_size(area.left_top(), vec2(area.width(), 3.0)),
        &|k| redline_at(&t, k),
    );

    let content = Rect::from_min_max(
        area.left_top() + vec2(30.0, 26.0),
        area.right_bottom() - vec2(30.0, 22.0),
    );

    // Lockup, title and meta
    let logo = lockup(theme);
    let logo_h = 20.0;
    let logo_w = logo_h * logo.width() as f32 / logo.height().max(1) as f32;
    c.image(
        Rect::from_min_size(content.left_top(), vec2(logo_w, logo_h)),
        logo,
    );
    let title_base = content.top() + logo_h + 28.0;
    c.text(
        pos2(content.left(), title_base),
        &sheet.title,
        Face::Display,
        22.0,
        t.ink,
        HAlign::Left,
    );
    for (i, line) in sheet.meta.iter().enumerate() {
        c.text(
            pos2(content.right(), title_base - 14.0 + 12.0 * i as f32),
            line,
            Face::Mono,
            8.5,
            t.ink_subtle,
            HAlign::Right,
        );
    }

    // Footer
    c.text(
        pos2(content.left(), content.bottom()),
        "ultralog.co",
        Face::Mono,
        7.5,
        t.ink_subtle,
        HAlign::Left,
    );
    c.text(
        pos2(content.right(), content.bottom()),
        &Sheet::footer_right(),
        Face::Mono,
        7.5,
        t.ink_subtle,
        HAlign::Right,
    );

    // Body, then its legend or ColorScale
    let legend_h = 24.0;
    let body = Rect::from_min_max(
        pos2(content.left(), title_base + 18.0),
        pos2(content.right(), content.bottom() - 18.0 - legend_h),
    );
    draw_body(c, &sheet.body, &t, body);
    let legend_y = body.bottom() + 16.0;
    match &sheet.body {
        Body::Chart(chart) => draw_series_legend(c, chart, &t, pos2(body.left(), legend_y)),
        Body::Grids(grids) => {
            if let Some(g) = grids.first() {
                draw_color_scale(c, g, &t, pos2(body.left(), legend_y));
            }
        }
    }
}

fn draw_body(c: &mut dyn Canvas, body: &Body, t: &Tokens, rect: Rect) {
    match body {
        Body::Chart(chart) => draw_chart(c, chart, t, rect),
        Body::Grids(grids) => {
            let n = grids.len().max(1) as f32;
            let gap = 24.0;
            let w = (rect.width() - gap * (n - 1.0)) / n;
            for (i, g) in grids.iter().enumerate() {
                let r = Rect::from_min_size(
                    rect.left_top() + vec2((w + gap) * i as f32, 0.0),
                    vec2(w, rect.height()),
                );
                draw_grid(c, g, t, r);
            }
        }
    }
}

fn series_style(t: &Tokens, index: usize) -> (Color32, Option<[f32; 2]>) {
    let (color, stroke) = theme::series(t, index, true);
    let dash = match stroke {
        SeriesStroke::Solid => None,
        SeriesStroke::Dashed => Some([4.0, 2.5]),
        SeriesStroke::Dotted => Some([1.0, 2.0]),
    };
    (color, dash)
}

/// "Nice" tick spacing for `span` seconds aiming at about `count` ticks.
fn nice_step(span: f64, count: f64) -> f64 {
    let raw = (span / count).max(1e-9);
    let mag = 10f64.powf(raw.log10().floor());
    let norm = raw / mag;
    let nice = if norm < 1.5 {
        1.0
    } else if norm < 3.5 {
        2.0
    } else if norm < 7.5 {
        5.0
    } else {
        10.0
    };
    nice * mag
}

fn draw_chart(c: &mut dyn Canvas, chart: &ChartModel, t: &Tokens, rect: Rect) {
    let plot = Rect::from_min_max(rect.left_top(), pos2(rect.right(), rect.bottom() - 14.0));
    c.fill_rect(plot, t.plot_bg);
    let span = (chart.t1 - chart.t0).max(1e-9);
    let x_of = |time: f64| plot.left() + ((time - chart.t0) / span) as f32 * plot.width();
    let grid = flatten(t.plot_grid, t.plot_bg);

    // Grid and time ticks
    let step = nice_step(span, 8.0);
    let mut tick = (chart.t0 / step).ceil() * step;
    while tick <= chart.t1 + 1e-9 {
        let x = x_of(tick);
        c.polyline(
            &[pos2(x, plot.top()), pos2(x, plot.bottom())],
            0.4,
            grid,
            None,
        );
        c.text(
            pos2(x, rect.bottom()),
            &format_tick(tick, step),
            Face::Mono,
            7.0,
            t.plot_axis,
            HAlign::Center,
        );
        tick += step;
    }
    for i in 1..4 {
        let y = plot.top() + plot.height() * i as f32 / 4.0;
        c.polyline(
            &[pos2(plot.left(), y), pos2(plot.right(), y)],
            0.4,
            grid,
            None,
        );
    }

    for s in &chart.series {
        let (color, dash) = series_style(t, s.color_index);
        let pts: Vec<Pos2> = s
            .points
            .iter()
            .map(|p| pos2(x_of(p[0]), plot.bottom() - p[1] as f32 * plot.height()))
            .collect();
        c.polyline(&pts, 1.2, color, dash);
    }
    c.stroke_rect(plot, 0.6, flatten(t.plot_grid_major, t.plot_bg));
}

fn format_tick(seconds: f64, step: f64) -> String {
    if seconds >= 60.0 {
        let m = (seconds / 60.0).floor();
        let s = seconds - m * 60.0;
        if step >= 1.0 {
            format!("{}:{:02.0}", m, s)
        } else {
            format!("{}:{:06.3}", m, s)
        }
    } else if step >= 1.0 {
        format!("{seconds:.0}")
    } else {
        format!("{seconds:.1}")
    }
}

fn draw_series_legend(c: &mut dyn Canvas, chart: &ChartModel, t: &Tokens, origin: Pos2) {
    let mut x = origin.x;
    for s in &chart.series {
        let (color, dash) = series_style(t, s.color_index);
        let y = origin.y;
        c.polyline(
            &[pos2(x, y - 3.0), pos2(x + 12.0, y - 3.0)],
            1.6,
            color,
            dash,
        );
        x += 16.0;
        let label = if s.unit.is_empty() {
            s.name.clone()
        } else {
            format!("{} ({})", s.name, s.unit)
        };
        c.text(pos2(x, y), &label, Face::Ui, 8.5, t.ink_muted, HAlign::Left);
        x += text::width(&label, Face::Ui, 8.5) + 16.0;
    }
}

/// Diagonal hatch segments inside `rect` (45°, `step` apart).
fn hatch(c: &mut dyn Canvas, rect: Rect, color: Color32, step: f32) {
    let mut k = -rect.height();
    while k < rect.width() {
        // Line from (left + k, bottom) going up-right; clip to the rect.
        let x0 = rect.left() + k;
        let (mut a, mut b) = (
            pos2(x0, rect.bottom()),
            pos2(x0 + rect.height(), rect.top()),
        );
        if a.x < rect.left() {
            a = pos2(rect.left(), rect.bottom() - (rect.left() - a.x));
        }
        if b.x > rect.right() {
            b = pos2(rect.right(), rect.top() + (b.x - rect.right()));
        }
        if a.x < b.x {
            c.polyline(&[a, b], 0.3, color, None);
        }
        k += step;
    }
}

fn draw_grid(c: &mut dyn Canvas, g: &GridModel, t: &Tokens, rect: Rect) {
    let mut area = rect;
    if let Some(caption) = &g.caption {
        c.text(
            pos2(area.left(), area.top() + 8.0),
            caption,
            Face::UiSemibold,
            8.0,
            t.ink_muted,
            HAlign::Left,
        );
        area = Rect::from_min_max(area.left_top() + vec2(0.0, 14.0), area.right_bottom());
    }
    let rows = g.norm.len();
    let cols = g.norm.first().map_or(0, Vec::len);
    if rows == 0 || cols == 0 {
        return;
    }
    // Axis titles: Y above the row labels, X centred under the column labels.
    c.text(
        pos2(area.left(), area.top() + 7.0),
        &g.y_title,
        Face::UiSemibold,
        7.0,
        t.ink_muted,
        HAlign::Left,
    );
    let plot = Rect::from_min_max(
        pos2(area.left() + 36.0, area.top() + 12.0),
        pos2(area.right(), area.bottom() - 22.0),
    );
    c.text(
        pos2(plot.center().x, area.bottom()),
        &g.x_title,
        Face::UiSemibold,
        7.0,
        t.ink_muted,
        HAlign::Center,
    );
    let cw = plot.width() / cols as f32;
    let ch = plot.height() / rows as f32;
    let value_size = (ch * 0.45).clamp(4.5, 8.0);
    let badge_size = (ch * 0.28).clamp(3.5, 5.5);
    if g.dense {
        c.fill_rect(plot, t.plot_bg);
    }

    for r in 0..rows {
        let y = plot.bottom() - ch * (r + 1) as f32;
        for col in 0..cols {
            let full = Rect::from_min_size(pos2(plot.left() + cw * col as f32, y), vec2(cw, ch));
            if g.dense {
                if let Some(v) = g.norm[r][col] {
                    // Slight overlap so neighbouring cells leave no hairline seams.
                    c.fill_rect(full.expand(0.15), heatmap::fill(t, g.kind, v));
                }
                continue;
            }
            let cell = full.shrink(0.4);
            let Some(v) = g.norm[r][col] else {
                c.fill_rect(cell, t.cell_empty);
                hatch(c, cell, flatten(t.line_subtle, t.cell_empty), 3.0);
                continue;
            };
            let fill = heatmap::fill(t, g.kind, v);
            c.fill_rect(cell, fill);
            if let Some(Some(label)) = g.labels.get(r).and_then(|row| row.get(col))
                && cw > 16.0
                && ch > 9.0
            {
                let mut text_color = heatmap::text_on(t, g.kind, v);
                if t.cell_plate.a() > 0 {
                    let w = text::width(label, Face::Mono, value_size) + 3.0;
                    c.fill_rect(
                        Rect::from_center_size(cell.center(), vec2(w, value_size + 1.5)),
                        flatten(t.cell_plate, fill),
                    );
                    text_color = t.ink;
                }
                c.text(
                    pos2(cell.center().x, cell.center().y + value_size * 0.35),
                    label,
                    Face::Mono,
                    value_size,
                    text_color,
                    HAlign::Center,
                );
            }
            if let Some(Some((count, conf))) = g
                .counts
                .as_ref()
                .and_then(|rows| rows.get(r).and_then(|row| row.get(col)).copied())
                && cw > 20.0
            {
                draw_badge(c, t, cell, count, conf, badge_size);
            }
        }
    }
    if g.dense {
        c.stroke_rect(plot, 0.6, flatten(t.plot_grid_major, t.plot_bg));
    }

    // Axis labels, skipping any that would collide with the previous one.
    let mut last_right = f32::NEG_INFINITY;
    for (k, label) in &g.x_ticks {
        let x = plot.left() + k.clamp(0.0, 1.0) * plot.width();
        let w = text::width(label, Face::Mono, 6.5);
        if x - w / 2.0 < last_right + 4.0 {
            continue;
        }
        last_right = x + w / 2.0;
        c.text(
            pos2(x, plot.bottom() + 9.0),
            label,
            Face::Mono,
            6.5,
            t.plot_axis,
            HAlign::Center,
        );
    }
    let mut last_y = f32::INFINITY;
    for (k, label) in &g.y_ticks {
        let y = plot.bottom() - k.clamp(0.0, 1.0) * plot.height();
        if last_y - y < 9.0 {
            continue;
        }
        last_y = y;
        c.text(
            pos2(plot.left() - 4.0, y + 2.3),
            label,
            Face::Mono,
            6.5,
            t.plot_axis,
            HAlign::Right,
        );
    }
}

fn draw_badge(
    c: &mut dyn Canvas,
    t: &Tokens,
    cell: Rect,
    count: usize,
    conf: Confidence,
    size: f32,
) {
    let text = count.to_string();
    let w = text::width(&text, Face::Mono, size) + 2.0;
    let r = Rect::from_min_size(
        pos2(cell.right() - w - 0.8, cell.top() + 0.8),
        vec2(w, size + 1.2),
    );
    let base = pos2(r.center().x, r.bottom() - 1.0);
    match conf {
        Confidence::High => {
            c.fill_rect(r, t.ink);
            c.text(base, &text, Face::Mono, size, t.bg_base, HAlign::Center);
        }
        Confidence::Medium => {
            c.fill_rect(r, t.bg_base);
            c.stroke_rect(r, 0.4, t.ink);
            c.text(base, &text, Face::Mono, size, t.ink, HAlign::Center);
        }
        Confidence::Low => {
            c.fill_rect(r, t.bg_base);
            let pts = [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
                r.left_top(),
            ];
            c.polyline(&pts, 0.4, t.ink_muted, Some([0.8, 0.6]));
            c.text(base, &text, Face::Mono, size, t.ink_muted, HAlign::Center);
        }
    }
}

fn draw_color_scale(c: &mut dyn Canvas, g: &GridModel, t: &Tokens, origin: Pos2) {
    let title = g.scale_title.clone();
    c.text(
        origin,
        &title,
        Face::UiSemibold,
        7.0,
        t.ink_muted,
        HAlign::Left,
    );
    let mut x = origin.x + text::width(&title, Face::UiSemibold, 7.0) + 12.0;
    c.text(
        pos2(x, origin.y),
        &g.scale_min,
        Face::Mono,
        7.5,
        t.ink_muted,
        HAlign::Left,
    );
    x += text::width(&g.scale_min, Face::Mono, 7.5) + 6.0;
    let bar = Rect::from_min_size(pos2(x, origin.y - 6.0), vec2(150.0, 6.0));
    let kind = g.kind;
    c.gradient_rect(bar, &|k| heatmap::fill(t, kind, k));
    c.text(
        pos2(bar.right() + 6.0, origin.y),
        &g.scale_max,
        Face::Mono,
        7.5,
        t.ink_muted,
        HAlign::Left,
    );
    if kind == MapKind::Diverging {
        let y = origin.y + 8.0;
        c.text(
            pos2(bar.left(), y),
            &t!("chart.rich"),
            Face::Mono,
            6.0,
            t.ink_subtle,
            HAlign::Left,
        );
        c.text(
            pos2(bar.center().x, y),
            &t!("chart.target"),
            Face::Mono,
            6.0,
            t.ink_subtle,
            HAlign::Center,
        );
        c.text(
            pos2(bar.right(), y),
            &t!("chart.lean"),
            Face::Mono,
            6.0,
            t.ink_subtle,
            HAlign::Right,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart_sheet() -> Sheet {
        let series = (0..3)
            .map(|i| SeriesModel {
                name: format!("Channel {i}"),
                unit: "kPa".into(),
                color_index: i,
                points: (0..200)
                    .map(|k| {
                        let x = k as f64 / 10.0;
                        [x, 0.5 + 0.4 * ((x + i as f64).sin())]
                    })
                    .collect(),
            })
            .collect();
        Sheet {
            title: "2025-03-06 Haltech pull".into(),
            meta: vec![
                "0:00.000 \u{2013} 0:20.000".into(),
                "223\u{2009}559 records \u{00B7} 114 ch".into(),
            ],
            body: Body::Chart(ChartModel {
                series,
                t0: 0.0,
                t1: 20.0,
            }),
        }
    }

    #[test]
    fn png_sheet_is_2x_with_paper_ground_and_redline() {
        let img = render_png(&chart_sheet(), ExportTheme::Paper);
        assert_eq!((img.width(), img.height()), (1920, 1080));
        let paper = export_tokens(ExportTheme::Paper).bg_base;
        let px = img.get_pixel(1900, 1060);
        assert_eq!([px[0], px[1], px[2]], [paper.r(), paper.g(), paper.b()]);
        // The redline starts cyan-ish at the top-left edge.
        let top = img.get_pixel(4, 2);
        assert_ne!([top[0], top[1], top[2]], [paper.r(), paper.g(), paper.b()]);
    }

    #[test]
    fn pdf_sheet_embeds_fonts_and_parses() {
        let bytes = render_pdf(&chart_sheet(), ExportTheme::Graphite);
        assert!(bytes.starts_with(b"%PDF"));
        let text = String::from_utf8_lossy(&bytes);
        assert!(
            text.contains("FontFile2") || text.contains("FontFile"),
            "fonts are embedded"
        );
    }

    /// `EXPORT_SHEET_DUMP=<dir> cargo test --lib dump_sheets -- --ignored` writes sample sheets
    /// for a visual check.
    #[test]
    #[ignore]
    fn dump_sheets() {
        let dir = std::env::var("EXPORT_SHEET_DUMP").expect("EXPORT_SHEET_DUMP");
        for theme in [ExportTheme::Paper, ExportTheme::Graphite] {
            let name = format!("{theme:?}").to_lowercase();
            render_png(&chart_sheet(), theme)
                .save(format!("{dir}/chart-{name}.png"))
                .unwrap();
            std::fs::write(
                format!("{dir}/chart-{name}.pdf"),
                render_pdf(&chart_sheet(), theme),
            )
            .unwrap();
        }
        // A dense scatter density map pair.
        let blob = |cx: f32, cy: f32| -> Vec<Vec<Option<f32>>> {
            (0..96)
                .map(|r| {
                    (0..96)
                        .map(|c| {
                            let (dx, dy) = ((c as f32 - cx) / 20.0, (r as f32 - cy) / 12.0);
                            let v = (-(dx * dx + dy * dy)).exp();
                            (v > 0.05).then_some(v)
                        })
                        .collect()
                })
                .collect()
        };
        let grid = |caption: &str, norm| GridModel {
            caption: Some(caption.into()),
            x_title: "RPM".into(),
            y_title: "MAP".into(),
            x_ticks: vec![
                (0.0, "800".into()),
                (0.5, "4000".into()),
                (1.0, "7200".into()),
            ],
            y_ticks: vec![(0.0, "20".into()), (0.5, "110".into()), (1.0, "200".into())],
            norm,
            labels: Vec::new(),
            counts: None,
            kind: MapKind::Sequential,
            scale_title: "Hits".into(),
            scale_min: "1".into(),
            scale_max: "4120".into(),
            dense: true,
        };
        let sheet = Sheet {
            title: "Scatter".into(),
            meta: vec!["223\u{2009}559 records".into()],
            body: Body::Grids(vec![
                grid("MAP vs RPM", blob(40.0, 50.0)),
                grid("AFR vs RPM", blob(60.0, 30.0)),
            ]),
        };
        render_png(&sheet, ExportTheme::Paper)
            .save(format!("{dir}/scatter-paper.png"))
            .unwrap();
    }

    #[test]
    fn copy_for_paste_is_the_body_only() {
        let img = render_png_body(&chart_sheet(), ExportTheme::Paper);
        assert_eq!(img.width(), 1920);
        assert!(img.height() < 1080);
    }
}
