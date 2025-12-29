//! Menu bar UI components (File, View, Units, Help menus).

use eframe::egui;

use crate::analytics;
use crate::app::UltraLogApp;
use crate::state::LoadingState;
use crate::units::{
    AccelerationUnit, DistanceUnit, FlowUnit, FuelEconomyUnit, PressureUnit, SpeedUnit,
    TemperatureUnit, VolumeUnit,
};

impl UltraLogApp {
    /// Render the application menu bar
    pub fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            // Increase font size for menu items
            ui.style_mut()
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(15.0));

            // File menu
            ui.menu_button("File", |ui| {
                ui.set_min_width(180.0);

                // Increase font size for dropdown items
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));

                let is_loading = matches!(self.loading_state, LoadingState::Loading(_));

                // Open file option
                if ui
                    .add_enabled(!is_loading, egui::Button::new("📂  Open Log File..."))
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

                // Export submenu
                let has_chart_data =
                    !self.files.is_empty() && !self.get_selected_channels().is_empty();
                ui.add_enabled_ui(has_chart_data, |ui| {
                    ui.menu_button("📤  Export", |ui| {
                        // Increase font size for submenu items
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                        if ui.button("Export as PNG...").clicked() {
                            self.export_chart_png();
                            ui.close();
                        }
                        if ui.button("Export as PDF...").clicked() {
                            self.export_chart_pdf();
                            ui.close();
                        }
                    });
                });
            });

            // View menu
            ui.menu_button("View", |ui| {
                ui.set_min_width(180.0);

                // Increase font size for dropdown items
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));

                // Cursor Tracking toggle
                if ui
                    .checkbox(&mut self.cursor_tracking, "🎯  Cursor Tracking")
                    .clicked()
                {
                    ui.close();
                }

                // Color Blind Mode toggle
                let old_color_blind_mode = self.color_blind_mode;
                if ui
                    .checkbox(&mut self.color_blind_mode, "👁  Color Blind Mode")
                    .clicked()
                {
                    if self.color_blind_mode != old_color_blind_mode {
                        analytics::track_colorblind_mode_toggled(self.color_blind_mode);
                    }
                    ui.close();
                }

                ui.separator();

                // Field Normalization toggle
                if ui
                    .checkbox(&mut self.field_normalization, "📝  Field Normalization")
                    .on_hover_text("Standardize channel names across different ECU types")
                    .clicked()
                {
                    ui.close();
                }

                // Edit mappings button
                if ui.button("      Edit Mappings...").clicked() {
                    self.show_normalization_editor = true;
                    ui.close();
                }

                ui.separator();

                // Auto-update preference
                if ui
                    .checkbox(
                        &mut self.auto_check_updates,
                        "🔄  Check for Updates on Startup",
                    )
                    .on_hover_text("Automatically check for new versions when the app starts")
                    .clicked()
                {
                    ui.close();
                }
            });

            // Units menu
            ui.menu_button("Units", |ui| {
                ui.set_min_width(180.0);

                // Increase font size for dropdown items
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));

                // Temperature submenu
                ui.menu_button("°C  Temperature", |ui| {
                    // Increase font size for submenu items
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.temperature,
                            TemperatureUnit::Celsius,
                            "Celsius (°C)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.temperature,
                            TemperatureUnit::Fahrenheit,
                            "Fahrenheit (°F)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.temperature,
                            TemperatureUnit::Kelvin,
                            "Kelvin (K)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                });

                // Pressure submenu
                ui.menu_button("💨  Pressure", |ui| {
                    // Increase font size for submenu items
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.pressure,
                            PressureUnit::KPa,
                            "Kilopascal (kPa)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.pressure,
                            PressureUnit::PSI,
                            "PSI",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.pressure,
                            PressureUnit::Bar,
                            "Bar",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                });

                // Speed submenu
                ui.menu_button("🚗  Speed", |ui| {
                    // Increase font size for submenu items
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.speed,
                            SpeedUnit::KmH,
                            "Kilometers/hour (km/h)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.speed,
                            SpeedUnit::Mph,
                            "Miles/hour (mph)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                });

                // Distance submenu
                ui.menu_button("📏  Distance", |ui| {
                    // Increase font size for submenu items
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.distance,
                            DistanceUnit::Kilometers,
                            "Kilometers (km)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.distance,
                            DistanceUnit::Miles,
                            "Miles (mi)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                });

                ui.separator();

                // Fuel Economy submenu
                ui.menu_button("⛽  Fuel Economy", |ui| {
                    // Increase font size for submenu items
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.fuel_economy,
                            FuelEconomyUnit::LPer100Km,
                            "Liters/100km (L/100km)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.fuel_economy,
                            FuelEconomyUnit::Mpg,
                            "Miles/gallon (mpg)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.fuel_economy,
                            FuelEconomyUnit::KmPerL,
                            "Kilometers/liter (km/L)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                });

                // Volume submenu
                ui.menu_button("📊  Volume", |ui| {
                    // Increase font size for submenu items
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.volume,
                            VolumeUnit::Liters,
                            "Liters (L)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.volume,
                            VolumeUnit::Gallons,
                            "Gallons (gal)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                });

                // Flow submenu
                ui.menu_button("💧  Flow Rate", |ui| {
                    // Increase font size for submenu items
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.flow,
                            FlowUnit::CcPerMin,
                            "cc/min",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .radio_value(&mut self.unit_preferences.flow, FlowUnit::LbPerHr, "lb/hr")
                        .clicked()
                    {
                        ui.close();
                    }
                });

                ui.separator();

                // Acceleration submenu
                ui.menu_button("📈  Acceleration", |ui| {
                    // Increase font size for submenu items
                    ui.style_mut()
                        .text_styles
                        .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.acceleration,
                            AccelerationUnit::MPerS2,
                            "m/s²",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.acceleration,
                            AccelerationUnit::G,
                            "g-force (g)",
                        )
                        .clicked()
                    {
                        ui.close();
                    }
                });
            });

            // Channels menu
            ui.menu_button("Channels", |ui| {
                ui.set_min_width(200.0);

                // Increase font size for dropdown items
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));

                if ui.button("ƒ(x)  Computed Channels...").clicked() {
                    self.show_computed_channels_manager = true;
                    ui.close();
                }
            });

            ui.menu_button("Help", |ui| {
                ui.set_min_width(200.0);

                // Increase font size for dropdown items
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                ui.style_mut()
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));

                if ui.button("📖  Documentation").clicked() {
                    let _ = open::that("https://github.com/SomethingNew71/UltraLog/wiki");
                    ui.close();
                }

                if ui.button("🐛  Report Issue").clicked() {
                    let _ = open::that("https://github.com/SomethingNew71/UltraLog/issues");
                    ui.close();
                }

                ui.separator();

                if ui.button("💝  Support Development").clicked() {
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
                    "🔄  Checking for Updates..."
                } else {
                    "🔄  Check for Updates"
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
