//! Sidebar UI rendering - files panel and view options.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::state::LoadingState;
use crate::ui::icons::draw_upload_icon;

impl UltraLogApp {
    /// Render the left sidebar with file list and view options
    pub fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("Files");
        ui.separator();

        // Show loading indicator
        if let LoadingState::Loading(filename) = &self.loading_state {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("Loading {}...", filename));
            });
            ui.separator();
        }

        let is_loading = matches!(self.loading_state, LoadingState::Loading(_));

        // File list (if any files loaded)
        if !self.files.is_empty() {
            let mut file_to_remove: Option<usize> = None;
            for (i, file) in self.files.iter().enumerate() {
                let is_selected = self.selected_file == Some(i);

                ui.horizontal(|ui| {
                    let response = ui.selectable_label(is_selected, &file.name);
                    if response.clicked() {
                        self.selected_file = Some(i);
                    }

                    // Delete button
                    if ui.small_button("x").clicked() {
                        file_to_remove = Some(i);
                    }
                });

                // Show ECU type and data info
                ui.indent(format!("file_indent_{}", i), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} | {} channels | {} points",
                            file.ecu_type.name(),
                            file.log.channels.len(),
                            file.log.data.len()
                        ))
                        .size(12.0)
                        .color(egui::Color32::GRAY),
                    );
                });
            }

            if let Some(index) = file_to_remove {
                self.remove_file(index);
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(5.0);

            // Add more files button (compact when files exist)
            ui.add_enabled_ui(!is_loading, |ui| {
                if ui.button("+ Add File").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter(
                            "Log Files",
                            &["csv", "log", "txt", "mlg", "hlgzip", "daq", "llg"],
                        )
                        .pick_file()
                    {
                        self.start_loading_file(path);
                    }
                }
            });
        } else if !is_loading {
            // Nice drop zone when no files loaded
            self.render_drop_zone(ui);
        }

        // View Options section at bottom
        self.render_view_options(ui);
    }

    /// Render the drop zone for when no files are loaded
    fn render_drop_zone(&mut self, ui: &mut egui::Ui) {
        let primary_color = egui::Color32::from_rgb(113, 120, 78); // Olive green
        let card_bg = egui::Color32::from_rgb(45, 45, 45); // Dark card for dark theme
        let text_gray = egui::Color32::from_rgb(150, 150, 150);

        ui.add_space(20.0);

        // Drop zone card
        egui::Frame::none()
            .fill(card_bg)
            .rounding(12.0)
            .inner_margin(20.0)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    // Upload icon
                    let icon_size = 32.0;
                    let (icon_rect, _) = ui.allocate_exact_size(
                        egui::vec2(icon_size, icon_size),
                        egui::Sense::hover(),
                    );
                    draw_upload_icon(ui, icon_rect.center(), icon_size, primary_color);

                    ui.add_space(12.0);

                    // Select file button
                    let button_response = egui::Frame::none()
                        .fill(primary_color)
                        .rounding(6.0)
                        .inner_margin(egui::vec2(16.0, 8.0))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("Select a file")
                                    .color(egui::Color32::WHITE)
                                    .size(14.0),
                            );
                        });

                    if button_response
                        .response
                        .interact(egui::Sense::click())
                        .clicked()
                    {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Log Files", &["csv", "mlg"])
                            .pick_file()
                        {
                            self.start_loading_file(path);
                        }
                    }

                    if button_response.response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    ui.add_space(12.0);

                    ui.label(egui::RichText::new("or").color(text_gray).size(12.0));

                    ui.add_space(8.0);

                    ui.label(
                        egui::RichText::new("Drop file here")
                            .color(egui::Color32::LIGHT_GRAY)
                            .size(13.0),
                    );

                    ui.add_space(12.0);

                    ui.label(
                        egui::RichText::new("CSV • LOG • TXT • MLG • HLGZIP • DAQ • LLG")
                            .color(text_gray)
                            .size(11.0),
                    );
                });
            });
    }

    /// Render view options at the bottom of the sidebar
    fn render_view_options(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            // Reverse order since we're bottom-up
            ui.add_space(10.0);

            // Only show options when we have data to view
            if !self.files.is_empty() && !self.selected_channels.is_empty() {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(35, 35, 35))
                    .rounding(8.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        // Cursor tracking checkbox
                        ui.checkbox(&mut self.cursor_tracking, "Cursor Tracking");
                        ui.label(
                            egui::RichText::new("Keep cursor centered while scrubbing")
                                .small()
                                .color(egui::Color32::GRAY),
                        );

                        // Window size slider (only show when cursor tracking is enabled)
                        if self.cursor_tracking {
                            ui.add_space(8.0);
                            ui.label("View Window:");
                            ui.add(
                                egui::Slider::new(&mut self.view_window_seconds, 5.0..=120.0)
                                    .suffix("s")
                                    .logarithmic(true),
                            );
                        }

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Color blind mode checkbox
                        ui.checkbox(&mut self.color_blind_mode, "Color Blind Mode");
                        ui.label(
                            egui::RichText::new("Use accessible color palette")
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                    });

                ui.add_space(5.0);
                ui.separator();
                ui.heading("View Options");
            }
        });
    }
}
