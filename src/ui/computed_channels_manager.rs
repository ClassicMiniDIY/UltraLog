//! Computed Channels Manager UI.
//!
//! Provides a simplified window for users to manage their computed channel library
//! and apply computed channels to the active log file.

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
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::Window::new("Computed Channels")
            .open(&mut open)
            .resizable(true)
            .default_width(500.0)
            .default_height(450.0)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // Header with help button
                ui.horizontal(|ui| {
                    ui.heading("Computed Channels");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Help button
                        let help_btn = ui.add(
                            egui::Button::new(egui::RichText::new("?").size(font_14))
                                .min_size(egui::vec2(24.0, 24.0)),
                        );
                        if help_btn.clicked() {
                            self.show_computed_channels_help = !self.show_computed_channels_help;
                        }
                        help_btn.on_hover_text("Show formula syntax help");

                        if ui.button("+ New").clicked() {
                            self.formula_editor_state.open_new();
                        }
                    });
                });

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Create virtual channels from mathematical formulas.")
                        .size(font_12)
                        .color(egui::Color32::GRAY),
                );

                ui.add_space(8.0);

                // Quick Create section
                ui.label(egui::RichText::new("Quick Create:").size(font_12).strong());
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui
                        .button("Rate of Change")
                        .on_hover_text("Create: Channel - Channel[-1]")
                        .clicked()
                    {
                        self.formula_editor_state.open_with_pattern(
                            "Rate of Change",
                            "{channel} - {channel}[-1]",
                            "/sample",
                            "Rate of change per sample",
                        );
                    }
                    if ui
                        .button("Moving Avg")
                        .on_hover_text("Create: 3-sample moving average")
                        .clicked()
                    {
                        self.formula_editor_state.open_with_pattern(
                            "Moving Average",
                            "({channel} + {channel}[-1] + {channel}[-2]) / 3",
                            "",
                            "3-sample moving average for smoothing",
                        );
                    }
                    if ui
                        .button("% Deviation")
                        .on_hover_text("Create: Percentage deviation from a target")
                        .clicked()
                    {
                        self.formula_editor_state.open_with_pattern(
                            "Deviation",
                            "({channel} - 14.7) / 14.7 * 100",
                            "%",
                            "Percentage deviation from target value",
                        );
                    }
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // Search filter
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "Your Library ({})",
                            self.computed_library.templates.len()
                        ))
                        .size(font_14)
                        .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.computed_channels_search)
                                .hint_text("🔍 Search...")
                                .desired_width(120.0),
                        );
                    });
                });

                ui.add_space(4.0);

                // Library templates section
                if self.computed_library.templates.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            egui::RichText::new("No computed channels yet")
                                .color(egui::Color32::GRAY),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Use Quick Create or click '+ New' to get started")
                                .color(egui::Color32::GRAY)
                                .size(font_12),
                        );
                        ui.add_space(20.0);
                    });
                } else {
                    // Collect actions to perform after rendering
                    let mut template_to_edit: Option<String> = None;
                    let mut template_to_delete: Option<String> = None;
                    let mut template_to_apply: Option<ComputedChannelTemplate> = None;
                    let mut template_to_duplicate: Option<ComputedChannelTemplate> = None;

                    let search_lower = self.computed_channels_search.to_lowercase();

                    egui::ScrollArea::vertical()
                        .id_salt("library_templates_scroll")
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for template in &self.computed_library.templates {
                                // Filter by search
                                if !search_lower.is_empty() {
                                    let matches = template
                                        .name
                                        .to_lowercase()
                                        .contains(&search_lower)
                                        || template.formula.to_lowercase().contains(&search_lower)
                                        || template.category.to_lowercase().contains(&search_lower);
                                    if !matches {
                                        continue;
                                    }
                                }

                                self.render_template_card(
                                    ui,
                                    template,
                                    font_12,
                                    font_14,
                                    &mut template_to_edit,
                                    &mut template_to_delete,
                                    &mut template_to_apply,
                                    &mut template_to_duplicate,
                                );
                                ui.add_space(4.0);
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

                    if let Some(ref template) = template_to_duplicate {
                        let mut new_template = template.clone();
                        new_template.id = uuid::Uuid::new_v4().to_string();
                        new_template.name = format!("{} (copy)", template.name);
                        new_template.is_builtin = false;
                        self.computed_library.add_template(new_template);
                        let _ = self.computed_library.save();
                        self.show_toast_success("Template duplicated");
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Applied channels section (for current file)
                self.render_applied_channels_section(ui, font_12, font_14);
            });

        // Render help popup if open
        if self.show_computed_channels_help {
            self.render_computed_channels_help(ctx);
        }

        if !open {
            self.show_computed_channels_manager = false;
        }
    }

    /// Render a single template card with cleaner layout
    #[allow(clippy::too_many_arguments)]
    fn render_template_card(
        &self,
        ui: &mut egui::Ui,
        template: &ComputedChannelTemplate,
        font_12: f32,
        font_14: f32,
        template_to_edit: &mut Option<String>,
        template_to_delete: &mut Option<String>,
        template_to_apply: &mut Option<ComputedChannelTemplate>,
        template_to_duplicate: &mut Option<ComputedChannelTemplate>,
    ) {
        egui::Frame::NONE
            .fill(egui::Color32::from_rgb(45, 48, 45))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Template info
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("ƒ")
                                    .color(egui::Color32::from_rgb(150, 200, 255))
                                    .size(font_14),
                            );
                            ui.label(
                                egui::RichText::new(&template.name)
                                    .strong()
                                    .size(font_14)
                                    .color(egui::Color32::WHITE),
                            );
                            if !template.unit.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("({})", template.unit))
                                        .size(font_12)
                                        .color(egui::Color32::GRAY),
                                );
                            }
                            if template.is_builtin {
                                ui.label(
                                    egui::RichText::new("★")
                                        .size(font_12)
                                        .color(egui::Color32::GOLD),
                                )
                                .on_hover_text("Built-in template");
                            }
                        });
                        ui.label(
                            egui::RichText::new(&template.formula)
                                .monospace()
                                .size(font_12)
                                .color(egui::Color32::from_rgb(160, 180, 160)),
                        );
                    });

                    // Buttons on the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Overflow menu for Edit/Delete/Duplicate
                        ui.menu_button("⋮", |ui| {
                            if ui.button("Edit").clicked() {
                                *template_to_edit = Some(template.id.clone());
                                ui.close();
                            }
                            if ui.button("Duplicate").clicked() {
                                *template_to_duplicate = Some(template.clone());
                                ui.close();
                            }
                            ui.separator();
                            if ui
                                .button(
                                    egui::RichText::new("Delete")
                                        .color(egui::Color32::from_rgb(255, 120, 120)),
                                )
                                .clicked()
                            {
                                *template_to_delete = Some(template.id.clone());
                                ui.close();
                            }
                        });

                        // Apply button (primary action)
                        if self.active_tab.is_some() {
                            let apply_btn = egui::Button::new(
                                egui::RichText::new("Apply").color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(80, 110, 80));

                            if ui.add(apply_btn).clicked() {
                                *template_to_apply = Some(template.clone());
                            }
                        }
                    });
                });
            });
    }

    /// Render the applied channels section
    fn render_applied_channels_section(&mut self, ui: &mut egui::Ui, font_12: f32, font_14: f32) {
        if let Some(tab_idx) = self.active_tab {
            let file_idx = self.tabs[tab_idx].file_index;
            let applied_count = self
                .file_computed_channels
                .get(&file_idx)
                .map(|c| c.len())
                .unwrap_or(0);

            ui.label(
                egui::RichText::new(format!("Applied to Current File ({})", applied_count))
                    .size(font_14)
                    .strong(),
            );
            ui.add_space(4.0);

            if applied_count == 0 {
                ui.label(
                    egui::RichText::new("Apply templates from the library above")
                        .color(egui::Color32::GRAY)
                        .size(font_12),
                );
            } else {
                let mut channel_to_remove: Option<usize> = None;
                let mut channel_to_select: Option<usize> = None;

                if let Some(channels) = self.file_computed_channels.get(&file_idx) {
                    for (idx, channel) in channels.iter().enumerate() {
                        egui::Frame::NONE
                            .fill(egui::Color32::from_rgb(40, 45, 50))
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::symmetric(10, 6))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Status indicator
                                    if channel.is_valid() {
                                        ui.label(
                                            egui::RichText::new("●")
                                                .color(egui::Color32::GREEN)
                                                .size(font_12),
                                        );
                                    } else {
                                        ui.label(
                                            egui::RichText::new("●")
                                                .color(egui::Color32::RED)
                                                .size(font_12),
                                        );
                                    }

                                    ui.label(
                                        egui::RichText::new(channel.name())
                                            .size(font_14)
                                            .color(egui::Color32::LIGHT_GREEN),
                                    );

                                    if let Some(error) = &channel.error {
                                        ui.label(
                                            egui::RichText::new(error)
                                                .size(font_12)
                                                .color(egui::Color32::RED),
                                        );
                                    }

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if ui
                                                .small_button("×")
                                                .on_hover_text("Remove")
                                                .clicked()
                                            {
                                                channel_to_remove = Some(idx);
                                            }
                                            if channel.is_valid()
                                                && ui
                                                    .small_button("+ Chart")
                                                    .on_hover_text("Add to chart")
                                                    .clicked()
                                            {
                                                channel_to_select = Some(idx);
                                            }
                                        },
                                    );
                                });
                            });
                        ui.add_space(2.0);
                    }
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
                egui::RichText::new("Load a log file to apply computed channels")
                    .color(egui::Color32::GRAY)
                    .size(font_12),
            );
        }
    }

    /// Render the help popup with examples and syntax reference
    fn render_computed_channels_help(&mut self, ctx: &egui::Context) {
        let mut open = true;

        egui::Window::new("Formula Help")
            .open(&mut open)
            .resizable(false)
            .default_width(400.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(400.0)
                    .show(ui, |ui| {
                        // Channel References
                        ui.label(egui::RichText::new("Channel References").strong());
                        ui.add_space(4.0);
                        Self::help_row(ui, "RPM", "Current value of RPM channel");
                        Self::help_row(
                            ui,
                            "\"Manifold Pressure\"",
                            "Channels with spaces (use quotes)",
                        );
                        Self::help_row(ui, "RPM[-1]", "Previous sample (index offset)");
                        Self::help_row(ui, "RPM[+2]", "2 samples ahead");
                        Self::help_row(ui, "RPM@-0.1s", "Value 100ms ago (time offset)");

                        ui.add_space(12.0);
                        ui.label(egui::RichText::new("Operators").strong());
                        ui.add_space(4.0);
                        Self::help_row(ui, "+ - * /", "Basic math");
                        Self::help_row(ui, "^", "Power (e.g., RPM^2)");
                        Self::help_row(ui, "( )", "Grouping");

                        ui.add_space(12.0);
                        ui.label(egui::RichText::new("Functions").strong());
                        ui.add_space(4.0);
                        Self::help_row(ui, "sin, cos, tan", "Trigonometry");
                        Self::help_row(ui, "sqrt, abs", "Square root, absolute value");
                        Self::help_row(ui, "ln, log, exp", "Logarithms, exponential");
                        Self::help_row(ui, "min, max", "Minimum, maximum");
                        Self::help_row(ui, "floor, ceil", "Rounding");

                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new("Statistics (for anomaly detection)").strong(),
                        );
                        ui.add_space(4.0);
                        Self::help_row(ui, "_mean_RPM", "Mean of entire RPM channel");
                        Self::help_row(ui, "_stdev_RPM", "Standard deviation");
                        Self::help_row(ui, "_min_RPM / _max_RPM", "Min/max values");

                        ui.add_space(12.0);
                        ui.label(egui::RichText::new("Examples").strong());
                        ui.add_space(4.0);
                        Self::example_row(ui, "RPM - RPM[-1]", "RPM change per sample");
                        Self::example_row(ui, "(AFR - 14.7) / 14.7 * 100", "AFR % deviation");
                        Self::example_row(ui, "(RPM - _mean_RPM) / _stdev_RPM", "Z-score");
                    });
            });

        if !open {
            self.show_computed_channels_help = false;
        }
    }

    fn help_row(ui: &mut egui::Ui, code: &str, description: &str) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(code)
                    .monospace()
                    .color(egui::Color32::LIGHT_GREEN),
            );
            ui.label(egui::RichText::new("—").color(egui::Color32::GRAY));
            ui.label(egui::RichText::new(description).color(egui::Color32::GRAY));
        });
    }

    fn example_row(ui: &mut egui::Ui, formula: &str, description: &str) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(formula)
                    .monospace()
                    .color(egui::Color32::from_rgb(180, 200, 255)),
            );
        });
        ui.label(
            egui::RichText::new(format!("  {}", description))
                .small()
                .color(egui::Color32::GRAY),
        );
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
