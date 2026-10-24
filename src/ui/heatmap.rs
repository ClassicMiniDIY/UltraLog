//! Shared painting for heatmaps and tables: Histogram, scatter density, Lambda Delay and Accel
//! Enrichment (`design/design-system/components/{HeatmapTable,ColorScale}/README.md`,
//! `3-data-visualization.md` → Colour maps, Tables and heatmaps).
//!
//! - Cells fill from the sequential (`seq`) or diverging (`div`) map, interpolated in OKLab.
//! - Value text uses the nearest stop's `on_seq_N` / `on_div_N`, on a `cell_plate` when the
//!   theme shows one (Color-blind safe, High contrast), so every value reaches 7:1.
//! - Empty cells are `cell_empty` with a 45° hatch, so "no samples" never reads as a low value.
//! - Confidence is a fill style on the count badge: filled high, outlined medium, dashed low.
//! - The selected cell gets a 2px `cyan` ring with a `bg_void` gap.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Ui, pos2,
    vec2,
};

use crate::ui::fonts::{self, style};
use crate::ui::theme::{
    self, Tokens, paint,
    tokens::{radius, space, stroke},
};

/// Which colour map a heatmap uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapKind {
    /// Magnitudes: hit counts, dead time, RPM, density.
    Sequential,
    /// Signed values around a target: AFR / λ vs target, correction %. 0.5 is on target.
    Diverging,
}

/// Fill colour for `v` in 0..=1.
pub fn fill(t: &Tokens, kind: MapKind, v: f32) -> Color32 {
    match kind {
        MapKind::Sequential => theme::seq(t, v),
        MapKind::Diverging => theme::div(t, v),
    }
}

/// Text colour for a value drawn on `fill(.., v)`.
pub fn text_on(t: &Tokens, kind: MapKind, v: f32) -> Color32 {
    match kind {
        MapKind::Sequential => theme::on_seq(t, v),
        MapKind::Diverging => theme::on_div(t, v),
    }
}

/// Paint a filled cell with an optional value label.
pub fn cell(
    painter: &Painter,
    rect: Rect,
    t: &Tokens,
    kind: MapKind,
    v: f32,
    label: Option<&str>,
    font: &FontId,
) {
    painter.rect_filled(
        rect,
        CornerRadius::same(radius::RADIUS_XS),
        fill(t, kind, v),
    );
    if let Some(label) = label {
        value_label(painter, rect.center(), label, font, t, text_on(t, kind, v));
    }
}

/// A value centred at `center`, on the theme's cell plate when it has one.
pub fn value_label(
    painter: &Painter,
    center: Pos2,
    label: &str,
    font: &FontId,
    t: &Tokens,
    color: Color32,
) {
    let galley = painter.layout_no_wrap(label.to_owned(), font.clone(), color);
    if t.cell_plate.a() > 0 {
        let plate = Rect::from_center_size(center, galley.size() + vec2(6.0, 2.0));
        painter.rect_filled(plate, CornerRadius::same(radius::RADIUS_XS), t.cell_plate);
        painter.galley(center - galley.size() / 2.0, galley, t.ink);
    } else {
        painter.galley(center - galley.size() / 2.0, galley, color);
    }
}

/// An empty cell: `cell_empty` with a 45° hatch.
pub fn empty_cell(painter: &Painter, rect: Rect, t: &Tokens) {
    painter.rect_filled(rect, CornerRadius::same(radius::RADIUS_XS), t.cell_empty);
    let p = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let s = Stroke::new(stroke::STROKE_HAIRLINE, t.line_subtle);
    let step = 6.0;
    let mut x = rect.left() - rect.height();
    while x < rect.right() {
        p.line_segment(
            [pos2(x, rect.bottom()), pos2(x + rect.height(), rect.top())],
            s,
        );
        x += step;
    }
}

/// How sure a cell is; drawn as a fill style, never a hue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeStyle {
    High,
    Medium,
    Low,
}

/// An event-count badge anchored at its top-right corner.
pub fn count_badge(
    painter: &Painter,
    top_right: Pos2,
    count: usize,
    style: BadgeStyle,
    t: &Tokens,
    font: &FontId,
) {
    let text = count.to_string();
    let galley = painter.layout_no_wrap(text.clone(), font.clone(), t.ink);
    let size = galley.size() + vec2(5.0, 1.0);
    let rect = Rect::from_min_size(pos2(top_right.x - size.x, top_right.y), size);
    let corner = CornerRadius::same(radius::RADIUS_XS);
    match style {
        BadgeStyle::High => {
            painter.rect_filled(rect, corner, t.ink);
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                text,
                font.clone(),
                t.bg_void,
            );
        }
        BadgeStyle::Medium => {
            painter.rect(
                rect,
                corner,
                t.bg_void,
                Stroke::new(1.0, t.ink),
                StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                text,
                font.clone(),
                t.ink,
            );
        }
        BadgeStyle::Low => {
            painter.rect_filled(rect, corner, t.bg_void);
            paint::dashed_rect(
                painter,
                rect.shrink(0.5),
                1.0,
                Stroke::new(1.0, t.ink_muted),
                2.0,
            );
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                text,
                font.clone(),
                t.ink_muted,
            );
        }
    }
}

