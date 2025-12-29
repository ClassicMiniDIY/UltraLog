//! Sidebar UI rendering - files panel and view options.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::state::{ActiveTool, LoadingState};
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
            let mut file_to_switch: Option<usize> = None;

            // Collect file info upfront to avoid borrow issues
            let file_info: Vec<(String, bool, String, usize, usize)> = self
                .files
                .iter()
                .enumerate()
                .map(|(i, file)| {
                    (
                        file.name.clone(),
                        self.selected_file == Some(i),
                        file.ecu_type.name().to_string(),
                        file.log.channels.len(),
                        file.log.data.len(),
                    )
                })
                .collect();

            for (i, (file_name, is_selected, ecu_name, channel_count, data_count)) in
                file_info.iter().enumerate()
            {
                ui.horizontal(|ui| {
                    let response = ui.selectable_label(*is_selected, file_name);
                    if response.clicked() {
                        file_to_switch = Some(i);
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
                            ecu_name, channel_count, data_count
                        ))
                        .size(12.0)
                        .color(egui::Color32::GRAY),
                    );
                });
            }

            // Handle deferred file switching
            if let Some(index) = file_to_switch {
                self.switch_to_file_tab(index);
            }

            if let Some(index) = file_to_remove {
                self.remove_file(index);
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(5.0);

            // Add more files button (styled to match drop zone button)
            ui.add_enabled_ui(!is_loading, |ui| {
                let primary_color = egui::Color32::from_rgb(113, 120, 78); // Olive green

                ui.vertical_centered(|ui| {
                    let button_response = egui::Frame::NONE
                        .fill(primary_color)
                        .corner_radius(6)
                        .inner_margin(egui::vec2(16.0, 8.0))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("+ Add File")
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
                            .add_filter(
                                "Log Files",
                                &["csv", "log", "txt", "mlg", "llg", "xrk", "drk"],
                            )
                            .pick_file()
                        {
                            self.start_loading_file(path);
                        }
                    }

                    if button_response.response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                });
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
        egui::Frame::NONE
            .fill(card_bg)
            .corner_radius(12)
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
                    let button_response = egui::Frame::NONE
                        .fill(primary_color)
                        .corner_radius(6)
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
                            .add_filter("Log Files", &["csv", "mlg", "llg", "xrk", "drk"])
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
                        egui::RichText::new("CSV • LOG • TXT • MLG")
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

            // Only show options when we have data to view and are in Log Viewer mode
            if !self.files.is_empty()
                && !self.get_selected_channels().is_empty()
                && self.active_tool == ActiveTool::LogViewer
            {
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(35, 35, 35))
                    .corner_radius(8)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        // Cursor tracking checkbox
                        ui.checkbox(&mut self.cursor_tracking, "🎯  Cursor Tracking");
                        ui.label(
                            egui::RichText::new("Keep cursor centered while scrubbing")
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
                        ui.checkbox(&mut self.color_blind_mode, "👁  Color Blind Mode");
                        ui.label(
                            egui::RichText::new("Use accessible color palette")
                                .color(egui::Color32::GRAY),
                        );

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Field normalization checkbox with right-aligned Edit button
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.field_normalization, "📝  Field Normalization");
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.small_button("Edit").clicked() {
                                        self.show_normalization_editor = true;
                                    }
                                },
                            );
                        });
                        ui.label(
                            egui::RichText::new("Standardize channel names across ECU types")
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
