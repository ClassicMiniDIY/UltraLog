//! Night Drive plot chrome shared by the single and stacked chart views
//! (`design/design-system/components/Plot/README.md`, `3-data-visualization.md`).
//!
//! - Series: colour and stroke from `theme::series`, 1.5px; a soloed series goes to 2px and the
//!   others drop to 40 %. At Neon and above a wider translucent copy (bloom) sits under each line.
//!   At Ultra a perspective floor sits behind the data. Color-blind safe turns both off.
//! - Cursor: a full-height `plot_cursor` line, never a series colour.
//! - Legend: a floating `plot_legend_bg` box (top-right; top-left plus direct end labels when the
//!   palette is color-blind safe) with the Cursor row first, then swatch, name and live value in
//!   mono. Hovering a row solos that series.
//! - Hover readout: under the mouse pointer a hairline and a box with every series' value at that
//!   time (issue #98), beside the cursor legend, which keeps the clicked point.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2,
};
use egui_plot::{Line, LineStyle, PlotBounds, PlotPoint, PlotPoints, PlotTransform, PlotUi, VLine};

use rust_i18n::t;

use crate::ui::fonts::{self, style};
use crate::ui::theme::{
    SeriesStroke, Tokens,
    tokens::{radius, space, stroke},
};

/// One series as the chart draws and lists it.
pub struct SeriesRow {
    pub name: String,
    /// "A" / "B" while the tab compares two logs (issue #98); drawn before the name.
    pub tag: Option<&'static str>,
    /// Value at the cursor, already unit-converted and formatted (mono column).
    pub value: Option<String>,
    pub color: Color32,
    pub stroke: SeriesStroke,
}

/// What the chart needs to style one plot.
pub struct PlotStyle<'a> {
    pub tokens: &'a Tokens,
    /// Color blind mode, High contrast (and exports): no bloom or floor, direct end labels.
    pub cvd_like: bool,
    /// Series index soloed by hovering its legend row last frame.
    pub solo: Option<usize>,
}

fn line_style(stroke: SeriesStroke) -> LineStyle {
    match stroke {
        SeriesStroke::Solid => LineStyle::Solid,
        // 6/4 dash per the spec (egui_plot's gap is a fixed fraction of the length).
        SeriesStroke::Dashed => LineStyle::Dashed { length: 6.0 },
        SeriesStroke::Dotted => LineStyle::Dotted { spacing: 4.0 },
    }
}

/// Draw the floor (Ultra), the series with bloom and solo, and the cursor. Call inside
/// `Plot::show` after setting the bounds. `points[i]` belongs to `rows[i]`.
pub fn draw_series(
    plot_ui: &mut PlotUi<'_>,
    style: &PlotStyle<'_>,
    rows: &[SeriesRow],
    points: &[Option<Vec<[f64; 2]>>],
    cursor_time: Option<f64>,
) {
    let t = style.tokens;
    if t.fx.plot_floor && !style.cvd_like {
        for segment in floor_segments(plot_ui.plot_bounds()) {
            plot_ui.line(
                Line::new("", PlotPoints::from(segment.to_vec()))
                    .color(t.plot_floor)
                    .width(stroke::STROKE_HAIRLINE)
                    .allow_hover(false),
            );
        }
    }

    let bloom = if style.cvd_like { 0.0 } else { t.fx.bloom };
    for (i, row) in rows.iter().enumerate() {
        let Some(points) = points.get(i).and_then(|p| p.as_ref()) else {
            continue;
        };
        let dimmed = style.solo.is_some_and(|s| s != i);
        let color = if dimmed {
            row.color.gamma_multiply(0.4)
        } else {
            row.color
        };
        if bloom > 0.0 && !dimmed {
            plot_ui.line(
                Line::new("", points.iter().copied().collect::<PlotPoints<'_>>())
                    .color(row.color.gamma_multiply(0.25))
                    .width(stroke::STROKE_SERIES + 2.0 * bloom)
                    .style(line_style(row.stroke))
                    .allow_hover(false),
            );
        }
        let width = if style.solo == Some(i) {
            stroke::STROKE_INDICATOR
        } else {
            stroke::STROKE_SERIES
        };
        plot_ui.line(
            Line::new(
                row.name.clone(),
                points.iter().copied().collect::<PlotPoints<'_>>(),
            )
            .color(color)
            .width(width)
            .style(line_style(row.stroke)),
        );
    }

    if let Some(time) = cursor_time {
        plot_ui.vline(
            VLine::new("Cursor", time)
                .color(t.plot_cursor)
                .width(stroke::STROKE_CURSOR),
        );
    }
}

/// Floor segments in plot coordinates: a horizon at 45 % of the height, lines fanning from a
/// vanishing point above it to the bottom edge, and horizontals closing up toward the horizon.
fn floor_segments(bounds: PlotBounds) -> Vec<[[f64; 2]; 2]> {
    let [x0, y0] = bounds.min();
    let [x1, y1] = bounds.max();
    let horizon = y0 + (y1 - y0) * 0.45;
    let depth = horizon - y0;
    let vanish = [(x0 + x1) / 2.0, horizon + depth * 0.9];
    let width = x1 - x0;
    let mut out = Vec::new();
    let fan = 16;
    for i in 0..=fan {
        let x = x0 - width * 0.75 + width * 2.5 * i as f64 / fan as f64;
        let k = (horizon - vanish[1]) / (y0 - vanish[1]);
        let start = [vanish[0] + (x - vanish[0]) * k, horizon];
        let end = [x, y0];
        if let Some(segment) = clip_x(start, end, x0, x1) {
            out.push(segment);
        }
    }
    for i in 1..=5 {
        let k = (i as f64 / 5.0).powi(2);
        let y = horizon - depth * k;
        out.push([[x0, y], [x1, y]]);
    }
    out
}

/// Clip the segment `a`-`b` to the band `x0..=x1`.
fn clip_x(a: [f64; 2], b: [f64; 2], x0: f64, x1: f64) -> Option<[[f64; 2]; 2]> {
    let dx = b[0] - a[0];
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a[0] - x0), (dx, x1 - a[0])] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else if p < 0.0 {
            t0 = t0.max(q / p);
        } else {
            t1 = t1.min(q / p);
        }
    }
    let at = |t: f64| [a[0] + dx * t, a[1] + (b[1] - a[1]) * t];
    (t0 < t1).then(|| [at(t0), at(t1)])
}

