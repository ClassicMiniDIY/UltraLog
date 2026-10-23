//! Button variants (`design/design-system/components/Button/README.md`).
//!
//! - **primary**: `cyan` fill, `on_cyan` label, glow on hover. One per view.
//! - **secondary**: `bg_raised` with a `line_control` border. Everything else.
//! - **ghost**: no frame until hovered. Low-weight actions beside a primary.
//! - **danger**: outlined in `danger`, never filled. Removes data the user loaded.
//!
//! Secondary and ghost take their colours from `Visuals`; primary and danger set them here.

use eframe::egui::{self, Color32, CornerRadius, Response, RichText, Shape, Stroke, Ui, Vec2};

use crate::ui::theme::{self, paint, tokens::radius};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

/// Regular button height (px).
const HEIGHT: f32 = 32.0;
/// `sm` button height for dense rows (px).
const HEIGHT_SM: f32 = 24.0;
/// Icon button size (px).
const ICON_SIZE: f32 = 28.0;

/// A button of `kind` with an optional leading Phosphor icon. Disable it by wrapping the call
/// in `ui.add_enabled_ui`.
pub fn button(ui: &mut Ui, kind: Kind, icon: Option<&str>, label: impl Into<String>) -> Response {
    show(ui, kind, icon, label.into(), HEIGHT)
}

/// The `sm` (24px) size for dense rows: library items, results toolbars.
pub fn button_sm(
    ui: &mut Ui,
    kind: Kind,
    icon: Option<&str>,
    label: impl Into<String>,
) -> Response {
    show(ui, kind, icon, label.into(), HEIGHT_SM)
}

/// A 28px square ghost button holding one icon. `hover_text` names the action.
pub fn icon_button(ui: &mut Ui, icon: &str, hover_text: impl Into<String>) -> Response {
    let t = theme::current(ui.ctx());
    ui.add(
        egui::Button::new(RichText::new(icon).size(16.0).color(t.ink_muted))
            .frame_when_inactive(false)
            .corner_radius(CornerRadius::same(radius::RADIUS_MD))
            .min_size(Vec2::splat(ICON_SIZE)),
    )
    .on_hover_text(hover_text.into())
}

fn show(ui: &mut Ui, kind: Kind, icon: Option<&str>, label: String, height: f32) -> Response {
    let t = theme::current(ui.ctx());
    let text = match icon {
        Some(icon) => format!("{icon}  {label}"),
        None => label,
    };
    let text = RichText::new(text).text_style(egui::TextStyle::Button);
    let corner = CornerRadius::same(radius::RADIUS_MD);
    // Reserve a slot under the button so the hover glow paints behind it.
    let glow_slot = ui.painter().add(Shape::Noop);

    let button = match kind {
        Kind::Primary => egui::Button::new(text.color(t.on_cyan)).fill(t.cyan),
        Kind::Secondary => egui::Button::new(text.color(t.ink)),
        Kind::Ghost => egui::Button::new(text.color(t.ink_muted)).frame_when_inactive(false),
        Kind::Danger => egui::Button::new(text.color(t.danger))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::new(1.0, t.danger)),
    };
    let response = ui.add(
        button
            .corner_radius(corner)
            .min_size(Vec2::new(0.0, height)),
    );

    if kind == Kind::Primary
        && response.hovered()
        && ui.is_enabled()
        && let Some(glow) = paint::glow(&t, response.rect, radius::RADIUS_MD, t.cyan)
    {
        ui.painter().set(glow_slot, glow);
    }
    if response.hovered() && ui.is_enabled() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}
