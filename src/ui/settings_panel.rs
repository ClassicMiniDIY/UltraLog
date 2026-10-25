//! Settings panel - consolidated settings for display, units, normalization, and updates.
//!
//! This panel provides a single location for all user preferences.

use eframe::egui;
use rust_i18n::t;

use crate::analytics;
use crate::app::UltraLogApp;
use crate::i18n::Language;
use crate::state::FontScale;
use crate::ui::components::{self, icon};
use crate::ui::theme;
use crate::ui::theme::tokens::space;
use crate::units::{
    AccelerationUnit, AfrLambdaUnit, DistanceUnit, FlowUnit, FuelEconomyUnit, PressureUnit,
    SpeedUnit, TemperatureUnit, VolumeUnit,
};
use crate::updater::UpdateState;

impl UltraLogApp {
    /// Render the settings panel content (called from side_panel.rs)
    pub fn render_settings_panel_content(&mut self, ui: &mut egui::Ui) {
        // Language settings section
        self.render_language_settings(ui);

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

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

    /// Render language settings section
    fn render_language_settings(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(
            egui::RichText::new(format!("{} {}", icon::GLOBE, t!("settings.language")))
                .heading()
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(t!("settings.language_desc"))
                    .small()
                    .color(self.tokens.ink_subtle),
            );

            ui.add_space(8.0);

            egui::ComboBox::from_id_salt("language_selector")
                .selected_text(self.language.display_name())
                .width(140.0)
                .show_ui(ui, |ui| {
                    for lang in Language::all() {
                        if ui
                            .selectable_value(&mut self.language, *lang, lang.display_name())
                            .changed()
                        {
                            // Update locale immediately
                            rust_i18n::set_locale(self.language.locale_code());

                            // Save to persistent settings
                            self.user_settings.language = self.language;
                            if let Err(e) = self.user_settings.save() {
                                self.show_toast_error(&t!("toast.failed_to_save", error = e));
                            }

                            // Request repaint to refresh all UI text
                            ui.ctx().request_repaint();
                        }
                    }
                });
        });
    }

    /// Ultra intensity: one slider, 0-100, with named stops (Stealth, Night Drive, Neon,
    /// Ultra) that snap within a small detent. Locked while High contrast is on.
    /// Spec: `design/design-system/components/IntensityControl/README.md`.
    fn render_intensity_control(&mut self, ui: &mut egui::Ui) {
        let t = self.tokens;
        let locked = self.high_contrast;
        let mut value = self.ultra_intensity;
        // Keep clear of the panel's floating scroll bar so the readout and "Ultra" stay visible.
        let width = ui.available_width() - ui.spacing().scroll.bar_width - space::SPACE_1;

        ui.horizontal(|ui| {
            ui.set_max_width(width);
            ui.label(
                egui::RichText::new(t!("settings.ultra_intensity"))
                    .text_style(egui::TextStyle::Body),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let readout = format!("{} \u{00B7} {}", theme::intensity_name(value), value);
                ui.label(
                    egui::RichText::new(readout)
                        .text_style(egui::TextStyle::Body)
                        .family(egui::FontFamily::Name(
                            crate::ui::fonts::DISPLAY_BOLD_ITALIC.into(),
                        ))
                        .color(if locked { t.ink_disabled } else { t.cyan }),
                );
            });
        });

        let small = egui::TextStyle::Small.resolve(ui.style());
        let slider = ui
            .add_enabled_ui(!locked, |ui| {
                components::intensity_slider(ui, &mut value, width)
            })
            .inner;
        if slider.changed() {
            value = theme::snap_intensity(value);
        }

        // Stop labels under the track; a click jumps to the nearest stop.
        let (rect, stops) = ui.allocate_exact_size(
            egui::vec2(width, small.size + 4.0),
            if locked {
                egui::Sense::hover()
            } else {
                egui::Sense::click()
            },
        );
        if stops.clicked()
            && let Some(pos) = stops.interact_pointer_pos()
        {
            let x = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) * 100.0;
            if let Some((stop, _)) = theme::INTENSITY_STOPS.iter().min_by(|a, b| {
                (f32::from(a.0) - x)
                    .abs()
                    .total_cmp(&(f32::from(b.0) - x).abs())
            }) {
                value = *stop;
            }
        }
        let painter = ui.painter_at(rect);
        for (stop, name) in theme::INTENSITY_STOPS {
            let x = rect.left() + rect.width() * f32::from(stop) / 100.0;
            let anchor = match stop {
                0 => egui::Align2::LEFT_TOP,
                100 => egui::Align2::RIGHT_TOP,
                _ => egui::Align2::CENTER_TOP,
            };
            let color = if locked {
                t.ink_disabled
            } else if theme::intensity_name(value) == name {
                t.ink
            } else {
                t.ink_subtle
            };
            painter.text(
                egui::pos2(x, rect.top()),
                anchor,
                name,
                small.clone(),
                color,
            );
        }

        let caption = if locked {
            t!("settings.ultra_intensity_locked")
        } else {
            t!("settings.ultra_intensity_desc")
        };
        ui.label(egui::RichText::new(caption).small().color(t.ink_subtle));

        if value != self.ultra_intensity {
            self.ultra_intensity = value;
            // One event per adjustment: at the end of a drag, or at once for a click/keypress.
            if !slider.dragged() && !slider.drag_stopped() {
                analytics::track_ultra_intensity_changed(value);
            }
        }
        if slider.drag_stopped() {
            analytics::track_ultra_intensity_changed(self.ultra_intensity);
        }
    }

    /// Render display settings section
    fn render_display_settings(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(
            egui::RichText::new(format!("{} {}", icon::MONITOR, t!("settings.display")))
                .heading()
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            self.render_intensity_control(ui);

            ui.add_space(8.0);

            // Colorblind mode
            let old_color_blind_mode = self.color_blind_mode;
            components::checkbox(
                ui,
                &mut self.color_blind_mode,
                t!("settings.color_blind_mode"),
            );
            if self.color_blind_mode != old_color_blind_mode {
                analytics::track_colorblind_mode_toggled(self.color_blind_mode);
            }
            ui.label(
                egui::RichText::new(t!("settings.color_blind_desc"))
                    .small()
                    .color(self.tokens.ink_subtle),
            );

            ui.add_space(8.0);

            // High contrast
            let old_high_contrast = self.high_contrast;
            components::checkbox(ui, &mut self.high_contrast, t!("settings.high_contrast"));
            if self.high_contrast != old_high_contrast {
                analytics::track_high_contrast_toggled(self.high_contrast);
            }
            ui.label(
                egui::RichText::new(t!("settings.high_contrast_desc"))
                    .small()
                    .color(self.tokens.ink_subtle),
            );

            ui.add_space(8.0);

            // Font size
            ui.label(
                egui::RichText::new(t!("settings.font_size")).text_style(egui::TextStyle::Body),
            );
            components::segmented(
                ui,
                &mut self.font_scale,
                &[
                    (FontScale::Small, "S"),
                    (FontScale::Medium, "M"),
                    (FontScale::Large, "L"),
                    (FontScale::ExtraLarge, "XL"),
                ],
            );

            ui.add_space(8.0);

            // Cursor tracking
            components::checkbox(
                ui,
                &mut self.cursor_tracking,
                t!("settings.cursor_tracking"),
            );
            ui.label(
                egui::RichText::new(t!("settings.cursor_tracking_desc"))
                    .small()
                    .color(self.tokens.ink_subtle),
            );

            if self.cursor_tracking {
                ui.add_space(4.0);
                let mut window_resp = None;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(t!("settings.window")).small());
                    window_resp = Some(
                        ui.add(
                            egui::Slider::new(&mut self.view_window_seconds, 5.0..=120.0)
                                .suffix("s")
                                .logarithmic(true)
                                .text(""),
                        ),
                    );
                });
                if window_resp.is_some_and(|r| r.changed()) {
                    self.current_view_window = self.view_window_seconds;
                }
            }

            ui.add_space(8.0);

            // Scroll to zoom
            let old_scroll_to_zoom = self.scroll_to_zoom;
            components::checkbox(ui, &mut self.scroll_to_zoom, t!("settings.scroll_to_zoom"));
            ui.label(
                egui::RichText::new(t!("settings.scroll_to_zoom_desc"))
                    .small()
                    .color(self.tokens.ink_subtle),
            );
            if self.scroll_to_zoom != old_scroll_to_zoom {
                self.user_settings.scroll_to_zoom = self.scroll_to_zoom;
                if let Err(e) = self.user_settings.save() {
                    self.show_toast_error(&t!("toast.failed_to_save", error = e));
                }
            }

            ui.add_space(8.0);

            // Chart grid
            let old_show_grid = self.show_grid;
            components::checkbox(ui, &mut self.show_grid, t!("settings.show_grid"));
            ui.label(
                egui::RichText::new(t!("settings.show_grid_desc"))
                    .small()
                    .color(self.tokens.ink_subtle),
            );
            if self.show_grid != old_show_grid {
                self.user_settings.show_grid = self.show_grid;
                if let Err(e) = self.user_settings.save() {
                    self.show_toast_error(&t!("toast.failed_to_save", error = e));
                }
            }

            if self.show_grid {
                ui.add_space(4.0);
                let mut slider_response = None;
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(t!("settings.grid_opacity")).small());
                    slider_response =
                        Some(ui.add(egui::Slider::new(&mut self.grid_opacity, 0..=255)));
                });
                // Persist only when the user releases the slider, otherwise
                // every drag pixel rewrites settings.json.
                let committed = slider_response
                    .map(|r| r.drag_stopped() || r.lost_focus())
                    .unwrap_or(false);
                if committed && self.user_settings.grid_opacity != self.grid_opacity {
                    self.user_settings.grid_opacity = self.grid_opacity;
                    if let Err(e) = self.user_settings.save() {
                        self.show_toast_error(&t!("toast.failed_to_save", error = e));
                    }
                }
            }
        });
    }

    /// Render field normalization settings
    fn render_normalization_settings(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(
            egui::RichText::new(format!("{} {}", icon::TEXT_AA, t!("settings.field_names")))
                .heading()
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            components::checkbox(
                ui,
                &mut self.field_normalization,
                t!("settings.field_normalization"),
            );
            ui.label(
                egui::RichText::new(t!("settings.field_normalization_desc"))
                    .small()
                    .color(self.tokens.ink_subtle),
            );

            ui.add_space(8.0);

            // Custom mappings count
            let custom_count = self.custom_normalizations.len();
            if custom_count > 0 {
                ui.label(
                    egui::RichText::new(t!("settings.custom_mappings", count = custom_count))
                        .small()
                        .color(self.tokens.ink_subtle),
                );
            }

            // Edit mappings button
            let btn = egui::Frame::NONE
                .fill(self.tokens.bg_raised)
                .corner_radius(4)
                .inner_margin(egui::vec2(12.0, 6.0))
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(t!("settings.edit_custom_mappings"))
                            .color(self.tokens.ink_muted)
                            .text_style(egui::TextStyle::Body),
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
        egui::CollapsingHeader::new(
            egui::RichText::new(format!("{} {}", icon::RULER, t!("settings.units")))
                .heading()
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(t!("settings.units_desc"))
                    .small()
                    .color(self.tokens.ink_subtle),
            );
            ui.add_space(8.0);

            // Create a grid for unit selections
            egui::Grid::new("unit_settings_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    // Temperature
                    ui.label(egui::RichText::new(t!("settings.temperature")).small());
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
                    ui.label(egui::RichText::new(t!("settings.pressure")).small());
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
                    ui.label(egui::RichText::new(t!("settings.speed")).small());
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
                    ui.label(egui::RichText::new(t!("settings.distance")).small());
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
                    ui.label(egui::RichText::new(t!("settings.fuel_economy")).small());
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
                    ui.label(egui::RichText::new(t!("settings.volume")).small());
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
                    ui.label(egui::RichText::new(t!("settings.flow_rate")).small());
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
                    ui.label(egui::RichText::new(t!("settings.acceleration")).small());
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

                    // AFR/Lambda
                    ui.label(egui::RichText::new(t!("settings.afr_lambda")).small());
                    egui::ComboBox::from_id_salt("afr_lambda_unit")
                        .selected_text(self.unit_preferences.afr_lambda.symbol())
                        .width(80.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.unit_preferences.afr_lambda,
                                AfrLambdaUnit::AFR,
                                "AFR",
                            );
                            ui.selectable_value(
                                &mut self.unit_preferences.afr_lambda,
                                AfrLambdaUnit::Lambda,
                                "λ",
                            );
                        });
                    ui.end_row();
                });
        });
    }

    /// Render update settings
    fn render_update_settings(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(
            egui::RichText::new(format!(
                "{} {}",
                icon::ARROWS_CLOCKWISE,
                t!("settings.updates")
            ))
            .heading()
            .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            // Auto-check preference
            components::checkbox(
                ui,
                &mut self.auto_check_updates,
                t!("settings.check_on_startup"),
            );
            ui.label(
                egui::RichText::new(t!("settings.auto_check_desc"))
                    .small()
                    .color(self.tokens.ink_subtle),
            );

            ui.add_space(8.0);

            // Check now button
            let is_checking = matches!(self.update_state, UpdateState::Checking);
            ui.add_enabled_ui(!is_checking, |ui| {
                let btn = egui::Frame::NONE
                    .fill(self.tokens.bg_raised)
                    .corner_radius(4)
                    .inner_margin(egui::vec2(12.0, 6.0))
                    .show(ui, |ui| {
                        let text = if is_checking {
                            t!("settings.checking")
                        } else {
                            t!("settings.check_for_updates")
                        };
                        ui.label(
                            egui::RichText::new(text)
                                .color(self.tokens.ink_muted)
                                .text_style(egui::TextStyle::Body),
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
                egui::RichText::new(t!("settings.current_version", version = version))
                    .small()
                    .color(self.tokens.ink_subtle),
            );

            // Show update status
            match &self.update_state {
                UpdateState::Checking => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new(t!("settings.checking"))
                                .small()
                                .color(self.tokens.ink_subtle),
                        );
                    });
                }
                UpdateState::UpdateAvailable(info) => {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            crate::ui::components::Status::Success.icon(),
                            t!("settings.update_available", version = &info.new_version)
                        ))
                        .small()
                        .color(self.tokens.success),
                    );

                    let view_btn = ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!(
                                "{} \u{2192}",
                                t!("settings.view_details")
                            ))
                            .small()
                            .color(self.tokens.cyan),
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
                        egui::RichText::new(format!("{} {}", icon::WARNING, msg))
                            .small()
                            .color(self.tokens.warning),
                    );
                }
                _ => {}
            }
        });
    }
}