/// How many of `series` legend rows to show in `max_rows`, and how many go into a "+N" row.
fn legend_fit(max_rows: usize, series: usize, cursor_rows: usize) -> (usize, usize) {
    let shown = if series + cursor_rows <= max_rows {
        series
    } else {
        max_rows.saturating_sub(cursor_rows + 1)
    };
    (shown, series - shown)
}

/// Paint the floating legend over `plot_rect` and return the row the pointer is on, if any
/// (used to solo that series next frame), and the legend's rect (the hover readout stays off
/// it). `cursor` is the formatted cursor time.
pub fn legend_overlay(
    ui: &Ui,
    plot_rect: Rect,
    style: &PlotStyle<'_>,
    rows: &[SeriesRow],
    cursor: Option<String>,
    id: Id,
) -> (Option<usize>, Option<Rect>) {
    let t = style.tokens;
    let name_font = fonts::text_style(style::LABEL).resolve(ui.style());
    let value_font = fonts::text_style(style::DATA).resolve(ui.style());
    let row_h = (name_font.size.max(value_font.size) + space::SPACE_2).round();
    let swatch_w = 14.0;

    let measure = |text: &str, font: &FontId| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font.clone(), t.ink)
            .size()
            .x
    };
    let cursor_label = t!("chart.cursor").to_string();
    let mut name_w = measure(&cursor_label, &name_font);
    let mut value_w = cursor.as_deref().map_or(0.0, |c| measure(c, &value_font));
    let mut tag_w: f32 = 0.0;
    for row in rows {
        name_w = name_w.max(measure(&row.name, &name_font));
        if let Some(v) = &row.value {
            value_w = value_w.max(measure(v, &value_font));
        }
        if let Some(tag) = row.tag {
            tag_w = tag_w.max(measure(tag, &value_font) + space::SPACE_2);
        }
    }
    let pad = space::SPACE_2 + 2.0;
    let width = pad + swatch_w + space::SPACE_2 + tag_w + name_w + space::SPACE_6 + value_w + pad;
    let inset = space::SPACE_3;
    // The legend is the only cursor readout, so a short plot shows the rows that fit and a
    // "+N" row for the rest instead of dropping the legend.
    let cursor_rows = usize::from(cursor.is_some());
    let max_rows = ((plot_rect.height() - 2.0 * inset - 2.0 * pad + space::SPACE_2) / row_h)
        .floor()
        .max(0.0) as usize;
    let (shown, hidden) = legend_fit(max_rows, rows.len(), cursor_rows);
    let count = cursor_rows + shown + usize::from(hidden > 0);
    if count > max_rows {
        // Not even the cursor row and a "+N" row fit.
        return (None, None);
    }
    let height = pad * 2.0 + row_h * count as f32 - space::SPACE_2;
    let origin = if style.cvd_like {
        plot_rect.left_top() + vec2(inset, inset)
    } else {
        pos2(plot_rect.right() - inset - width, plot_rect.top() + inset)
    };
    let rect = Rect::from_min_size(origin, vec2(width, height));
    if !plot_rect.contains_rect(rect) {
        return (None, None);
    }

    let painter = ui.painter().with_clip_rect(plot_rect);
    painter.rect(
        rect,
        CornerRadius::same(radius::RADIUS_MD),
        t.plot_legend_bg,
        Stroke::new(stroke::STROKE_HAIRLINE, t.line),
        egui::StrokeKind::Inside,
    );

    let mut y = rect.top() + pad;
    let swatch_x = rect.left() + pad;
    let name_x = swatch_x + swatch_w + space::SPACE_2;
    let value_x = rect.right() - pad;
    if let Some(cursor) = cursor {
        let cy = y + row_h / 2.0 - space::SPACE_1;
        painter.circle_filled(pos2(swatch_x + swatch_w / 2.0, cy), 4.0, t.plot_cursor);
        painter.text(
            pos2(name_x, cy),
            Align2::LEFT_CENTER,
            &cursor_label,
            name_font.clone(),
            t.ink_muted,
        );
        painter.text(
            pos2(value_x, cy),
            Align2::RIGHT_CENTER,
            cursor,
            value_font.clone(),
            t.ink,
        );
        y += row_h;
    }

    let mut hovered = None;
    for (i, row) in rows.iter().enumerate().take(shown) {
        let row_rect =
            Rect::from_min_size(pos2(rect.left(), y - space::SPACE_1), vec2(width, row_h));
        let response = ui.interact(row_rect, id.with(("legend_row", i)), Sense::hover());
        if response.hovered() {
            hovered = Some(i);
        }
        let dimmed = style.solo.is_some_and(|s| s != i);
        let cy = y + row_h / 2.0 - space::SPACE_1;
        swatch(
            &painter,
            pos2(swatch_x, cy),
            swatch_w,
            if dimmed {
                row.color.gamma_multiply(0.4)
            } else {
                row.color
            },
            row.stroke,
        );
        if let Some(tag) = row.tag {
            painter.text(
                pos2(name_x, cy),
                Align2::LEFT_CENTER,
                tag,
                value_font.clone(),
                if dimmed { t.ink_subtle } else { t.ink_muted },
            );
        }
        painter.text(
            pos2(name_x + tag_w, cy),
            Align2::LEFT_CENTER,
            &row.name,
            name_font.clone(),
            if dimmed { t.ink_subtle } else { t.ink },
        );
        if let Some(v) = &row.value {
            painter.text(
                pos2(value_x, cy),
                Align2::RIGHT_CENTER,
                v,
                value_font.clone(),
                if dimmed { t.ink_subtle } else { t.ink },
            );
        }
        y += row_h;
    }
    if hidden > 0 {
        painter.text(
            pos2(name_x, y + row_h / 2.0 - space::SPACE_1),
            Align2::LEFT_CENTER,
            format!("+{hidden}"),
            value_font.clone(),
            t.ink_subtle,
        );
    }
    (hovered, Some(rect))
}

