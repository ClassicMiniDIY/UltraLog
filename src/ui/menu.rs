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

        egui::MenuBar::new().ui(ui, |ui| {
            // File menu
            ui.menu_button(t!("menu.file"), |ui| {
                ui.set_min_width(180.0);

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

                // Export opens the ExportDialog for the active tool
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

                let has_table_data = self
                    .active_tool
                    .generator_kind()
                    .is_some_and(|k| self.table_generator.table(k).is_some());

                let has_scatter_data =
                    !self.files.is_empty() && self.active_tool == ActiveTool::ScatterPlot;
                let can_export =
                    has_chart_data || has_histogram_data || has_scatter_data || has_table_data;

                ui.add_enabled_ui(can_export, |ui| {
                    if ui.button(t!("menu.export_ellipsis")).clicked() {
                        self.show_export_dialog = true;
                        ui.close();
                    }
                });
            });

            // View menu - tool modes and panels
            ui.menu_button(t!("menu.view"), |ui| {
                ui.set_min_width(200.0);

                // Chart grid toggle
                let old_show_grid = self.show_grid;
                ui.checkbox(
                    &mut self.show_grid,
                    egui::RichText::new(t!("menu.show_grid")).text_style(egui::TextStyle::Body),
                );
                if self.show_grid != old_show_grid {
                    self.user_settings.show_grid = self.show_grid;
                    if let Err(e) = self.user_settings.save() {
                        self.show_toast_error(&t!("toast.failed_to_save", error = e));
                    }
                }

                ui.separator();

                // Tool modes
                ui.label(
                    egui::RichText::new(t!("menu.tool_mode"))
                        .text_style(egui::TextStyle::Body)
                        .color(self.tokens.ink_subtle),
                );

                for (i, tool) in ActiveTool::ALL.into_iter().enumerate() {
                    let label = match tool {
                        ActiveTool::LogViewer => t!("menu.log_viewer"),
                        ActiveTool::ScatterPlot => t!("menu.scatter_plots"),
                        ActiveTool::Histogram => t!("menu.histogram"),
                        ActiveTool::LambdaDelay => t!("menu.lambda_delay"),
                        ActiveTool::AccelEnrich => t!("menu.accel_enrich"),
                    };
                    if ui
                        .radio(self.active_tool == tool, label.as_ref())
                        .on_hover_text(format!("\u{2318}{}", i + 1))
                        .clicked()
                    {
                        self.set_active_tool(tool);
                        ui.close();
                    }
                }

                ui.separator();

                // Panel navigation
                ui.label(
                    egui::RichText::new(t!("menu.side_panel"))
                        .text_style(egui::TextStyle::Body)
                        .color(self.tokens.ink_subtle),
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
                        ActivePanel::ToolProperties,
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

                if ui.button(t!("menu.documentation")).clicked() {
                    let _ = open::that("https://github.com/ClassicMiniDIY/UltraLog/wiki");
                    ui.close();
                }

                if ui.button(t!("menu.report_issue")).clicked() {
                    let _ = open::that("https://github.com/ClassicMiniDIY/UltraLog/issues");
                    ui.close();
                }

                ui.separator();

                if ui.button(t!("menu.support_development")).clicked() {
                    let _ = open::that("https://github.com/sponsors/ClassicMiniDIY");
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
                        .color(self.tokens.ink_subtle),
                    );
                });
            });
        });
    }
}
