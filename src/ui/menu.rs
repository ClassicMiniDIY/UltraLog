//! Menu bar UI components (File, View, Help menus).
//!
//! Simplified menu structure - settings moved to Settings panel.

use eframe::egui;
use rust_i18n::t;

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
            ui.menu_button(t!("menu.file"), |ui| {
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
                    .add_enabled(!is_loading, egui::Button::new(t!("menu.open_log_file")))
                    .on_hover_text("\u{2318}O")
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
                    .add_enabled(has_tabs, egui::Button::new(t!("menu.close_tab")))
                    .on_hover_text("\u{2318}W")
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
                    ui.menu_button(t!("menu.export"), |ui| {
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Button, egui::FontId::proportional(font_14));

                        if self.active_tool == ActiveTool::Histogram && has_histogram_data {
                            if ui.button(t!("menu.export_histogram_pdf")).clicked() {
                                self.export_histogram_pdf();
                                ui.close();
                            }
                        } else if has_chart_data {
                            if ui.button(t!("menu.export_png")).clicked() {
                                self.export_chart_png();
                                ui.close();
                            }
                            if ui.button(t!("menu.export_pdf")).clicked() {
                                self.export_chart_pdf();
                                ui.close();
                            }
                        }
                    });
                });
            });

            // View menu - tool modes and panels
            ui.menu_button(t!("menu.view"), |ui| {
                ui.set_min_width(200.0);

                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(font_14));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(font_14));

                // Tool modes
                ui.label(
                    egui::RichText::new(t!("menu.tool_mode"))
                        .size(font_14)
                        .color(egui::Color32::GRAY),
                );

                if ui
                    .radio_value(
                        &mut self.active_tool,
                        ActiveTool::LogViewer,
                        t!("menu.log_viewer"),
                    )
                    .on_hover_text("\u{2318}1")
                    .clicked()
                {
                    ui.close();
                }
                if ui
                    .radio_value(
                        &mut self.active_tool,
                        ActiveTool::ScatterPlot,
                        t!("menu.scatter_plots"),
                    )
                    .on_hover_text("\u{2318}2")
                    .clicked()
                {
                    ui.close();
                }
                if ui
                    .radio_value(
                        &mut self.active_tool,
                        ActiveTool::Histogram,
                        t!("menu.histogram"),
                    )
                    .on_hover_text("\u{2318}3")
                    .clicked()
                {
                    ui.close();
                }

                ui.separator();

                // Panel navigation
                ui.label(
                    egui::RichText::new(t!("menu.side_panel"))
                        .size(font_14)
                        .color(egui::Color32::GRAY),
                );

                if ui
                    .radio_value(&mut self.active_panel, ActivePanel::Files, t!("menu.files"))
                    .on_hover_text("\u{2318}\u{21E7}F")
                    .clicked()
                {
                    ui.close();
                }
                if ui
                    .radio_value(
                        &mut self.active_panel,
                        ActivePanel::Channels,
                        t!("menu.channels"),
                    )
                    .on_hover_text("\u{2318}\u{21E7}C")
                    .clicked()
                {
                    ui.close();
                }
                if ui
                    .radio_value(&mut self.active_panel, ActivePanel::Tools, t!("menu.tools"))
                    .on_hover_text("\u{2318}\u{21E7}T")
                    .clicked()
                {
                    ui.close();
                }
                if ui
                    .radio_value(
                        &mut self.active_panel,
                        ActivePanel::Settings,
                        t!("menu.settings"),
                    )
                    .on_hover_text("\u{2318},")
                    .clicked()
                {
                    ui.close();
                }
            });

            // Help menu
            ui.menu_button(t!("menu.help"), |ui| {
                ui.set_min_width(200.0);

                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(font_14));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(font_14));

                if ui.button(t!("menu.documentation")).clicked() {
                    let _ = open::that("https://github.com/SomethingNew71/UltraLog/wiki");
                    ui.close();
                }

                if ui.button(t!("menu.report_issue")).clicked() {
                    let _ = open::that("https://github.com/SomethingNew71/UltraLog/issues");
                    ui.close();
                }

                ui.separator();

                if ui.button(t!("menu.support_development")).clicked() {
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
                    t!("menu.checking_for_updates")
                } else {
                    t!("menu.check_for_updates")
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
                        egui::RichText::new(t!(
                            "menu.version",
                            version = env!("CARGO_PKG_VERSION")
                        ))
                        .color(egui::Color32::GRAY),
                    );
                });
            });
        });
    }
}
