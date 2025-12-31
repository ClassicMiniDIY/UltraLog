//! Settings panel - consolidated settings for display, units, normalization, and updates.
//!
//! This panel provides a single location for all user preferences.

use eframe::egui;

use crate::analytics;
use crate::app::UltraLogApp;
use crate::state::FontScale;
use crate::units::{
    AccelerationUnit, DistanceUnit, FlowUnit, FuelEconomyUnit, PressureUnit, SpeedUnit,
    TemperatureUnit, VolumeUnit,
};
use crate::updater::UpdateState;

impl UltraLogApp {
    /// Render the settings panel content (called from side_panel.rs)
    pub fn render_settings_panel_content(&mut self, ui: &mut egui::Ui) {
        // Display settings section
        self.render_display_settings(ui);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Field normalization settings
        self.render_normalization_settings(ui);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Unit preferences
        self.render_unit_settings(ui);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Update settings
        self.render_update_settings(ui);
    }

    /// Render display settings section
    fn render_display_settings(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(egui::RichText::new("🖥 Display").size(font_14).strong())
            .default_open(true)
            .show(ui, |ui| {
                // Colorblind mode
                let old_color_blind_mode = self.color_blind_mode;
                ui.checkbox(
                    &mut self.color_blind_mode,
                    egui::RichText::new("Color Blind Mode").size(font_14),
                );
                if self.color_blind_mode != old_color_blind_mode {
                    analytics::track_colorblind_mode_toggled(self.color_blind_mode);
                }
                ui.label(
                    egui::RichText::new("Use accessible color palette (Wong's palette)")
                        .size(font_12)
                        .color(egui::Color32::GRAY),
                );

                ui.add_space(8.0);

                // Font size
                ui.label(egui::RichText::new("Font Size:").size(font_14));
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.font_scale, FontScale::Small, "S");
                    ui.selectable_value(&mut self.font_scale, FontScale::Medium, "M");
                    ui.selectable_value(&mut self.font_scale, FontScale::Large, "L");
                    ui.selectable_value(&mut self.font_scale, FontScale::ExtraLarge, "XL");
                });

                ui.add_space(8.0);

                // Cursor tracking
                ui.checkbox(
                    &mut self.cursor_tracking,
                    egui::RichText::new("Cursor Tracking").size(font_14),
                );
                ui.label(
                    egui::RichText::new("Keep cursor centered while scrubbing")
                        .size(font_12)
                        .color(egui::Color32::GRAY),
                );

