//! Menu bar UI components (File, View, Help menus).
//!
//! Simplified menu structure - settings moved to Settings panel.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::state::{ActivePanel, ActiveTool, LoadingState};

impl UltraLogApp {
    /// Render the application menu bar
    pub fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        // Pre-compute scaled font sizes for use in closures
        let font_14 = self.scaled_font(14.0);
        let font_15 = self.scaled_font(15.0);

        egui::MenuBar::new().ui(ui, |ui| {
            // Increase font size for menu items
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(font_15));

            // File menu
            ui.menu_button("File", |ui| {
                ui.set_min_width(180.0);

                // Increase font size for dropdown items
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(font_14));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(font_14));

                let is_loading = matches!(self.loading_state, LoadingState::Loading(_));

                // Open file option
                if ui
                    .add_enabled(!is_loading, egui::Button::new("Open Log File..."))
                    .on_hover_text("⌘O")
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Log Files", crate::state::SUPPORTED_EXTENSIONS)
                        .pick_file()
                    {
                        self.start_loading_file(path);
                    }
                    ui.close();
                }

                ui.separator();

                // Close current tab
                let has_tabs = !self.tabs.is_empty();
                if ui
                    .add_enabled(has_tabs, egui::Button::new("Close Tab"))
                    .on_hover_text("⌘W")
                    .clicked()
                {
                    if let Some(tab_idx) = self.active_tab {
                        self.close_tab(tab_idx);
                    }
                    ui.close();
                }

                ui.separator();

                // Export submenu - context-aware based on active tool
                let has_chart_data =
                    !self.files.is_empty() && !self.get_selected_channels().is_empty();
                let has_histogram_data = !self.files.is_empty()
                    && self.active_tool == ActiveTool::Histogram
                    && self.active_tab.is_some()
                    && {
                        let tab_idx = self.active_tab.unwrap();
                        let config = &self.tabs[tab_idx].histogram_state.config;
                        config.x_channel.is_some() && config.y_channel.is_some()
                    };

                let can_export = has_chart_data || has_histogram_data;

                ui.add_enabled_ui(can_export, |ui| {
                    ui.menu_button("Export", |ui| {
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Button, egui::FontId::proportional(font_14));

                        if self.active_tool == ActiveTool::Histogram && has_histogram_data {
                            if ui.button("Export Histogram as PDF...").clicked() {
                                self.export_histogram_pdf();
                                ui.close();
                            }
                        } else if has_chart_data {
                            if ui.button("Export as PNG...").clicked() {
                                self.export_chart_png();
                                ui.close();
                            }
                            if ui.button("Export as PDF...").clicked() {
                                self.export_chart_pdf();
                                ui.close();
                            }
                        }
                    });
                });
            });

            // View menu - tool modes and panels
            ui.menu_button("View", |ui| {
                ui.set_min_width(200.0);

                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(font_14));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(font_14));

                // Tool modes
                ui.label(
                    egui::RichText::new("Tool Mode")
                        .size(font_14)
                        .color(egui::Color32::GRAY),
                );

                if ui
                    .radio_value(&mut self.active_tool, ActiveTool::LogViewer, "Log Viewer")
                    .on_hover_text("⌘1")
                    .clicked()
                {
                    ui.close();
                }
                if ui
                    .radio_value(
                        &mut self.active_tool,
                        ActiveTool::ScatterPlot,
                        "Scatter Plots",
                    )
                    .on_hover_text("⌘2")
                    .clicked()
                {
                    ui.close();
                }
                if ui
                    .radio_value(&mut self.active_tool, ActiveTool::Histogram, "Histogram")
                    .on_hover_text("⌘3")
                    .clicked()
                {
                    ui.close();
                }

                ui.separator();

                // Panel navigation
                ui.label(
                    egui::RichText::new("Side Panel")
                        .size(font_14)
                        .color(egui::Color32::GRAY),
                );

                if ui
                    .radio_value(&mut self.active_panel, ActivePanel::Files, "Files")
                    .on_hover_text("⌘⇧F")
                    .clicked()
                {
                    ui.close();
                }
                if ui
                    .radio_value(&mut self.active_panel, ActivePanel::Channels, "Channels")
                    .on_hover_text("⌘⇧C")
                    .clicked()
                {
                    ui.close();
                }
                if ui
                    .radio_value(&mut self.active_panel, ActivePanel::Tools, "Tools")
                    .on_hover_text("⌘⇧T")
                    .clicked()
                {
                    ui.close();
                }
                if ui
                    .radio_value(&mut self.active_panel, ActivePanel::Settings, "Settings")
                    .on_hover_text("⌘,")
                    .clicked()
                {
                    ui.close();
                }
            });

            // Help menu
            ui.menu_button("Help", |ui| {
                ui.set_min_width(200.0);

                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(font_14));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(font_14));

                if ui.button("Documentation").clicked() {
                    let _ = open::that("https://github.com/SomethingNew71/UltraLog/wiki");
                    ui.close();
                }

                if ui.button("Report Issue").clicked() {
                    let _ = open::that("https://github.com/SomethingNew71/UltraLog/issues");
                    ui.close();
                }

                ui.separator();

                if ui.button("Support Development").clicked() {
                    let _ = open::that("https://github.com/sponsors/SomethingNew71");
                    ui.close();
                }

                ui.separator();

                // Check for Updates
                let is_checking = matches!(
                    self.update_state,
                    crate::updater::UpdateState::Checking
                        | crate::updater::UpdateState::Downloading
                );
                let button_text = if is_checking {
                    "Checking for Updates..."
                } else {
                    "Check for Updates"
                };

                if ui
                    .add_enabled(!is_checking, egui::Button::new(button_text))
                    .clicked()
                {
                    self.start_update_check();
                    ui.close();
                }

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                            .color(egui::Color32::GRAY),
                    );
                });
            });
        });
    }
}
