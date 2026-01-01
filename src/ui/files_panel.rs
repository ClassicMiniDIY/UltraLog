//! Files panel - file management, loading, and file list.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::state::{LoadingState, SUPPORTED_EXTENSIONS};
use crate::ui::icons::draw_upload_icon;

impl UltraLogApp {
    /// Render the files panel content (called from side_panel.rs)
    pub fn render_files_panel_content(&mut self, ui: &mut egui::Ui) {
        // Show loading indicator
        if let LoadingState::Loading(filename) = &self.loading_state {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("Loading {}...", filename));
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
        }

        let is_loading = matches!(self.loading_state, LoadingState::Loading(_));

        // File list (if any files loaded)
        if !self.files.is_empty() {
            self.render_file_list(ui);

            ui.add_space(10.0);

            // Add more files button
            ui.add_enabled_ui(!is_loading, |ui| {
                self.render_add_file_button(ui);
            });
        } else if !is_loading {
            // Nice drop zone when no files loaded
            self.render_drop_zone_card(ui);
        }
    }

    /// Render the list of loaded files
    fn render_file_list(&mut self, ui: &mut egui::Ui) {
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

        ui.label(
            egui::RichText::new(format!("Loaded Files ({})", file_info.len()))
                .size(self.scaled_font(13.0))
                .color(egui::Color32::GRAY),
        );
        ui.add_space(4.0);

        for (i, (file_name, is_selected, ecu_name, channel_count, data_count)) in
            file_info.iter().enumerate()
        {
            // File card with selection highlight
            let card_bg = if *is_selected {
                egui::Color32::from_rgb(50, 55, 45) // Subtle olive tint for selected
            } else {
                egui::Color32::from_rgb(40, 40, 40)
            };

            let card_border = if *is_selected {
                egui::Color32::from_rgb(113, 120, 78) // Olive green for selected
            } else {
                egui::Color32::from_rgb(55, 55, 55)
            };

            egui::Frame::NONE
                .fill(card_bg)
                .stroke(egui::Stroke::new(1.0, card_border))
                .corner_radius(6)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // File name (clickable)
                        let response = ui.add(
                            egui::Label::new(
                                egui::RichText::new(file_name)
                                    .size(self.scaled_font(14.0))
                                    .color(if *is_selected {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::LIGHT_GRAY
                                    }),
                            )
                            .sense(egui::Sense::click()),
                        );

                        if response.clicked() {
                            file_to_switch = Some(i);
                        }

                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        // Spacer
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Delete button
                            let close_btn = ui.add(
                                egui::Label::new(
                                    egui::RichText::new("x")
                                        .size(self.scaled_font(16.0))
                                        .color(egui::Color32::from_rgb(150, 150, 150)),
                                )
                                .sense(egui::Sense::click()),
                            );

                            if close_btn.clicked() {
                                file_to_remove = Some(i);
                            }

                            if close_btn.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        });
                    });

                    // ECU type and data info
                    ui.label(
                        egui::RichText::new(format!(
                            "{} • {} ch • {} pts",
                            ecu_name, channel_count, data_count
                        ))
                        .size(self.scaled_font(11.0))
                        .color(egui::Color32::GRAY),
                    );
                });

            ui.add_space(4.0);
        }

        // Handle deferred file switching
        if let Some(index) = file_to_switch {
            self.switch_to_file_tab(index);
        }

        if let Some(index) = file_to_remove {
            self.remove_file(index);
        }
    }

    /// Render the "Add File" button
    fn render_add_file_button(&mut self, ui: &mut egui::Ui) {
        let primary_color = egui::Color32::from_rgb(113, 120, 78); // Olive green

        let button_response = egui::Frame::NONE
            .fill(primary_color)
            .corner_radius(6)
            .inner_margin(egui::vec2(16.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("+")
                            .color(egui::Color32::WHITE)
                            .size(self.scaled_font(16.0)),
                    );
                    ui.label(
                        egui::RichText::new("Add File")
                            .color(egui::Color32::WHITE)
                            .size(self.scaled_font(14.0)),
                    );
                });
            });

        if button_response
            .response
            .interact(egui::Sense::click())
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Log Files", crate::state::SUPPORTED_EXTENSIONS)
                .pick_file()
            {
                self.start_loading_file(path);
            }
        }

        if button_response.response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    /// Render the drop zone card for when no files are loaded
    fn render_drop_zone_card(&mut self, ui: &mut egui::Ui) {
        let primary_color = egui::Color32::from_rgb(113, 120, 78); // Olive green
        let card_bg = egui::Color32::from_rgb(45, 45, 45);
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
                                    .size(self.scaled_font(14.0)),
                            );
                        });

                    if button_response
                        .response
                        .interact(egui::Sense::click())
                        .clicked()
                    {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Log Files", crate::state::SUPPORTED_EXTENSIONS)
                            .pick_file()
                        {
                            self.start_loading_file(path);
                        }
                    }

                    if button_response.response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    ui.add_space(12.0);

                    ui.label(
                        egui::RichText::new("or")
                            .color(text_gray)
                            .size(self.scaled_font(12.0)),
                    );

                    ui.add_space(8.0);

                    ui.label(
                        egui::RichText::new("Drop file here")
                            .color(egui::Color32::LIGHT_GRAY)
                            .size(self.scaled_font(13.0)),
                    );

                    ui.add_space(12.0);

                    let extensions_text = SUPPORTED_EXTENSIONS
                        .iter()
                        .map(|ext| ext.to_uppercase())
                        .collect::<Vec<_>>()
                        .join(" • ");
                    ui.label(
                        egui::RichText::new(extensions_text)
                            .color(text_gray)
                            .size(self.scaled_font(11.0)),
                    );
                });
            });
    }
}