/// The legend / channel-card swatch: a short line in the series colour, dashed or dotted to
/// match the series stroke, so identity never rests on colour alone.
pub fn swatch(
    painter: &egui::Painter,
    left_center: Pos2,
    width: f32,
    color: Color32,
    kind: SeriesStroke,
) {
    let s = Stroke::new(stroke::STROKE_INDICATOR, color);
    let a = left_center;
    let b = left_center + vec2(width, 0.0);
    match kind {
        SeriesStroke::Solid => {
            painter.line_segment([a, b], s);
        }
        SeriesStroke::Dashed => {
            painter.extend(egui::Shape::dashed_line(&[a, b], s, 4.0, 2.5));
        }
        SeriesStroke::Dotted => {
            painter.extend(egui::Shape::dotted_line(&[a, b], color, 3.5, 1.0));
        }
    }
}

/// Direct labels at each line's right end (color-blind safe palettes): the series name in its
/// colour, pushed apart vertically so labels never overlap.
pub fn direct_labels(
    ui: &Ui,
    transform: &PlotTransform,
    rows: &[SeriesRow],
    points: &[Option<Vec<[f64; 2]>>],
) {
    let plot_rect = *transform.frame();
    let x_max = transform.bounds().max()[0];
    let font = fonts::text_style(style::DATA_SM).resolve(ui.style());
    let mut labels: Vec<(f32, usize)> = rows
        .iter()
        .enumerate()
        .filter_map(|(i, _)| {
            let pts = points.get(i)?.as_ref()?;
            let last = pts.iter().rev().find(|p| p[0] <= x_max)?;
            let pos = transform.position_from_point(&PlotPoint::new(last[0], last[1]));
            Some((pos.y, i))
        })
        .collect();
    labels.sort_by(|a, b| a.0.total_cmp(&b.0));
    let gap = font.size + 2.0;
    for k in 1..labels.len() {
        if labels[k].0 - labels[k - 1].0 < gap {
            labels[k].0 = labels[k - 1].0 + gap;
        }
    }
    let painter = ui.painter().with_clip_rect(plot_rect);
    for (y, i) in labels {
        let y = y.clamp(plot_rect.top() + gap / 2.0, plot_rect.bottom() - gap / 2.0);
        painter.text(
            pos2(plot_rect.right() - space::SPACE_2, y - gap / 2.0),
            Align2::RIGHT_CENTER,
            // With the tag: in Color blind mode and High contrast a pair shares its colour.
            match rows[i].tag {
                Some(tag) => format!("{tag} {}", rows[i].name),
                None => rows[i].name.clone(),
            },
            font.clone(),
            rows[i].color,
        );
    }
}