                if self.cursor_tracking {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Window:").size(font_12));
                        ui.add(
                            egui::Slider::new(&mut self.view_window_seconds, 5.0..=120.0)
                                .suffix("s")
                                .logarithmic(true)
                                .text(""),
                        );
                    });
                }
            });
    }

    /// Render field normalization settings
    fn render_normalization_settings(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(egui::RichText::new("📝 Field Names").size(font_14).strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.checkbox(
                    &mut self.field_normalization,
                    egui::RichText::new("Field Normalization").size(font_14),
                );
                ui.label(
                    egui::RichText::new("Standardize channel names across ECU types")
                        .size(font_12)
                        .color(egui::Color32::GRAY),
                );

                ui.add_space(8.0);

                // Custom mappings count
                let custom_count = self.custom_normalizations.len();
                if custom_count > 0 {
                    ui.label(
                        egui::RichText::new(format!("{} custom mappings", custom_count))
                            .size(font_12)
                            .color(egui::Color32::GRAY),
                    );
                }

                // Edit mappings button
                let btn = egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(60, 60, 60))
                    .corner_radius(4)
                    .inner_margin(egui::vec2(12.0, 6.0))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Edit Custom Mappings")
                                .color(egui::Color32::LIGHT_GRAY)
                                .size(font_14),
                        );
                    });

                if btn.response.interact(egui::Sense::click()).clicked() {
                    self.show_normalization_editor = true;
                }

                if btn.response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });
    }

    /// Render unit preferences
    fn render_unit_settings(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(egui::RichText::new("📐 Units").size(font_14).strong())
            .default_open(true)
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Select preferred units for display")
                        .size(font_12)
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(8.0);

                // Create a grid for unit selections
                egui::Grid::new("unit_settings_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        // Temperature
                        ui.label(egui::RichText::new("Temperature:").size(font_12));
                        egui::ComboBox::from_id_salt("temp_unit")
                            .selected_text(self.unit_preferences.temperature.symbol())
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.unit_preferences.temperature,
                                    TemperatureUnit::Celsius,
                                    "°C",
                                );
                                ui.selectable_value(
                                    &mut self.unit_preferences.temperature,
                                    TemperatureUnit::Fahrenheit,
                                    "°F",
                                );
                                ui.selectable_value(
                                    &mut self.unit_preferences.temperature,
                                    TemperatureUnit::Kelvin,
                                    "K",
                                );
                            });
                        ui.end_row();

                        // Pressure
                        ui.label(egui::RichText::new("Pressure:").size(font_12));
                        egui::ComboBox::from_id_salt("pressure_unit")
                            .selected_text(self.unit_preferences.pressure.symbol())
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.unit_preferences.pressure,
                                    PressureUnit::KPa,
                                    "kPa",
                                );
                                ui.selectable_value(
                                    &mut self.unit_preferences.pressure,
                                    PressureUnit::PSI,
                                    "psi",
                                );
                                ui.selectable_value(
                                    &mut self.unit_preferences.pressure,
                                    PressureUnit::Bar,
                                    "bar",
                                );
                            });
                        ui.end_row();

                        // Speed
                        ui.label(egui::RichText::new("Speed:").size(font_12));
                        egui::ComboBox::from_id_salt("speed_unit")
                            .selected_text(self.unit_preferences.speed.symbol())
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.unit_preferences.speed,
                                    SpeedUnit::KmH,
                                    "km/h",
                                );
                                ui.selectable_value(
                                    &mut self.unit_preferences.speed,
                                    SpeedUnit::Mph,
                                    "mph",
                                );
                            });
                        ui.end_row();

                        // Distance
                        ui.label(egui::RichText::new("Distance:").size(font_12));
                        egui::ComboBox::from_id_salt("distance_unit")
                            .selected_text(self.unit_preferences.distance.symbol())
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.unit_preferences.distance,
                                    DistanceUnit::Kilometers,
                                    "km",
                                );
                                ui.selectable_value(
                                    &mut self.unit_preferences.distance,
                                    DistanceUnit::Miles,
                                    "mi",
                                );
                            });
                        ui.end_row();

                        // Fuel Economy
                        ui.label(egui::RichText::new("Fuel Economy:").size(font_12));
                        egui::ComboBox::from_id_salt("fuel_unit")
                            .selected_text(self.unit_preferences.fuel_economy.symbol())
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.unit_preferences.fuel_economy,
                                    FuelEconomyUnit::LPer100Km,
                                    "L/100km",
                                );
                                ui.selectable_value(
                                    &mut self.unit_preferences.fuel_economy,
                                    FuelEconomyUnit::Mpg,
                                    "mpg",
                                );
                                ui.selectable_value(
                                    &mut self.unit_preferences.fuel_economy,
                                    FuelEconomyUnit::KmPerL,
                                    "km/L",
                                );
                            });
                        ui.end_row();

                        // Volume
                        ui.label(egui::RichText::new("Volume:").size(font_12));
                        egui::ComboBox::from_id_salt("volume_unit")
                            .selected_text(self.unit_preferences.volume.symbol())
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.unit_preferences.volume,
                                    VolumeUnit::Liters,
                                    "L",
                                );
                                ui.selectable_value(
                                    &mut self.unit_preferences.volume,
                                    VolumeUnit::Gallons,
                                    "gal",
                                );
                            });
                        ui.end_row();

                        // Flow Rate
                        ui.label(egui::RichText::new("Flow Rate:").size(font_12));
                        egui::ComboBox::from_id_salt("flow_unit")
                            .selected_text(self.unit_preferences.flow.symbol())
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.unit_preferences.flow,
                                    FlowUnit::CcPerMin,
                                    "cc/min",
                                );
                                ui.selectable_value(
                                    &mut self.unit_preferences.flow,
                                    FlowUnit::LbPerHr,
                                    "lb/hr",
                                );
                            });
                        ui.end_row();

                        // Acceleration
                        ui.label(egui::RichText::new("Acceleration:").size(font_12));
                        egui::ComboBox::from_id_salt("accel_unit")
                            .selected_text(self.unit_preferences.acceleration.symbol())
                            .width(80.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.unit_preferences.acceleration,
                                    AccelerationUnit::MPerS2,
                                    "m/s²",
                                );
                                ui.selectable_value(
                                    &mut self.unit_preferences.acceleration,
                                    AccelerationUnit::G,
                                    "g",
                                );
                            });
                        ui.end_row();
                    });
            });
    }

    /// Render update settings
    fn render_update_settings(&mut self, ui: &mut egui::Ui) {
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        egui::CollapsingHeader::new(egui::RichText::new("🔄 Updates").size(font_14).strong())
            .default_open(true)
            .show(ui, |ui| {
                // Auto-check preference
                ui.checkbox(
                    &mut self.auto_check_updates,
                    egui::RichText::new("Check on startup").size(font_14),
                );
                ui.label(
                    egui::RichText::new("Automatically check for new versions")
                        .size(font_12)
                        .color(egui::Color32::GRAY),
                );

                ui.add_space(8.0);

                // Check now button
                let is_checking = matches!(self.update_state, UpdateState::Checking);
                ui.add_enabled_ui(!is_checking, |ui| {
                    let btn = egui::Frame::NONE
                        .fill(egui::Color32::from_rgb(60, 60, 60))
                        .corner_radius(4)
                        .inner_margin(egui::vec2(12.0, 6.0))
                        .show(ui, |ui| {
                            let text = if is_checking {
                                "Checking..."
                            } else {
                                "Check for Updates"
                            };
                            ui.label(
                                egui::RichText::new(text)
                                    .color(egui::Color32::LIGHT_GRAY)
                                    .size(font_14),
                            );
                        });

                    if !is_checking && btn.response.interact(egui::Sense::click()).clicked() {
                        self.start_update_check();
                    }

                    if btn.response.hovered() && !is_checking {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                });

                ui.add_space(8.0);

                // Version info
                let version = env!("CARGO_PKG_VERSION");
                ui.label(
                    egui::RichText::new(format!("Current version: {}", version))
                        .size(font_12)
                        .color(egui::Color32::GRAY),
                );

                // Show update status
                match &self.update_state {
                    UpdateState::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                egui::RichText::new("Checking...")
                                    .size(font_12)
                                    .color(egui::Color32::GRAY),
                            );
                        });
                    }
                    UpdateState::UpdateAvailable(info) => {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "✓ Update available: v{}",
                                info.new_version
                            ))
                            .size(font_12)
                            .color(egui::Color32::from_rgb(150, 200, 150)),
                        );

                        let view_btn = ui.add(
                            egui::Label::new(
                                egui::RichText::new("View Details →")
                                    .size(font_12)
                                    .color(egui::Color32::from_rgb(150, 180, 220)),
                            )
                            .sense(egui::Sense::click()),
                        );

                        if view_btn.clicked() {
                            self.show_update_dialog = true;
                        }

                        if view_btn.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                    UpdateState::Error(msg) => {
                        ui.label(
                            egui::RichText::new(format!("⚠ {}", msg))
                                .size(font_12)
                                .color(egui::Color32::from_rgb(200, 150, 100)),
                        );
                    }
                    _ => {}
                }
            });
    }
}
