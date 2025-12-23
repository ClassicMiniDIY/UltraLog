//! Menu bar UI components (File, Units, Help menus).

use eframe::egui;

use crate::app::UltraLogApp;
use crate::state::LoadingState;
use crate::units::{
    AccelerationUnit, DistanceUnit, FlowUnit, FuelEconomyUnit, PressureUnit, SpeedUnit,
    TemperatureUnit, VolumeUnit,
};

impl UltraLogApp {
    /// Render the application menu bar
    pub fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
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
                        .add_filter("Log Files", &["csv", "mlg"])
                        .pick_file()
                    {
                        self.start_loading_file(path);
                    }
                    ui.close_menu();
                }

                ui.separator();

                // Export submenu
                let has_chart_data = !self.files.is_empty() && !self.selected_channels.is_empty();
                ui.add_enabled_ui(has_chart_data, |ui| {
                    ui.menu_button("📤  Export", |ui| {
                        // Increase font size for submenu items
                        ui.style_mut()
                            .text_styles
                            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
                        if ui.button("Export as PNG...").clicked() {
                            self.export_chart_png();
                            ui.close_menu();
                        }
                        if ui.button("Export as PDF...").clicked() {
                            self.export_chart_pdf();
                            ui.close_menu();
                        }
                    });
                });
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
                ui.menu_button("♨️  Temperature", |ui| {
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
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.temperature,
                            TemperatureUnit::Fahrenheit,
                            "Fahrenheit (°F)",
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.temperature,
                            TemperatureUnit::Kelvin,
                            "Kelvin (K)",
                        )
                        .clicked()
                    {
                        ui.close_menu();
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
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.pressure,
                            PressureUnit::PSI,
                            "PSI",
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.pressure,
                            PressureUnit::Bar,
                            "Bar",
                        )
                        .clicked()
                    {
                        ui.close_menu();
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
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.speed,
                            SpeedUnit::Mph,
                            "Miles/hour (mph)",
                        )
                        .clicked()
                    {
                        ui.close_menu();
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
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.distance,
                            DistanceUnit::Miles,
                            "Miles (mi)",
                        )
                        .clicked()
                    {
                        ui.close_menu();
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
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.fuel_economy,
                            FuelEconomyUnit::Mpg,
                            "Miles/gallon (mpg)",
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.fuel_economy,
                            FuelEconomyUnit::KmPerL,
                            "Kilometers/liter (km/L)",
                        )
                        .clicked()
                    {
                        ui.close_menu();
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
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.volume,
                            VolumeUnit::Gallons,
                            "Gallons (gal)",
                        )
                        .clicked()
                    {
                        ui.close_menu();
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
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(&mut self.unit_preferences.flow, FlowUnit::LbPerHr, "lb/hr")
                        .clicked()
                    {
                        ui.close_menu();
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
                        ui.close_menu();
                    }
                    if ui
                        .radio_value(
                            &mut self.unit_preferences.acceleration,
                            AccelerationUnit::G,
                            "g-force (g)",
                        )
                        .clicked()
                    {
                        ui.close_menu();
                    }
                });
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
                    let _ = open::that("https://github.com/SomethingNew71/UltraLog#readme");
                    ui.close_menu();
                }

                if ui.button("🐛  Report Issue").clicked() {
                    let _ = open::that("https://github.com/SomethingNew71/UltraLog/issues");
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("📧  Email Support").clicked() {
                    let _ =
                        open::that("mailto:support@classicminidiy.com?subject=UltraLog%20Support");
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("💝  Support Development").clicked() {
                    let _ = open::that("https://github.com/sponsors/SomethingNew71");
                    ui.close_menu();
                }

                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                });
            });
        });
    }
}
