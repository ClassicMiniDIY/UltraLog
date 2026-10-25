//! Toast notification system for user feedback.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::state::ToastType;
use crate::ui::components::Status;
use crate::ui::theme::tokens::radius;

/// How long a toast stays on screen.
const TOAST_DURATION: std::time::Duration = std::time::Duration::from_secs(3);

impl UltraLogApp {
    /// Render toast notifications in the bottom right corner
    pub fn render_toast(&mut self, ctx: &egui::Context) {
        if let Some((message, time, toast_type)) = &self.toast_message {
            let shown_for = time.elapsed();
            if shown_for < TOAST_DURATION {
                // Wake up to take the toast down even if nothing else repaints.
                ctx.request_repaint_after(TOAST_DURATION - shown_for);
                let margin = 20.0;

                let t = self.tokens;
                let fill = toast_type.fill(&t);
                let accent = toast_type.accent(&t);
                let shadow = ctx.global_style().visuals.popup_shadow;

                egui::Area::new(egui::Id::new("toast"))
                    .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-margin, -margin))
                    // Above windows and not interactable, so a toast never takes Esc or clicks
                    // from the window under it.
                    .order(egui::Order::Tooltip)
                    .interactable(false)
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
                                let status = match toast_type {
                                    ToastType::Info => Status::Info,
                                    ToastType::Success => Status::Success,
                                    ToastType::Warning => Status::Warning,
                                    ToastType::Error => Status::Danger,
                                };
                                ui.horizontal_top(|ui| {
                                    ui.label(egui::RichText::new(status.icon()).color(accent));
                                    ui.label(
                                        egui::RichText::new(message)
                                            .color(t.ink)
                                            .text_style(egui::TextStyle::Body),
                                    );
                                });
                            });
                    });
            } else {
                self.toast_message = None;
            }
        }
    }
}