/// The selected cell: a `bg_void` gap, then a 2px `cyan` ring.
pub fn selected_ring(painter: &Painter, rect: Rect, t: &Tokens) {
    let corner = |e: f32| CornerRadius::same((f32::from(radius::RADIUS_XS) + e) as u8);
    painter.rect_stroke(
        rect.expand(1.0),
        corner(1.0),
        Stroke::new(2.0, t.bg_void),
        StrokeKind::Middle,
    );
    painter.rect_stroke(
        rect.expand(3.0),
        corner(3.0),
        Stroke::new(stroke::STROKE_INDICATOR, t.cyan),
        StrokeKind::Middle,
    );
}

/// The ColorScale beside every map: an overline title, the real min / max in mono, and the
/// gradient. The diverging scale labels its ends and middle (rich · target · lean).
pub fn color_scale(ui: &mut Ui, kind: MapKind, title: &str, min_label: &str, max_label: &str) {
    let t = theme::current(ui.ctx());
    ui.label(
        egui::RichText::new(title.to_uppercase())
            .text_style(fonts::text_style(style::OVERLINE))
            .extra_letter_spacing(0.9)
            .color(t.ink_muted),
    );
    let mono = fonts::text_style(style::DATA_SM).resolve(ui.style());
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(min_label)
                .font(mono.clone())
                .color(t.ink_muted),
        );
        let (rect, _) = ui.allocate_exact_size(vec2(180.0, 8.0), egui::Sense::hover());
        gradient_bar(ui.painter(), rect, &t, kind);
        ui.label(
            egui::RichText::new(max_label)
                .font(mono.clone())
                .color(t.ink_muted),
        );
        if kind == MapKind::Diverging {
            let y = rect.bottom() + space::SPACE_1;
            for (x, text, align) in [
                (rect.left(), "rich", Align2::LEFT_TOP),
                (rect.center().x, "target", Align2::CENTER_TOP),
                (rect.right(), "lean", Align2::RIGHT_TOP),
            ] {
                ui.painter()
                    .text(pos2(x, y), align, text, mono.clone(), t.ink_subtle);
            }
        }
    });
    if kind == MapKind::Diverging {
        ui.add_space(mono.size);
    }
}

/// A horizontal gradient of the map across `rect` (pill ends).
pub fn gradient_bar(painter: &Painter, rect: Rect, t: &Tokens, kind: MapKind) {
    let steps = 48;
    let mut mesh = egui::Mesh::default();
    for i in 0..=steps {
        let k = i as f32 / steps as f32;
        let x = rect.left() + rect.width() * k;
        let c = fill(t, kind, k);
        mesh.colored_vertex(pos2(x, rect.top()), c);
        mesh.colored_vertex(pos2(x, rect.bottom()), c);
    }
    for i in 0..steps as u32 {
        let a = i * 2;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// A vertical gradient (low at the bottom) for scales that sit beside a square map.
pub fn gradient_bar_vertical(painter: &Painter, rect: Rect, t: &Tokens, kind: MapKind) {
    let steps = 48;
    let mut mesh = egui::Mesh::default();
    for i in 0..=steps {
        let k = i as f32 / steps as f32;
        let y = rect.bottom() - rect.height() * k;
        let c = fill(t, kind, k);
        mesh.colored_vertex(pos2(rect.left(), y), c);
        mesh.colored_vertex(pos2(rect.right(), y), c);
    }
    for i in 0..steps as u32 {
        let a = i * 2;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::tokens as ut;

    #[test]
    fn cell_text_reaches_contrast_on_every_stop() {
        fn lum(c: Color32) -> f32 {
            let lin = |v: u8| {
                let v = v as f32 / 255.0;
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            let [r, g, b, _] = c.to_srgba_unmultiplied();
            0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
        }
        let ratio = |a: Color32, b: Color32| {
            let (x, y) = (lum(a), lum(b));
            (x.max(y) + 0.05) / (x.min(y) + 0.05)
        };
        for t in [ut::NIGHT, ut::CVD, ut::HC] {
            for kind in [MapKind::Sequential, MapKind::Diverging] {
                let stops = if kind == MapKind::Sequential { 6 } else { 4 };
                for i in 0..=stops {
                    let v = i as f32 / stops as f32;
                    // With a plate the text is ink on the plate; without, on_* on the fill.
                    let r = if t.cell_plate.a() > 0 {
                        ratio(t.ink, t.cell_plate.to_opaque())
                    } else {
                        ratio(text_on(&t, kind, v), fill(&t, kind, v))
                    };
                    assert!(r >= 4.5, "{kind:?} stop {i}: {r:.2}");
                }
            }
        }
    }
}