/// Read and store the soloed series for a plot between frames.
pub fn solo_memory(ctx: &egui::Context, id: Id, update: Option<Option<usize>>) -> Option<usize> {
    let key = id.with("solo");
    if let Some(value) = update {
        ctx.data_mut(|d| d.insert_temp(key, value));
        value
    } else {
        ctx.data(|d| d.get_temp::<Option<usize>>(key)).flatten()
    }
}

/// Gap between the mouse pointer and the hover readout box.
const READOUT_OFFSET: f32 = 14.0;

/// Where the hover readout box of `size` goes: below-right of the pointer, flipped left or up
/// when it would leave the plot, and kept inside the plot.
fn readout_origin(pointer: Pos2, size: egui::Vec2, plot: Rect) -> Pos2 {
    let mut x = pointer.x + READOUT_OFFSET;
    if x + size.x > plot.right() {
        x = pointer.x - READOUT_OFFSET - size.x;
    }
    let mut y = pointer.y + READOUT_OFFSET;
    if y + size.y > plot.bottom() {
        y = pointer.y - READOUT_OFFSET - size.y;
    }
    pos2(
        x.clamp(plot.left(), (plot.right() - size.x).max(plot.left())),
        y.clamp(plot.top(), (plot.bottom() - size.y).max(plot.top())),
    )
}

/// The hover readout's hairline alone: a full-height line at the pointer. Also drawn while
/// Align at cursor waits for its click, when the box is off.
pub fn hover_line(ui: &Ui, plot_rect: Rect, style: &PlotStyle<'_>, pointer: Pos2) {
    let area = plot_rect.intersect(ui.clip_rect());
    if !area.contains(pointer) {
        return;
    }
    ui.painter().with_clip_rect(area).line_segment(
        [
            pos2(pointer.x, plot_rect.top()),
            pos2(pointer.x, plot_rect.bottom()),
        ],
        Stroke::new(stroke::STROKE_HAIRLINE, style.tokens.ink_subtle),
    );
}

