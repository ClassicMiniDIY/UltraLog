//! Tools panel - analysis tools, computed channels library, and export options.
//!
//! Provides quick access to analysis and export functionality inline in the side panel.

use eframe::egui;
use rust_i18n::t;

use crate::analysis::tables::GeneratorKind;
use crate::app::UltraLogApp;
use crate::state::ActiveTool;

impl UltraLogApp {
    /// Render the tools panel content (called from side_panel.rs)
    pub fn render_tools_panel_content(&mut self, ui: &mut egui::Ui) {
        let _font_12 = self.scaled_font(12.0);
        let _font_14 = self.scaled_font(14.0);

        // Analysis Tools Section
        self.render_tools_analysis_section(ui);

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        // Table Generators Section
        self.render_tools_table_section(ui);

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        // Computed Channels Section
        self.render_tools_computed_section(ui);

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        // Export Section
        self.render_tools_export_section(ui);
    }

    /// Render the analysis tools section
    fn render_tools_analysis_section(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(
            egui::RichText::new("📈 Analysis Tools")
                .size(font_14)
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Run signal processing and statistical analysis on log data.")
                    .size(font_12)
                    .color(self.tokens.ink_subtle),
            );
            ui.add_space(8.0);

            let has_file = self.selected_file.is_some() && !self.files.is_empty();

            if has_file {
                // Show available analyzers count
                let analyzer_count = self.analyzer_registry.all().len();
                ui.label(
                    egui::RichText::new(format!("{} analyzers available", analyzer_count))
                        .size(font_12)
                        .color(self.tokens.ink_subtle),
                );
                ui.add_space(4.0);

                // Open full analysis panel button
                let primary_color = self.tokens.cyan;
                let btn = egui::Frame::NONE
                    .fill(primary_color)
                    .corner_radius(4)
                    .inner_margin(egui::vec2(12.0, 6.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Open Analysis Panel")
                                .color(self.tokens.on_cyan)
                                .size(font_14),
                        );
                    });

                if btn.response.interact(egui::Sense::click()).clicked() {
                    self.show_analysis_panel = true;
                }

                if btn.response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                // Show results count if any
                if let Some(file_idx) = self.selected_file
                    && let Some(results) = self.analysis_results.get(&file_idx)
                    && !results.is_empty()
                {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "✓ {} analysis results for current file",
                            results.len()
                        ))
                        .size(font_12)
                        .color(self.tokens.success),
                    );
                }
            } else {
                ui.label(
                    egui::RichText::new("Load a file to access analysis tools")
                        .size(font_12)
                        .color(self.tokens.ink_subtle)
                        .italics(),
                );
            }
        });
    }

    /// Render the table generators section (lambda delay / accel enrichment)
    fn render_tools_table_section(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(
            egui::RichText::new(format!("📊 {}", t!("table_gen.section_title")))
                .size(font_14)
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(t!("table_gen.section_help"))
                    .size(font_12)
                    .color(self.tokens.ink_subtle),
            );
            ui.add_space(8.0);

            let has_file = self.selected_file.is_some() && !self.files.is_empty();
            for kind in GeneratorKind::ALL {
                let name = kind.create().name();
                let status = self.table_generator.status(kind);
                let enabled = has_file || status.is_some();
                ui.horizontal(|ui| {
                    let tool = ActiveTool::for_generator(kind);
                    let btn = egui::Button::new(egui::RichText::new(name).size(font_12))
                        .selected(self.active_tool == tool);
                    if ui.add_enabled(enabled, btn).clicked() {
                        self.set_active_tool(tool);
                    }
                    if let Some(status) = status {
                        ui.label(
                            egui::RichText::new(status)
                                .size(font_12)
                                .color(self.tokens.success),
                        );
                    }
                });
            }
            if !has_file {
                ui.label(
                    egui::RichText::new(t!("table_gen.load_file_hint"))
                        .size(font_12)
                        .color(self.tokens.ink_subtle)
                        .italics(),
                );
            }
        });
    }

    /// Render the computed channels section
    fn render_tools_computed_section(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(
            egui::RichText::new("ƒ Computed Channels")
                .size(font_14)
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Create virtual channels from mathematical formulas.")
                    .size(font_12)
                    .color(self.tokens.ink_subtle),
            );
            ui.add_space(8.0);

            // Library count
            let template_count = self.computed_library.templates.len();
            ui.label(
                egui::RichText::new(format!("{} templates in library", template_count))
                    .size(font_12)
                    .color(self.tokens.ink_subtle),
            );
            ui.add_space(4.0);

            // Buttons row
            ui.horizontal(|ui| {
                // New Channel button
                let accent_color = self.tokens.cyan;
                let new_btn = egui::Frame::NONE
                    .fill(accent_color)
                    .corner_radius(4)
                    .inner_margin(egui::vec2(10.0, 5.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("+ New")
                                .color(self.tokens.on_cyan)
                                .size(font_12),
                        );
                    });

                if new_btn.response.interact(egui::Sense::click()).clicked() {
                    self.formula_editor_state.open_new();
                }

                if new_btn.response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                // Manage Library button
                let manage_btn = egui::Frame::NONE
                    .fill(self.tokens.bg_raised)
                    .corner_radius(4)
                    .inner_margin(egui::vec2(10.0, 5.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Manage Library")
                                .color(self.tokens.ink_muted)
                                .size(font_12),
                        );
                    });

                if manage_btn.response.interact(egui::Sense::click()).clicked() {
                    self.show_computed_channels_manager = true;
                }

                if manage_btn.response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });

            // Show applied channels for current file
            if let Some(file_idx) = self.selected_file
                && let Some(channels) = self.file_computed_channels.get(&file_idx)
                && !channels.is_empty()
            {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!(
                        "✓ {} computed channels on current file",
                        channels.len()
                    ))
                    .size(font_12)
                    .color(self.tokens.success),
                );
            }

            // Quick apply section
            if !self.computed_library.templates.is_empty() && self.selected_file.is_some() {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Quick Apply:")
                        .size(font_12)
                        .color(self.tokens.ink_subtle),
                );

                // Show first few templates as quick apply buttons
                let templates: Vec<_> = self
                    .computed_library
                    .templates
                    .iter()
                    .take(5)
                    .map(|t| (t.id.clone(), t.name.clone()))
                    .collect();

                for (id, name) in templates {
                    let response = ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("  • {}", name))
                                .size(font_12)
                                .color(self.tokens.cyan),
                        )
                        .sense(egui::Sense::click()),
                    );

                    if response.clicked() {
                        // Find and apply the template
                        if let Some(template) =
                            self.computed_library.templates.iter().find(|t| t.id == id)
                        {
                            let template_clone = template.clone();
                            self.apply_computed_channel_template(&template_clone);
                        }
                    }

                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            }
        });
    }

    /// Render the export section
    fn render_tools_export_section(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(egui::RichText::new("📤 Export").size(font_14).strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Export visualizations as images or documents.")
                        .size(font_12)
                        .color(self.tokens.ink_subtle),
                );
                ui.add_space(8.0);

                let has_file = self.selected_file.is_some() && !self.files.is_empty();
                let has_channels = !self.get_selected_channels().is_empty();
                let has_table = self
                    .active_tool
                    .generator_kind()
                    .is_some_and(|k| self.table_generator.table(k).is_some());

                // Determine what can be exported based on active tool
                let can_export_png = match self.active_tool {
                    ActiveTool::LogViewer => has_file && has_channels,
                    ActiveTool::ScatterPlot => has_file,
                    ActiveTool::Histogram => has_file,
                    ActiveTool::LambdaDelay | ActiveTool::AccelEnrich => has_table,
                };

                let can_export_pdf = can_export_png;

                ui.horizontal(|ui| {
                    // PNG Export
                    ui.add_enabled_ui(can_export_png, |ui| {
                        let btn = egui::Frame::NONE
                            .fill(if can_export_png {
                                self.tokens.bg_raised
                            } else {
                                self.tokens.bg_field
                            })
                            .stroke(egui::Stroke::new(1.0, self.tokens.line_control))
                            .corner_radius(4)
                            .inner_margin(egui::vec2(12.0, 6.0))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("PNG")
                                        .color(if can_export_png {
                                            self.tokens.ink
                                        } else {
                                            self.tokens.ink_disabled
                                        })
                                        .size(font_14),
                                );
                            });

                        if can_export_png && btn.response.interact(egui::Sense::click()).clicked() {
                            match self.active_tool {
                                ActiveTool::LogViewer => self.export_chart_png(),
                                ActiveTool::ScatterPlot => self.export_scatter_plot_png(),
                                ActiveTool::Histogram => self.export_histogram_png(),
                                ActiveTool::LambdaDelay | ActiveTool::AccelEnrich => {
                                    self.export_table_png()
                                }
                            }
                        }

                        if btn.response.hovered() && can_export_png {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    });

                    // PDF Export
                    ui.add_enabled_ui(can_export_pdf, |ui| {
                        let btn = egui::Frame::NONE
                            .fill(if can_export_pdf {
                                self.tokens.bg_raised
                            } else {
                                self.tokens.bg_field
                            })
                            .stroke(egui::Stroke::new(1.0, self.tokens.line_control))
                            .corner_radius(4)
                            .inner_margin(egui::vec2(12.0, 6.0))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("PDF")
                                        .color(if can_export_pdf {
                                            self.tokens.ink
                                        } else {
                                            self.tokens.ink_disabled
                                        })
                                        .size(font_14),
                                );
                            });

                        if can_export_pdf && btn.response.interact(egui::Sense::click()).clicked() {
                            match self.active_tool {
                                ActiveTool::LogViewer => self.export_chart_pdf(),
                                ActiveTool::ScatterPlot => self.export_scatter_plot_pdf(),
                                ActiveTool::Histogram => self.export_histogram_pdf(),
                                ActiveTool::LambdaDelay | ActiveTool::AccelEnrich => {
                                    self.export_table_pdf()
                                }
                            }
                        }

                        if btn.response.hovered() && can_export_pdf {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    });
                });

                // Show appropriate help text based on tool and state
                if !has_file {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Load a file to enable export")
                            .size(font_12)
                            .color(self.tokens.ink_subtle)
                            .italics(),
                    );
                } else if self.active_tool == ActiveTool::LogViewer && !has_channels {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Select channels to enable chart export")
                            .size(font_12)
                            .color(self.tokens.ink_subtle)
                            .italics(),
                    );
                }
            });
    }
}
