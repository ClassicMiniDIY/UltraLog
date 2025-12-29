//! Field Normalization Editor UI.
//!
//! Provides a window for users to view and customize field name mappings.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::normalize::get_builtin_mappings;

impl UltraLogApp {
    /// Render the field normalization editor window
    pub fn render_normalization_editor(&mut self, ctx: &egui::Context) {
        if !self.show_normalization_editor {
            return;
        }

        let mut open = true;

        egui::Window::new("Field Normalization Editor")
            .open(&mut open)
            .resizable(true)
            .default_width(550.0)
            .default_height(500.0)
            .order(egui::Order::Foreground) // Ensure window is on top of chart overlays
            .show(ctx, |ui| {
                // Header with reset button
                ui.horizontal(|ui| {
                    ui.heading("Field Name Mappings");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if !self.custom_normalizations.is_empty()
                            && ui.button("Reset to Defaults").clicked()
                        {
                            self.custom_normalizations.clear();
                            self.norm_editor_extend_source.clear();
                            self.norm_editor_selected_target = None;
                            self.norm_editor_custom_source.clear();
                            self.norm_editor_custom_target.clear();
                        }
                    });
                });
                ui.add_space(4.0);

                // --- Extend Built-in Mappings Section ---
                ui.separator();
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Extend Built-in Mappings").strong());
                ui.label(
                    egui::RichText::new(
                        "Add your own source names that map to existing normalized field names.",
                    )
                    .color(egui::Color32::GRAY),
                );
                ui.add_space(8.0);

                // Get built-in mapping names for the dropdown
                let builtin_mappings = get_builtin_mappings();
                let builtin_names: Vec<&str> = builtin_mappings.iter().map(|(n, _)| *n).collect();

                ui.horizontal(|ui| {
                    ui.label("Source Name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.norm_editor_extend_source)
                            .hint_text("e.g., MyCustomRPM")
                            .desired_width(150.0),
                    );
                    ui.label("→");
                    ui.label("Maps To:");

                    // Dropdown for selecting existing normalized name
                    let selected_text = self
                        .norm_editor_selected_target
                        .as_deref()
                        .unwrap_or("Select...");
                    egui::ComboBox::from_id_salt("extend_builtin_combo")
                        .selected_text(selected_text)
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            for name in &builtin_names {
                                let is_selected = self.norm_editor_selected_target.as_deref() == Some(*name);
                                if ui.selectable_label(is_selected, *name).clicked() {
                                    self.norm_editor_selected_target = Some(name.to_string());
                                }
                            }
                        });

                    if ui.button("Add").clicked()
                        && !self.norm_editor_extend_source.is_empty()
                    {
                        if let Some(target) = &self.norm_editor_selected_target {
                            self.custom_normalizations.insert(
                                self.norm_editor_extend_source.to_lowercase(),
                                target.clone(),
                            );
                            self.norm_editor_extend_source.clear();
                        }
                    }
                });

                ui.add_space(12.0);

                // --- Custom Mappings Section ---
                ui.separator();
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Create New Mapping").strong());
                ui.label(
                    egui::RichText::new(
                        "Define completely new normalized names for your custom channels.",
                    )
                    .color(egui::Color32::GRAY),
                );
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("Source Name:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.norm_editor_custom_source)
                            .hint_text("e.g., Sensor_XYZ")
                            .desired_width(150.0),
                    );
                    ui.label("→");
                    ui.label("Display As:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.norm_editor_custom_target)
                            .hint_text("e.g., Custom Sensor")
                            .desired_width(150.0),
                    );
                    if ui.button("Add").clicked()
                        && !self.norm_editor_custom_source.is_empty()
                        && !self.norm_editor_custom_target.is_empty()
                    {
                        self.custom_normalizations.insert(
                            self.norm_editor_custom_source.to_lowercase(),
                            self.norm_editor_custom_target.clone(),
                        );
                        self.norm_editor_custom_source.clear();
                        self.norm_editor_custom_target.clear();
                    }
                });

                ui.add_space(12.0);

                // --- Your Custom Mappings ---
                if !self.custom_normalizations.is_empty() {
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Your Custom Mappings").strong());
                        ui.label(
                            egui::RichText::new(format!("({})", self.custom_normalizations.len()))
                                .color(egui::Color32::GRAY),
                        );
                    });
                    ui.add_space(4.0);

                    let mut to_remove: Option<String> = None;

                    egui::ScrollArea::vertical()
                        .id_salt("custom_mappings_scroll")
                        .max_height(120.0)
                        .show(ui, |ui| {
                            egui::Grid::new("custom_mappings_grid")
                                .striped(true)
                                .num_columns(3)
                                .min_col_width(100.0)
                                .spacing([16.0, 8.0])
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new("Source").strong());
                                    ui.label(egui::RichText::new("Display As").strong());
                                    ui.label("");
                                    ui.end_row();

                                    // Sort by target name for better organization
                                    let mut sorted: Vec<_> = self.custom_normalizations.iter().collect();
                                    sorted.sort_by(|a, b| a.1.cmp(b.1));

                                    for (source, target) in sorted {
                                        ui.label(source);
                                        ui.label(
                                            egui::RichText::new(target)
                                                .color(egui::Color32::LIGHT_BLUE),
                                        );
                                        if ui.small_button("Remove").clicked() {
                                            to_remove = Some(source.clone());
                                        }
                                        ui.end_row();
                                    }
                                });
                        });

                    if let Some(key) = to_remove {
                        self.custom_normalizations.remove(&key);
                    }
                }

                ui.add_space(12.0);
                ui.separator();

                // Built-in mappings reference (collapsible)
                egui::CollapsingHeader::new("Built-in Mappings Reference")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(
                                "These are the default mappings. You can extend them by adding new source names above.",
                            )
                            .color(egui::Color32::GRAY),
                        );
                        ui.add_space(8.0);

                        egui::ScrollArea::vertical()
                            .id_salt("builtin_mappings_scroll")
                            .max_height(200.0)
                            .show(ui, |ui| {
                                for (normalized, sources) in &builtin_mappings {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            egui::RichText::new(*normalized)
                                                .strong()
                                                .color(egui::Color32::LIGHT_BLUE),
                                        );
                                        ui.label("←");
                                        ui.label(
                                            egui::RichText::new(sources.join(", "))
                                                .color(egui::Color32::GRAY),
                                        );
                                    });
                                    ui.add_space(2.0);
                                }
                            });
                    });
            });

        if !open {
            self.show_normalization_editor = false;
        }
    }
}