/// The hover readout (issue #98): a hairline at the pointer and, beside it, a box with the time
/// and every series' value there (`rows` come from `series_rows_at` for the hover time). A
/// series with no data at that time (log B outside its span) shows a dash.
pub fn hover_readout(
    ui: &Ui,
    plot_rect: Rect,
    style: &PlotStyle<'_>,
    rows: &[SeriesRow],
    time: &str,
    pointer: Pos2,
) {
    // Place and flip inside the part of the plot that is on screen: a stacked plot can be
    // partly scrolled out of its ScrollArea.
    let area = plot_rect.intersect(ui.clip_rect());
    if rows.is_empty() || !area.contains(pointer) {
        return;
    }
    let t = style.tokens;
    let painter = ui.painter().with_clip_rect(area);
    hover_line(ui, plot_rect, style, pointer);

    let name_font = fonts::text_style(style::LABEL).resolve(ui.style());
    let value_font = fonts::text_style(style::DATA).resolve(ui.style());
    let row_h = (name_font.size.max(value_font.size) + space::SPACE_2).round();
    let swatch_w = 14.0;
    let measure = |text: &str, font: &FontId| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font.clone(), t.ink)
            .size()
            .x
    };
    let dash = "\u{2013}";
    let mut name_w: f32 = 0.0;
    let mut value_w = measure(time, &value_font);
    let mut tag_w: f32 = 0.0;
    for row in rows {
        name_w = name_w.max(measure(&row.name, &name_font));
        value_w = value_w.max(measure(row.value.as_deref().unwrap_or(dash), &value_font));
        if let Some(tag) = row.tag {
            tag_w = tag_w.max(measure(tag, &value_font) + space::SPACE_2);
        }
    }
    let pad = space::SPACE_2 + 2.0;
    let width = pad + swatch_w + space::SPACE_2 + tag_w + name_w + space::SPACE_6 + value_w + pad;
    // A short plot lists the rows that fit and a "+N" row for the rest, like the legend.
    let max_rows = ((area.height() - 2.0 * pad + space::SPACE_2) / row_h)
        .floor()
        .max(0.0) as usize;
    let (shown, hidden) = legend_fit(max_rows, rows.len(), 1);
    let count = 1 + shown + usize::from(hidden > 0);
    if count > max_rows {
        return;
    }
    let height = pad * 2.0 + row_h * count as f32 - space::SPACE_2;
    let origin = readout_origin(pointer, vec2(width, height), area);
    let rect = Rect::from_min_size(origin, vec2(width, height));
    painter.rect(
        rect,
        CornerRadius::same(radius::RADIUS_MD),
        t.plot_legend_bg,
        Stroke::new(stroke::STROKE_HAIRLINE, t.line),
        egui::StrokeKind::Inside,
    );

    let swatch_x = rect.left() + pad;
    let name_x = swatch_x + swatch_w + space::SPACE_2;
    let value_x = rect.right() - pad;
    let mut y = rect.top() + pad;
    let cy = y + row_h / 2.0 - space::SPACE_1;
    painter.text(
        pos2(value_x, cy),
        Align2::RIGHT_CENTER,
        time,
        value_font.clone(),
        t.ink_muted,
    );
    y += row_h;
    for row in rows.iter().take(shown) {
        let cy = y + row_h / 2.0 - space::SPACE_1;
        swatch(
            &painter,
            pos2(swatch_x, cy),
            swatch_w,
            row.color,
            row.stroke,
        );
        if let Some(tag) = row.tag {
            painter.text(
                pos2(name_x, cy),
                Align2::LEFT_CENTER,
                tag,
                value_font.clone(),
                t.ink_muted,
            );
        }
        painter.text(
            pos2(name_x + tag_w, cy),
            Align2::LEFT_CENTER,
            &row.name,
            name_font.clone(),
            t.ink,
        );
        let (value, color) = match &row.value {
            Some(v) => (v.as_str(), t.ink),
            None => (dash, t.ink_subtle),
        };
        painter.text(
            pos2(value_x, cy),
            Align2::RIGHT_CENTER,
            value,
            value_font.clone(),
            color,
        );
        y += row_h;
    }
    if hidden > 0 {
        painter.text(
            pos2(name_x, y + row_h / 2.0 - space::SPACE_1),
            Align2::LEFT_CENTER,
            format!("+{hidden}"),
            value_font,
            t.ink_subtle,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{legend_fit, readout_origin};
    use eframe::egui::{Rect, pos2, vec2};

    #[test]
    fn the_readout_stays_inside_the_plot() {
        let plot = Rect::from_min_max(pos2(0.0, 0.0), pos2(800.0, 400.0));
        let size = vec2(200.0, 100.0);
        // Room below-right: there.
        assert_eq!(
            readout_origin(pos2(100.0, 100.0), size, plot),
            pos2(114.0, 114.0)
        );
        // Near the right edge: flips left of the pointer.
        assert_eq!(
            readout_origin(pos2(700.0, 100.0), size, plot),
            pos2(486.0, 114.0)
        );
        // Near the bottom: flips above the pointer.
        assert_eq!(
            readout_origin(pos2(100.0, 390.0), size, plot),
            pos2(114.0, 276.0)
        );
        // A box larger than the plot pins to the top-left.
        assert_eq!(
            readout_origin(pos2(10.0, 10.0), vec2(900.0, 500.0), plot),
            pos2(0.0, 0.0)
        );
    }

    #[test]
    fn legend_keeps_the_rows_that_fit_and_counts_the_rest() {
        // Everything fits: cursor row + 4 series in 6 rows.
        assert_eq!(legend_fit(6, 4, 1), (4, 0));
        // 10 series + cursor in 6 rows: cursor, 4 series, "+6".
        assert_eq!(legend_fit(6, 10, 1), (4, 6));
        // Room for the cursor and the "+N" row only.
        assert_eq!(legend_fit(2, 3, 1), (0, 3));
        // No room at all.
        assert_eq!(legend_fit(0, 3, 1), (0, 3));
    }
}
