//! Computed Channels Manager UI.
//!
//! Provides a window for users to manage their computed channel library
//! and apply computed channels to the active log file, including quick templates
//! and anomaly detection channels.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::computed::{ComputedChannel, ComputedChannelTemplate};
use crate::expression::{
    build_channel_bindings, compute_all_channel_statistics, evaluate_all_records,
    evaluate_all_records_with_stats, extract_channel_references,
};
use crate::parsers::types::ComputedChannelInfo;
use crate::parsers::Channel;
use crate::state::{SelectedChannel, CHART_COLORS};

impl UltraLogApp {
    /// Render the computed channels manager window
    pub fn render_computed_channels_manager(&mut self, ctx: &egui::Context) {
        if !self.show_computed_channels_manager {
            return;
        }

        let mut open = true;

        egui::Window::new("Computed Channels")
            .open(&mut open)
            .resizable(true)
            .default_width(650.0)
            .default_height(550.0)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // Header with Add button
                ui.horizontal(|ui| {
                    ui.heading("Channel Library");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("+ New Channel").clicked() {
                            self.formula_editor_state.open_new();
                        }
                    });
                });

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Create reusable computed channels from mathematical formulas.",
                    )
                    .color(egui::Color32::GRAY),
                );
                ui.add_space(8.0);

                ui.separator();
                ui.add_space(4.0);

                // Library templates section
                if self.computed_library.templates.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            egui::RichText::new("No computed channels yet")
                                .color(egui::Color32::GRAY),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(
                                "Click '+ New Channel' to create your first computed channel.",
                            )
                            .color(egui::Color32::GRAY)
                            .small(),
                        );
                        ui.add_space(20.0);
                    });
                } else {
                    ui.label(
                        egui::RichText::new(format!(
                            "Templates ({})",
                            self.computed_library.templates.len()
                        ))
                        .strong(),
                    );
                    ui.add_space(4.0);

                    // Collect actions to perform after rendering (avoid borrow issues)
                    let mut template_to_edit: Option<String> = None;
                    let mut template_to_delete: Option<String> = None;
                    let mut template_to_apply: Option<ComputedChannelTemplate> = None;

                    // Group templates by category
                    let categories: Vec<String> = {
                        let mut cats: Vec<String> = self
                            .computed_library
                            .templates
                            .iter()
                            .map(|t| {
                                if t.category.is_empty() {
                                    "Custom".to_string()
                                } else {
                                    t.category.clone()
                                }
                            })
                            .collect();
                        cats.sort();
                        cats.dedup();
                        // Put common categories in a specific order
                        let order = ["Rate", "Engine", "Smoothing", "Anomaly", "Custom"];
                        cats.sort_by_key(|c| {
                            order
                                .iter()
                                .position(|&o| o == c)
                                .unwrap_or(order.len())
                        });
                        cats
                    };

                    egui::ScrollArea::vertical()
                        .id_salt("library_templates_scroll")
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for category in &categories {
                                let cat_templates: Vec<&ComputedChannelTemplate> = self
                                    .computed_library
                                    .templates
                                    .iter()
                                    .filter(|t| {
                                        let t_cat = if t.category.is_empty() {
                                            "Custom"
                                        } else {
                                            &t.category
                                        };
                                        t_cat == category
                                    })
                                    .collect();

                                if cat_templates.is_empty() {
                                    continue;
                                }

                                // Category header with color
                                let cat_color = match category.as_str() {
                                    "Rate" => egui::Color32::from_rgb(100, 180, 255),
                                    "Engine" => egui::Color32::from_rgb(255, 180, 100),
                                    "Smoothing" => egui::Color32::from_rgb(180, 255, 100),
                                    "Anomaly" => egui::Color32::from_rgb(255, 100, 100),
                                    _ => egui::Color32::GRAY,
                                };

                                egui::CollapsingHeader::new(
                                    egui::RichText::new(format!(
                                        "{} ({})",
                                        category,
                                        cat_templates.len()
                                    ))
                                    .color(cat_color),
                                )
                                .default_open(true)
                                .show(ui, |ui| {
                                    for template in cat_templates {
                                        egui::Frame::NONE
                                            .fill(egui::Color32::from_rgb(50, 50, 50))
                                            .corner_radius(5.0)
                                            .inner_margin(egui::Margin::symmetric(10, 8))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    // Template info
                                                    ui.vertical(|ui| {
                                                        ui.horizontal(|ui| {
                                                            // Built-in indicator
                                                            if template.is_builtin {
                                                                ui.label(
                                                                    egui::RichText::new("★")
                                                                        .color(egui::Color32::GOLD),
                                                                );
                                                            }
                                                            ui.label(
                                                                egui::RichText::new(&template.name)
                                                                    .strong()
                                                                    .color(egui::Color32::LIGHT_BLUE),
                                                            );
                                                            if !template.unit.is_empty() {
                                                                ui.label(
                                                                    egui::RichText::new(format!(
                                                                        "({})",
                                                                        template.unit
                                                                    ))
                                                                    .small()
                                                                    .color(egui::Color32::GRAY),
                                                                );
                                                            }
                                                        });
                                                        ui.label(
                                                            egui::RichText::new(&template.formula)
                                                                .monospace()
                                                                .small()
                                                                .color(egui::Color32::from_rgb(
                                                                    180, 180, 180,
                                                                )),
                                                        );
                                                        if !template.description.is_empty() {
                                                            ui.label(
                                                                egui::RichText::new(
                                                                    &template.description,
                                                                )
                                                                .small()
                                                                .color(egui::Color32::GRAY),
                                                            );
                                                        }
                                                    });

                                                    // Buttons on the right
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            if ui.small_button("Delete").clicked() {
                                                                template_to_delete =
                                                                    Some(template.id.clone());
                                                            }
                                                            if ui.small_button("Edit").clicked() {
                                                                template_to_edit =
                                                                    Some(template.id.clone());
                                                            }
                                                            if self.active_tab.is_some()
                                                                && ui
                                                                    .button(
                                                                        egui::RichText::new("Apply")
                                                                            .color(
                                                                                egui::Color32::WHITE,
                                                                            ),
                                                                    )
                                                                    .clicked()
                                                            {
                                                                template_to_apply =
                                                                    Some((*template).clone());
                                                            }
                                                        },
                                                    );
                                                });
                                            });
                                        ui.add_space(4.0);
                                    }
                                });
                            }
                        });

                    // Process actions after rendering
                    if let Some(id) = template_to_edit {
                        if let Some(template) = self.computed_library.find_template(&id) {
                            self.formula_editor_state.open_edit(template);
                        }
                    }

                    if let Some(id) = template_to_delete {
                        self.computed_library.remove_template(&id);
                        let _ = self.computed_library.save();
                    }

                    if let Some(ref template) = template_to_apply {
                        self.apply_computed_channel_template(template);
                    }
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(4.0);

                // Applied channels section (for current file)
                if let Some(tab_idx) = self.active_tab {
                    let file_idx = self.tabs[tab_idx].file_index;
                    let applied_count = self
                        .file_computed_channels
                        .get(&file_idx)
                        .map(|c| c.len())
                        .unwrap_or(0);

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Applied to Current File ({})", applied_count))
                                .strong(),
                        );
                    });
                    ui.add_space(4.0);

                    if applied_count == 0 {
                        ui.label(
                            egui::RichText::new(
                                "No computed channels applied to this file. Click 'Apply' on a template above.",
                            )
                            .color(egui::Color32::GRAY)
                            .small(),
                        );
                    } else {
                        let mut channel_to_remove: Option<usize> = None;
                        let mut channel_to_select: Option<usize> = None;

                        if let Some(channels) = self.file_computed_channels.get(&file_idx) {
                            egui::ScrollArea::vertical()
                                .id_salt("applied_channels_scroll")
                                .max_height(150.0)
                                .show(ui, |ui| {
                                    for (idx, channel) in channels.iter().enumerate() {
                                        egui::Frame::NONE
                                            .fill(egui::Color32::from_rgb(40, 50, 40))
                                            .corner_radius(5.0)
                                            .inner_margin(egui::Margin::symmetric(10, 6))
                                            .show(ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    // Status indicator
                                                    if channel.is_valid() {
                                                        ui.label(
                                                            egui::RichText::new("●")
                                                                .color(egui::Color32::GREEN),
                                                        );
                                                    } else {
                                                        ui.label(
                                                            egui::RichText::new("●")
                                                                .color(egui::Color32::RED),
                                                        );
                                                    }

                                                    ui.label(
                                                        egui::RichText::new(channel.name())
                                                            .color(egui::Color32::LIGHT_GREEN),
                                                    );

                                                    if let Some(error) = &channel.error {
                                                        ui.label(
                                                            egui::RichText::new(format!(
                                                                "Error: {}",
                                                                error
                                                            ))
                                                            .small()
                                                            .color(egui::Color32::RED),
                                                        );
                                                    }

                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            if ui.small_button("Remove").clicked() {
                                                                channel_to_remove = Some(idx);
                                                            }
                                                            if channel.is_valid()
                                                                && ui.small_button("Add to Chart").clicked()
                                                            {
                                                                channel_to_select = Some(idx);
                                                            }
                                                        },
                                                    );
                                                });
                                            });
                                        ui.add_space(2.0);
                                    }
                                });
                        }

                        if let Some(idx) = channel_to_remove {
                            self.remove_computed_channel(idx);
                        }

                        if let Some(idx) = channel_to_select {
                            self.add_computed_channel_to_chart(idx);
                        }
                    }
                } else {
                    ui.label(
                        egui::RichText::new("Load a log file to apply computed channels.")
                            .color(egui::Color32::GRAY),
                    );
                }

                // Examples section
                ui.add_space(12.0);
                ui.separator();

                egui::CollapsingHeader::new("Example Formulas")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Common computed channel examples:")
                                .color(egui::Color32::GRAY),
                        );
                        ui.add_space(4.0);

                        // Rate of Change examples
                        ui.label(egui::RichText::new("Rate of Change:").strong());
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("  RPM - RPM[-1]")
                                    .monospace()
                                    .color(egui::Color32::LIGHT_GREEN),
                            );
                            ui.label(
                                egui::RichText::new("— RPM change per sample")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("  TPS - TPS@-0.1s")
                                    .monospace()
                                    .color(egui::Color32::LIGHT_GREEN),
                            );
                            ui.label(
                                egui::RichText::new("— TPS change over 100ms")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });

                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Engine Calculations:").strong());
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("  (AFR - 14.7) / 14.7 * 100")
                                    .monospace()
                                    .color(egui::Color32::LIGHT_GREEN),
                            );
                            ui.label(
                                egui::RichText::new("— AFR % deviation from stoich")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("  MAP / 101.325 * 100")
                                    .monospace()
                                    .color(egui::Color32::LIGHT_GREEN),
                            );
                            ui.label(
                                egui::RichText::new("— MAP as % of atmosphere")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });

                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Averaging / Smoothing:").strong());
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("  (RPM + RPM[-1] + RPM[-2]) / 3")
                                    .monospace()
                                    .color(egui::Color32::LIGHT_GREEN),
                            );
                            ui.label(
                                egui::RichText::new("— 3-sample moving average")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });

                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Unit Conversions:").strong());
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("  MAP * 0.145038")
                                    .monospace()
                                    .color(egui::Color32::LIGHT_GREEN),
                            );
                            ui.label(
                                egui::RichText::new("— kPa to PSI")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("  (ECT - 32) * 5 / 9")
                                    .monospace()
                                    .color(egui::Color32::LIGHT_GREEN),
                            );
                            ui.label(
                                egui::RichText::new("— Fahrenheit to Celsius")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                    });

                // Syntax help section
                egui::CollapsingHeader::new("Formula Syntax Reference")
                    .default_open(false)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Channel References:").strong());
                        ui.label("  RPM              - Current value of RPM channel");
                        ui.label("  \"Manifold Pressure\" - Channels with spaces (use quotes)");
                        ui.label("  RPM[-1]          - Previous sample (index offset)");
                        ui.label("  RPM[+2]          - 2 samples ahead");
                        ui.label("  RPM@-0.1s        - Value 100ms ago (time offset)");

                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Operators:").strong());
                        ui.label("  + - * /          - Basic math");
                        ui.label("  ^                - Power");
                        ui.label("  ( )              - Grouping");

                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Functions:").strong());
                        ui.label("  sin, cos, tan    - Trigonometry");
                        ui.label("  sqrt, abs        - Square root, absolute value");
                        ui.label("  ln, log, exp     - Logarithms, exponential");
                        ui.label("  min, max         - Minimum, maximum");
                        ui.label("  floor, ceil      - Rounding");

                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Statistics (for anomaly detection):").strong());
                        ui.label("  _mean_RPM        - Mean of entire RPM channel");
                        ui.label("  _stdev_RPM       - Standard deviation of RPM");
                        ui.label("  _min_RPM         - Minimum value of RPM");
                        ui.label("  _max_RPM         - Maximum value of RPM");
                        ui.label("  _range_RPM       - Range (max - min) of RPM");
                        ui.label("");
                        ui.label(
                            egui::RichText::new("Z-score example: (RPM - _mean_RPM) / _stdev_RPM")
                                .small()
                                .color(egui::Color32::LIGHT_GREEN),
                        );
                    });
            });

        if !open {
            self.show_computed_channels_manager = false;
        }
    }

    /// Apply a computed channel template to the current file
    pub fn apply_computed_channel_template(&mut self, template: &ComputedChannelTemplate) {
        let Some(tab_idx) = self.active_tab else {
            self.show_toast_warning("No active tab");
            return;
        };

        let file_idx = self.tabs[tab_idx].file_index;
        let file = &self.files[file_idx];

        // Get available channel names
        let available_channels: Vec<String> = file.log.channels.iter().map(|c| c.name()).collect();

        // Extract channel references and build bindings
        let refs = extract_channel_references(&template.formula);
        let bindings = match build_channel_bindings(&refs, &available_channels) {
            Ok(b) => b,
            Err(e) => {
                self.show_toast_error(&format!("Failed to apply: {}", e));
                return;
            }
        };

        // Check if formula uses statistical variables (for z-score anomaly detection)
        let needs_statistics = template.formula.contains("_mean_")
            || template.formula.contains("_stdev_")
            || template.formula.contains("_min_")
            || template.formula.contains("_max_")
            || template.formula.contains("_range_");

        // Evaluate the formula (with or without statistics)
        let cached_data = if needs_statistics {
            // Compute statistics for all channels
            let statistics = compute_all_channel_statistics(&available_channels, &file.log.data);

            match evaluate_all_records_with_stats(
                &template.formula,
                &bindings,
                &file.log.data,
                &file.log.times,
                Some(&statistics),
            ) {
                Ok(data) => Some(data),
                Err(e) => {
                    self.show_toast_error(&format!("Evaluation failed: {}", e));
                    return;
                }
            }
        } else {
            match evaluate_all_records(
                &template.formula,
                &bindings,
                &file.log.data,
                &file.log.times,
            ) {
                Ok(data) => Some(data),
                Err(e) => {
                    self.show_toast_error(&format!("Evaluation failed: {}", e));
                    return;
                }
            }
        };

        // Create the computed channel
        let mut channel = ComputedChannel::from_template(template.clone());
        channel.channel_bindings = bindings;
        channel.cached_data = cached_data;

        // Add to file's computed channels
        self.file_computed_channels
            .entry(file_idx)
            .or_default()
            .push(channel);

        self.show_toast_success(&format!("Applied '{}'", template.name));
    }

    /// Add a computed channel to the chart
    fn add_computed_channel_to_chart(&mut self, computed_idx: usize) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };

        let file_idx = self.tabs[tab_idx].file_index;
        let computed_channels = match self.file_computed_channels.get(&file_idx) {
            Some(c) => c,
            None => return,
        };

        let computed = match computed_channels.get(computed_idx) {
            Some(c) => c,
            None => return,
        };

        // Create a channel index for the computed channel
        // We use a virtual index: regular_channels_count + computed_index
        let regular_count = self.files[file_idx].log.channels.len();
        let virtual_channel_index = regular_count + computed_idx;

        // Check for duplicate
        if self.tabs[tab_idx]
            .selected_channels
            .iter()
            .any(|c| c.file_index == file_idx && c.channel_index == virtual_channel_index)
        {
            self.show_toast_warning("Channel already on chart");
            return;
        }

        // Check max channels
        if self.tabs[tab_idx].selected_channels.len() >= 10 {
            self.show_toast_warning("Maximum 10 channels reached");
            return;
        }

        // Find unused color
        let used_colors: std::collections::HashSet<usize> = self.tabs[tab_idx]
            .selected_channels
            .iter()
            .map(|c| c.color_index)
            .collect();
        let color_index = (0..CHART_COLORS.len())
            .find(|i| !used_colors.contains(i))
            .unwrap_or(0);

        // Create the selected channel with computed channel info
        let channel = Channel::Computed(ComputedChannelInfo {
            name: computed.template.name.clone(),
            formula: computed.template.formula.clone(),
            unit: computed.template.unit.clone(),
        });

        self.tabs[tab_idx].selected_channels.push(SelectedChannel {
            file_index: file_idx,
            channel_index: virtual_channel_index,
            channel,
            color_index,
        });

        self.show_toast_success(&format!("Added '{}' to chart", computed.name()));
    }
}
