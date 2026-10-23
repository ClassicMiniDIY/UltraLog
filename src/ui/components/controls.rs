//! Form controls: Segmented, Checkbox and the Ultra intensity Slider
//! (`design/design-system/components/{Segmented,Checkbox,Slider,IntensityControl}/README.md`).

use eframe::egui::{
    self, Align2, CornerRadius, FontId, Mesh, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    pos2, vec2,
};

use crate::ui::components::icon;
use crate::ui::fonts::{self, style};
use crate::ui::theme::{
    self,
    tokens::{radius, space, stroke},
};

/// A mutually exclusive choice of 2-5 short options shown at once. The active segment is
/// `cyan_soft` with `cyan` text and a 1px cyan inset; the others `ink_muted` on `bg_field`.
/// Returns true when the value changed.
pub fn segmented<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let t = theme::current(ui.ctx());
    let font = egui::TextStyle::Button.resolve(ui.style());
    let mut changed = false;
    egui::Frame::NONE
        .fill(t.bg_field)
        .stroke(Stroke::new(stroke::STROKE_HAIRLINE, t.line_control))
        .corner_radius(CornerRadius::same(radius::RADIUS_SM))
        .inner_margin(egui::Margin::same(2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (option, label) in options {
                    let active = *value == *option;
                    let galley =
                        ui.painter()
                            .layout_no_wrap((*label).to_owned(), font.clone(), t.ink);
                    let size = vec2(
                        galley.size().x + 2.0 * space::SPACE_3,
                        galley.size().y + space::SPACE_2,
                    );
                    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                    let corner = CornerRadius::same(radius::RADIUS_SM - 1);
                    let color = if active {
                        ui.painter().rect(
                            rect,
                            corner,
                            t.cyan_soft,
                            Stroke::new(stroke::STROKE_HAIRLINE, t.cyan),
                            StrokeKind::Inside,
                        );
                        t.cyan
                    } else if response.hovered() {
                        ui.painter().rect_filled(rect, corner, t.bg_hover);
                        t.ink
                    } else {
                        t.ink_muted
                    };
                    ui.painter()
                        .galley(rect.center() - galley.size() / 2.0, galley, color);
                    if response.clicked() && !active {
                        *value = *option;
                        changed = true;
                    }
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            });
        });
    changed
}

/// A 16px checkbox: `line_control` border on `bg_field`; checked fills `cyan` with an `on_cyan`
/// check. The label is `ink`, 13px. The whole row is the hit area.
pub fn checkbox(ui: &mut Ui, checked: &mut bool, label: impl Into<String>) -> Response {
    let t = theme::current(ui.ctx());
    let enabled = ui.is_enabled();
    let font = fonts::text_style(style::LABEL).resolve(ui.style());
    let text_color = if enabled { t.ink } else { t.ink_disabled };
    let galley = ui.painter().layout_no_wrap(label.into(), font, text_color);
    let box_size = 16.0;
    let size = vec2(
        box_size + space::SPACE_2 + galley.size().x,
        galley.size().y.max(box_size) + 2.0,
    );
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    let boxr = Rect::from_min_size(
        pos2(rect.left(), rect.center().y - box_size / 2.0),
        vec2(box_size, box_size),
    );
    let corner = CornerRadius::same(radius::RADIUS_XS + 1);
    let painter = ui.painter();
    if *checked {
        painter.rect_filled(boxr, corner, if enabled { t.cyan } else { t.ink_disabled });
        painter.text(
            boxr.center(),
            Align2::CENTER_CENTER,
            icon::CHECK,
            FontId::proportional(12.0),
            t.on_cyan,
        );
    } else {
        let border = if response.hovered() && enabled {
            t.ink_muted
        } else {
            t.line_control
        };
        painter.rect(
            boxr,
            corner,
            t.bg_field,
            Stroke::new(stroke::STROKE_HAIRLINE, border),
            StrokeKind::Inside,
        );
    }
    painter.galley(
        pos2(
            boxr.right() + space::SPACE_2,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        text_color,
    );
    if response.hovered() && enabled {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

/// The Ultra intensity track: 0..=100 on a 4px track that previews the dial, grey
/// `line_control` into the sunset `redline_*` stops, with a 14px `ink` thumb ringed in `cyan`.
/// Arrow keys step by 1 when focused. Returns the response (`changed()` on any change).
pub fn intensity_slider(ui: &mut Ui, value: &mut u8, width: f32) -> Response {
    let t = theme::current(ui.ctx());
    let enabled = ui.is_enabled();
    let thumb = 14.0;
    let (rect, mut response) =
        ui.allocate_exact_size(vec2(width, thumb + 6.0), Sense::click_and_drag());
    let track = Rect::from_min_max(
        pos2(rect.left() + thumb / 2.0, rect.center().y - 2.0),
        pos2(rect.right() - thumb / 2.0, rect.center().y + 2.0),
    );

    let old = *value;
    if enabled {
        if let Some(p) = response.interact_pointer_pos()
            && (response.dragged() || response.clicked())
        {
            let k = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
            *value = (k * 100.0).round() as u8;
        }
        if response.has_focus() {
            let (left, right) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowLeft),
                    i.key_pressed(egui::Key::ArrowRight),
                )
            });
            if left {
                *value = value.saturating_sub(1);
            }
            if right {
                *value = (*value + 1).min(100);
            }
        }
    }
    if *value != old {
        response.mark_changed();
    }

    let painter = ui.painter();
    // Track: line_control up to the cyan stop, then the sunset.
    let stops = [
        (0.0, t.line_control),
        (0.35, t.redline_0),
        (0.6, t.redline_1),
        (0.8, t.redline_2),
        (0.92, t.redline_3),
        (1.0, t.redline_4),
    ];
    let mut mesh = Mesh::default();
    for (k, color) in stops {
        let color = if enabled { color } else { t.ink_disabled };
        let x = track.left() + track.width() * k;
        mesh.colored_vertex(Pos2::new(x, track.top()), color);
        mesh.colored_vertex(Pos2::new(x, track.bottom()), color);
    }
    for i in 0..(stops.len() as u32 - 1) {
        let a = i * 2;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    painter.add(egui::Shape::mesh(mesh));

    let x = track.left() + track.width() * f32::from(*value) / 100.0;
    let center = pos2(x, rect.center().y);
    painter.circle(
        center,
        thumb / 2.0,
        if enabled { t.ink } else { t.ink_disabled },
        Stroke::new(
            stroke::STROKE_INDICATOR,
            if enabled { t.cyan } else { t.line_control },
        ),
    );
    if response.hovered() && enabled {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    response
}
