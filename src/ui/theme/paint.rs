//! The custom painters the design system allows.
//!
//! The brief (`design/design-system/6-implementation.md`) keeps custom painting to four small
//! helpers — redline, glow, series bloom, perspective floor — and the plan adds three
//! (`docs/plans/2026-10-23-night-drive-design-system.md`, Decision 2): the focus ring, the plot
//! legend overlay and the major grid. Everything else comes from egui `Visuals`.

use eframe::egui::{
    self, Color32, CornerRadius, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind,
};

use super::Tokens;
use super::tokens::{radius, stroke};

/// Sunset stop positions along the redline (cyan → violet → magenta → orange → gold).
const REDLINE_STOPS: [f32; 5] = [0.0, 0.30, 0.55, 0.80, 1.0];

/// The 2px sunset redline along the top edge of `rect` (windows, welcome screen, export
/// headers). Flat `line_control` hairline when the theme turns the gradient off (Stealth, High
/// contrast). `inset` keeps it inside rounded corners.
pub fn redline(painter: &Painter, rect: Rect, t: &Tokens, inset: f32) {
    let left = rect.left() + inset;
    let right = rect.right() - inset;
    if right <= left {
        return;
    }
    let height = stroke::STROKE_INDICATOR;
    let top = rect.top();
    if !t.fx.redline_gradient {
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, top + height)),
            CornerRadius::ZERO,
            t.line_control,
        );
        return;
    }
    let colors = [
        t.redline_0,
        t.redline_1,
        t.redline_2,
        t.redline_3,
        t.redline_4,
    ];
    let mut mesh = Mesh::default();
    for (stop, color) in REDLINE_STOPS.iter().zip(colors) {
        let x = left + (right - left) * stop;
        mesh.colored_vertex(Pos2::new(x, top), color);
        mesh.colored_vertex(Pos2::new(x, top + height), color);
    }
    for i in 0..(REDLINE_STOPS.len() as u32 - 1) {
        let a = i * 2;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    painter.add(Shape::mesh(mesh));
}

/// A glow behind `rect` in `color` (primary button hover, active mode-nav bar). `None` at
/// intensities that turn glows off.
pub fn glow(t: &Tokens, rect: Rect, corner_radius: u8, color: Color32) -> Option<Shape> {
    super::glow(t, color).map(|shadow| Shape::Rect(shadow.as_shape(rect, corner_radius)))
}

/// Keyboard focus: a ground-coloured gap, then a solid `cyan` ring (5px in High contrast).
pub fn focus_ring(painter: &Painter, rect: Rect, corner_radius: f32, t: &Tokens, wide: bool) {
    let gap = 2.0;
    let ring = if wide { 3.0 } else { 2.0 };
    let r = |extra: f32| CornerRadius::same((corner_radius + extra).round().min(255.0) as u8);
    painter.rect_stroke(
        rect.expand(gap / 2.0),
        r(gap / 2.0),
        Stroke::new(gap, t.bg_void),
        StrokeKind::Middle,
    );
    painter.rect_stroke(
        rect.expand(gap + ring / 2.0),
        r(gap + ring / 2.0),
        Stroke::new(ring, t.cyan),
        StrokeKind::Middle,
    );
}

const KEYBOARD_FOCUS: &str = "ultralog_keyboard_focus";

/// Paint the focus ring around whichever widget has keyboard focus, on a foreground layer so
/// every control gets the same ring without its own code. Call once per frame, after the UI.
///
/// egui also focuses a button that was clicked with the mouse; the ring shows only after the
/// keyboard moved focus (Tab, arrows) or on a text field, and hides again on the next pointer
/// press.
pub fn keyboard_focus_ring(ctx: &egui::Context, t: &Tokens, high_contrast: bool) {
    let id = egui::Id::new(KEYBOARD_FOCUS);
    let (tabbed, pressed) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Tab)
                || i.key_pressed(egui::Key::ArrowDown)
                || i.key_pressed(egui::Key::ArrowUp),
            i.pointer.any_pressed(),
        )
    });
    let mut keyboard = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if tabbed {
        keyboard = true;
    } else if pressed {
        keyboard = false;
    }
    ctx.data_mut(|d| d.insert_temp(id, keyboard));

    let Some(focused) = ctx.memory(|m| m.focused()) else {
        return;
    };
    let Some(response) = ctx.read_response(focused) else {
        return;
    };
    let is_text = egui::TextEdit::load_state(ctx, focused).is_some();
    if !(keyboard || is_text) || !response.rect.is_positive() {
        return;
    }
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("ultralog_focus_ring"),
    ));
    let painter = painter.with_clip_rect(response.interact_rect.expand(8.0));
    focus_ring(
        &painter,
        response.rect,
        f32::from(radius::RADIUS_SM),
        t,
        high_contrast,
    );
}

/// The synthwave perspective floor: lines from a vanishing point above `rect` fanning to its
/// bottom edge, crossed by horizontals that close up toward the horizon. Drawn before the
/// content (drop zone; plots at Ultra).
pub fn floor(painter: &Painter, rect: Rect, color: Color32) {
    if color.a() == 0 || !rect.is_positive() {
        return;
    }
    let s = Stroke::new(stroke::STROKE_HAIRLINE, color);
    let horizon = rect.top() + rect.height() * 0.35;
    let vanish = Pos2::new(rect.center().x, horizon);
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let fan = 14;
    for i in 0..=fan {
        let x = rect.left() - rect.width() * 0.5 + rect.width() * 2.0 * i as f32 / fan as f32;
        painter.line_segment([vanish, Pos2::new(x, rect.bottom())], s);
    }
    let rows = 8;
    for i in 1..=rows {
        // Quadratic spacing: rows bunch toward the horizon like a receding floor.
        let k = (i as f32 / rows as f32).powi(2);
        let y = horizon + (rect.bottom() - horizon) * k;
        painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], s);
    }
}
