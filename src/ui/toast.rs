//! Toast notification system for user feedback.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::ui::theme::tokens::radius;

impl UltraLogApp {
    /// Render toast notifications in the bottom right corner
    pub fn render_toast(&mut self, ctx: &egui::Context) {
        if let Some((message, time, toast_type)) = &self.toast_message {
            if time.elapsed().as_secs() < 3 {
                let margin = 20.0;

                let t = self.tokens;
                let fill = toast_type.fill(&t);
                let accent = toast_type.accent(&t);
                let shadow = ctx.global_style().visuals.popup_shadow;

                egui::Area::new(egui::Id::new("toast"))
                    .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-margin, -margin))
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        egui::Frame::NONE
                            .fill(fill)
                            .stroke(egui::Stroke::new(1.0, accent))
                            .corner_radius(radius::RADIUS_MD)
                            .inner_margin(egui::Margin::symmetric(16, 12))
                            .shadow(shadow)
                            .show(ui, |ui| {
                                // Set min/max width for proper text wrapping
                                ui.set_min_width(200.0);
                                ui.set_max_width(400.0);
                                ui.label(
                                    egui::RichText::new(message)
                                        .color(t.ink)
                                        .text_style(egui::TextStyle::Body),
                                );
                            });
                    });
            } else {
                self.toast_message = None;
            }
        }
    }
}
