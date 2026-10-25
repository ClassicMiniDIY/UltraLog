//! Tools panel - analysis tools, computed channels library, and export options.
//!
//! Provides quick access to analysis and export functionality inline in the side panel.

use eframe::egui;
use rust_i18n::t;

use crate::analysis::tables::GeneratorKind;
use crate::app::UltraLogApp;
use crate::state::ActiveTool;
use crate::ui::components::{self, Kind, icon};

impl UltraLogApp {
    /// Render the tools panel content (called from side_panel.rs)
    pub fn render_tools_panel_content(&mut self, ui: &mut egui::Ui) {
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
        egui::CollapsingHeader::new(egui::RichText::new("📈 Analysis Tools").heading().strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(
                        "Run signal processing and statistical analysis on log data.",
                    )
                    .small()
                    .color(self.tokens.ink_subtle),
                );
                ui.add_space(8.0);

                let has_file = self.selected_file.is_some() && !self.files.is_empty();

                if has_file {
                    // Show available analyzers count
                    let analyzer_count = self.analyzer_registry.all().len();
                    ui.label(
                        egui::RichText::new(format!("{} analyzers available", analyzer_count))
                            .small()
                            .color(self.tokens.ink_subtle),
                    );
                    ui.add_space(4.0);

                    // Open full analysis panel button
                    if components::button(
                        ui,
                        Kind::Primary,
                        Some(icon::FLASK),
                        "Open analysis panel",
                    )
                    .clicked()
                    {
                        self.show_analysis_panel = true;
                    }

                    // Show results count if any
                    if let Some(file_idx) = self.selected_file
                        && let Some(results) = self.analysis_results.get(&file_idx)
                        && !results.is_empty()
                    {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {} analysis results for current file",
                                icon::CHECK,
                                results.len()
                            ))
                            .small()
                            .color(self.tokens.success),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new("Load a file to access analysis tools")
                            .small()
                            .color(self.tokens.ink_subtle)
                            .italics(),
                    );
                }
            });
    }

    /// Render the table generators section (lambda delay / accel enrichment)
    fn render_tools_table_section(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(
            egui::RichText::new(format!("{} {}", icon::TABLE, t!("table_gen.section_title")))
                .heading()
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(t!("table_gen.section_help"))
                    .small()
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
                    let btn = egui::Button::new(egui::RichText::new(name).small())
                        .selected(self.active_tool == tool);
                    if ui.add_enabled(enabled, btn).clicked() {
                        self.set_active_tool(tool);
                    }
                    if let Some(status) = status {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {status}",
                                crate::ui::components::Status::Success.icon()
                            ))
                            .small()
                            .color(self.tokens.success),
                        );
                    }
                });
            }
            if !has_file {
                ui.label(
                    egui::RichText::new(t!("table_gen.load_file_hint"))
                        .small()
                        .color(self.tokens.ink_subtle)
                        .italics(),
                );
            }
        });
    }

    /// Render the computed channels section
    fn render_tools_computed_section(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(
            egui::RichText::new("ƒ Computed Channels")
                .heading()
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Create virtual channels from mathematical formulas.")
                    .small()
                    .color(self.tokens.ink_subtle),
            );
            ui.add_space(8.0);

            // Library count
            let template_count = self.computed_library.templates.len();
            ui.label(
                egui::RichText::new(format!("{} templates in library", template_count))
                    .small()
                    .color(self.tokens.ink_subtle),
            );
            ui.add_space(4.0);

            // Buttons row
            ui.horizontal(|ui| {
                if components::button_sm(ui, Kind::Ghost, Some(icon::PLUS), "New").clicked() {
                    self.formula_editor_state.open_new();
                }
                if components::button_sm(ui, Kind::Secondary, None, "Manage library").clicked() {
                    self.show_computed_channels_manager = true;
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
                        "{} {} computed channels on current file",
                        icon::CHECK,
                        channels.len()
                    ))
                    .small()
                    .color(self.tokens.success),
                );
            }

            // Quick apply section
            if !self.computed_library.templates.is_empty() && self.selected_file.is_some() {
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Quick Apply:")
                        .small()
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
                                .small()
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
        egui::CollapsingHeader::new(
            egui::RichText::new(format!("{} Export", icon::EXPORT))
                .heading()
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new("Export visualizations as images or documents.")
                    .small()
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

            ui.add_enabled_ui(can_export_png, |ui| {
                let export = components::button(
                    ui,
                    Kind::Secondary,
                    Some(icon::DOWNLOAD_SIMPLE),
                    t!("export.open"),
                );
                if export.clicked() {
                    self.show_export_dialog = true;
                }
            });

            // Show appropriate help text based on tool and state
            if !has_file {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Load a file to enable export")
                        .small()
                        .color(self.tokens.ink_subtle)
                        .italics(),
                );
            } else if self.active_tool == ActiveTool::LogViewer && !has_channels {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Select channels to enable chart export")
                        .small()
                        .color(self.tokens.ink_subtle)
                        .italics(),
                );
            }
        });
    }
}
