//! Floating tool windows and dialogs (`design/design-system/components/Window/README.md`).
//!
//! `bg_raised`, `line` border, `radius_lg` and the window shadow come from `Visuals`. This adds
//! the title in Saira 600, the 2px sunset redline along the top edge, and Esc to close.
//!
//! ```ignore
//! let mut open = self.show_thing;
//! let response = components::window(t!("thing.title")).open(&mut open).show(ctx, |ui| { /* … */ });
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

#[cfg(test)]
mod tests {
    use super::*;

    fn escape() -> egui::Event {
        egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// One frame showing the windows that are open; updates `open`, returns the library rect.
    fn frame(ctx: &egui::Context, open: &mut [bool; 2], events: Vec<egui::Event>) -> egui::Rect {
        let mut library = egui::Rect::NOTHING;
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            for (i, name) in ["Library", "Editor"].into_iter().enumerate() {
                if open[i] {
                    let mut o = true;
                    let r = window(name)
                        .order(egui::Order::Foreground)
                        .show(ui.ctx(), |ui| ui.label(name));
                    finish_window(ui.ctx(), r.as_ref().map(|r| &r.response), &mut o);
                    if i == 0
                        && let Some(r) = &r
                    {
                        library = r.response.rect;
                    }
                    open[i] = o;
                }
            }
        });
        output.textures_delta.clear();
        library
    }

    #[test]
    fn esc_closes_the_newest_window_first() {
        let ctx = egui::Context::default();
        // The window title uses the app's display family and theme text styles.
        crate::ui::fonts::install(&ctx, crate::i18n::Language::English);
        let tokens = crate::ui::theme::tokens_at(crate::ui::theme::ThemeSettings::default());
        crate::ui::theme::install(&ctx, &tokens, 1.0);
        let mut open = [true, false];
        let mut rect = egui::Rect::NOTHING;
        for _ in 0..3 {
            rect = frame(&ctx, &mut open, vec![]);
        }
        // The editor opens from a click inside the library (press, then release opens it): the
        // newer window is on top, and Esc closes it before the library.
        let at = rect.center();
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(
            &ctx,
            &mut open,
            vec![egui::Event::PointerMoved(at), button(true)],
        );
        open[1] = true;
        frame(&ctx, &mut open, vec![button(false)]);
        for _ in 0..3 {
            frame(&ctx, &mut open, vec![]);
        }
        frame(&ctx, &mut open, vec![escape()]);
        assert_eq!(open, [true, false], "first Esc closes the editor");
        frame(&ctx, &mut open, vec![]);
        frame(&ctx, &mut open, vec![escape()]);
        assert_eq!(open, [false, false], "second Esc closes the library");
    }
}
