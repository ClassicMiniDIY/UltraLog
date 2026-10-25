//! Floating tool windows and dialogs (`design/design-system/components/Window/README.md`).
//!
//! `bg_raised`, `line` border, `radius_lg` and the window shadow come from `Visuals`. This adds
//! the title in Saira 600, the 2px sunset redline along the top edge, and Esc to close.
//!
//! ```ignore
//! let mut open = self.show_thing;
//! let response = components::window(t!("thing.title")).open(&mut open).show(ctx, |ui| { … });
//! components::finish_window(ctx, response.as_ref().map(|r| &r.response), &mut open);
//! self.show_thing = open;
//! ```

use eframe::egui::{self, RichText};

use crate::ui::fonts;
use crate::ui::theme::{self, paint, tokens::radius};

/// An `egui::Window` with the Night Drive title. Chain the usual builder calls on it.
pub fn window<'open>(title: impl Into<String>) -> egui::Window<'open> {
    egui::Window::new(
        RichText::new(title.into())
            .text_style(egui::TextStyle::Heading)
            .family(egui::FontFamily::Name(fonts::DISPLAY.into())),
    )
}

/// Draw the redline on a shown window and close it on Esc when it is the topmost window.
pub fn finish_window(ctx: &egui::Context, response: Option<&egui::Response>, open: &mut bool) {
    let Some(response) = response else {
        return;
    };
    let t = theme::current(ctx);
    let painter = ctx.layer_painter(response.layer_id);
    paint::redline(
        &painter,
        response.rect,
        &t,
        f32::from(radius::RADIUS_LG) * 0.6,
    );

    // Topmost among *visible* layers of this order: egui keeps hidden layers (a toast that
    // faded, a closed popup or window) in its order list, so `top_layer_id` alone would leave
    // Esc dead after any of them had been shown.
    let topmost = ctx.memory(|m| {
        m.layer_ids()
            .filter(|l| l.order == response.layer_id.order && m.areas().is_visible(l))
            .last()
    }) == Some(response.layer_id);
    if *open && topmost && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        *open = false;
    }
}
