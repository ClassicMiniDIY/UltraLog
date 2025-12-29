//! Formula Editor UI.
//!
//! Provides a modal window for creating and editing computed channel formulas.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::computed::ComputedChannelTemplate;
use crate::expression::{
    build_channel_bindings, extract_channel_references, generate_preview, validate_formula,
};

impl UltraLogApp {
    /// Render the formula editor dialog
    pub fn render_formula_editor(&mut self, ctx: &egui::Context) {
        if !self.formula_editor_state.is_open {
            return;
        }

        let mut open = true;
        let mut should_save = false;

        let title = if self.formula_editor_state.is_editing() {
            "Edit Computed Channel"
        } else {
            "New Computed Channel"
        };

        egui::Window::new(title)
            .open(&mut open)
            .resizable(true)
            .default_width(500.0)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.add_space(4.0);

                // Name field
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.formula_editor_state.name)
                            .hint_text("e.g., RPM Delta")
                            .desired_width(300.0),
                    );
                });

                ui.add_space(8.0);

                // Formula field
                ui.label("Formula:");
                let formula_response = ui.add(
                    egui::TextEdit::multiline(&mut self.formula_editor_state.formula)
                        .hint_text("e.g., RPM - RPM[-1]")
                        .desired_width(ui.available_width())
                        .desired_rows(3)
                        .font(egui::TextStyle::Monospace),
                );

                // Validate on formula change
                if formula_response.changed() {
                    self.validate_current_formula();
                }

                // Show validation status
                ui.add_space(4.0);
                if let Some(error) = &self.formula_editor_state.validation_error {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Error:").color(egui::Color32::RED));
                        ui.label(egui::RichText::new(error).color(egui::Color32::RED).small());
                    });
                } else if !self.formula_editor_state.formula.is_empty() {
                    ui.label(egui::RichText::new("Formula valid").color(egui::Color32::GREEN));
                }

                ui.add_space(8.0);

                // Unit field
                ui.horizontal(|ui| {
                    ui.label("Unit:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.formula_editor_state.unit)
                            .hint_text("e.g., RPM/s")
                            .desired_width(150.0),
                    );
                });

                ui.add_space(8.0);

                // Description field
                ui.label("Description (optional):");
                ui.add(
                    egui::TextEdit::multiline(&mut self.formula_editor_state.description)
                        .hint_text("What does this computed channel calculate?")
                        .desired_width(ui.available_width())
                        .desired_rows(2),
                );

                ui.add_space(8.0);

                // Channel browser
                if self.active_tab.is_some() {
                    egui::CollapsingHeader::new("Available Channels")
                        .default_open(false)
                        .show(ui, |ui| {
                            let channels = self.get_available_channel_names();
                            if channels.is_empty() {
                                ui.label(
                                    egui::RichText::new("No channels available")
                                        .color(egui::Color32::GRAY),
                                );
                            } else {
                                egui::ScrollArea::vertical()
                                    .id_salt("channel_browser")
                                    .max_height(120.0)
                                    .show(ui, |ui| {
                                        ui.horizontal_wrapped(|ui| {
                                            for name in &channels {
                                                if ui.small_button(name).clicked() {
                                                    // Insert channel name into formula
                                                    let insert = if name.contains(' ') {
                                                        format!("\"{}\"", name)
                                                    } else {
                                                        name.clone()
                                                    };
                                                    self.formula_editor_state
                                                        .formula
                                                        .push_str(&insert);
                                                    self.validate_current_formula();
                                                }
                                            }
                                        });
                                    });
                            }
                        });
                }

                // Preview section
                if let Some(preview_values) = &self.formula_editor_state.preview_values {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.label(egui::RichText::new("Preview (first 5 values):").strong());
                    ui.horizontal(|ui| {
                        for (i, val) in preview_values.iter().take(5).enumerate() {
                            if i > 0 {
                                ui.label(",");
                            }
                            ui.label(
                                egui::RichText::new(format!("{:.2}", val))
                                    .monospace()
                                    .color(egui::Color32::LIGHT_GREEN),
                            );
                        }
                    });
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                // Buttons
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        self.formula_editor_state.close();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let can_save = !self.formula_editor_state.name.is_empty()
                            && !self.formula_editor_state.formula.is_empty()
                            && self.formula_editor_state.validation_error.is_none();

                        ui.add_enabled_ui(can_save, |ui| {
                            if ui.button("Save").clicked() {
                                should_save = true;
                            }
                        });

                        // Validate button
                        if ui.button("Validate").clicked() {
                            self.validate_current_formula();
                        }
                    });
                });
            });

        // Handle save action (outside of window closure to avoid borrow issues)
        if should_save {
            self.save_formula_editor();
        }

        if !open {
            self.formula_editor_state.close();
        }
    }

    /// Validate the current formula in the editor
    fn validate_current_formula(&mut self) {
        let formula = self.formula_editor_state.formula.clone();

        if formula.is_empty() {
            self.formula_editor_state.validation_error = None;
            self.formula_editor_state.preview_values = None;
            return;
        }

        // Get available channels
        let available_channels = self.get_available_channel_names();

        // Validate the formula
        match validate_formula(&formula, &available_channels) {
            Ok(()) => {
                self.formula_editor_state.validation_error = None;

                // Generate preview if we have data
                if let Some(tab_idx) = self.active_tab {
                    let file_idx = self.tabs[tab_idx].file_index;
                    if file_idx < self.files.len() {
                        let file = &self.files[file_idx];
                        let refs = extract_channel_references(&formula);
                        if let Ok(bindings) = build_channel_bindings(&refs, &available_channels) {
                            if let Ok(preview) = generate_preview(
                                &formula,
                                &bindings,
                                &file.log.data,
                                &file.log.times,
                                5,
                            ) {
                                self.formula_editor_state.preview_values = Some(preview);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                self.formula_editor_state.validation_error = Some(e);
                self.formula_editor_state.preview_values = None;
            }
        }
    }

    /// Save the formula from the editor to the library
    fn save_formula_editor(&mut self) {
        let state = &self.formula_editor_state;

        if state.is_editing() {
            // Update existing template
            if let Some(id) = &state.editing_template_id {
                if let Some(template) = self.computed_library.find_template_mut(id) {
                    template.name = state.name.clone();
                    template.formula = state.formula.clone();
                    template.unit = state.unit.clone();
                    template.description = state.description.clone();
                    template.touch();
                }
            }
        } else {
            // Create new template
            let template = ComputedChannelTemplate::new(
                state.name.clone(),
                state.formula.clone(),
                state.unit.clone(),
                state.description.clone(),
            );
            self.computed_library.add_template(template);
        }

        // Save library
        if let Err(e) = self.computed_library.save() {
            self.show_toast_error(&format!("Failed to save: {}", e));
        } else {
            self.show_toast_success("Channel saved to library");
        }

        self.formula_editor_state.close();
    }
}
