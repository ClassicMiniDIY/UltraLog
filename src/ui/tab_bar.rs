//! Tab bar UI for managing multiple log file views.

use eframe::egui;

use crate::app::UltraLogApp;

impl UltraLogApp {
    /// Render the tab bar for switching between log files
    pub fn render_tab_bar(&mut self, ui: &mut egui::Ui) {
        if self.tabs.is_empty() {
            return;
        }

        let mut tab_to_activate: Option<usize> = None;
        let mut tab_to_close: Option<usize> = None;

        // Collect tab info to avoid borrow issues
        let tab_info: Vec<(String, bool)> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| (tab.name.clone(), self.active_tab == Some(i)))
            .collect();

        ui.horizontal(|ui| {
            for (i, (name, is_active)) in tab_info.iter().enumerate() {
                let tab_color = if *is_active {
                    egui::Color32::from_rgb(60, 60, 60)
                } else {
                    egui::Color32::from_rgb(40, 40, 40)
                };

                let text_color = if *is_active {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::from_rgb(180, 180, 180)
                };

                let border_color = if *is_active {
                    egui::Color32::from_rgb(113, 120, 78) // Primary olive green
                } else {
                    egui::Color32::from_rgb(60, 60, 60)
                };

                egui::Frame::NONE
                    .fill(tab_color)
                    .corner_radius(egui::CornerRadius {
                        nw: 6,
                        ne: 6,
                        sw: 0,
                        se: 0,
                    })
                    .stroke(egui::Stroke::new(
                        if *is_active { 2.0 } else { 1.0 },
                        border_color,
                    ))
                    .inner_margin(egui::Margin {
                        left: 12,
                        right: 8,
                        top: 6,
                        bottom: 6,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Tab name (clickable)
                            let font_13 = self.scaled_font(13.0);
                            let label_response = ui.add(
                                egui::Label::new(
                                    egui::RichText::new(name).color(text_color).size(font_13),
                                )
                                .sense(egui::Sense::click()),
                            );

                            if label_response.clicked() {
                                tab_to_activate = Some(i);
                            }
                            if label_response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }

                            ui.add_space(4.0);

                            // Close button
                            let font_14 = self.scaled_font(14.0);
                            let close_btn = ui.add(
                                egui::Label::new(
                                    egui::RichText::new("x")
                                        .color(egui::Color32::from_rgb(150, 150, 150))
                                        .size(font_14),
                                )
                                .sense(egui::Sense::click()),
                            );

                            if close_btn.clicked() {
                                tab_to_close = Some(i);
                            }

                            if close_btn.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        });
                    });

                ui.add_space(2.0);
            }
        });

        // Handle deferred tab activation
        if let Some(index) = tab_to_activate {
            self.active_tab = Some(index);
            self.selected_file = Some(self.tabs[index].file_index);
        }

        // Handle deferred tab close
        if let Some(index) = tab_to_close {
            self.close_tab(index);
        }

        // Separator line under tabs
        ui.add_space(2.0);
        let rect = ui.available_rect_before_wrap();
        ui.painter().line_segment(
            [
                egui::pos2(rect.left(), rect.top()),
                egui::pos2(rect.right(), rect.top()),
            ],
            egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 60)),
        );
    }
}
