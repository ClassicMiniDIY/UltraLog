//! Formula Editor UI.
//!
//! Provides a modal window for creating and editing computed channel formulas.
//! Features an expanded channel browser, quick pattern buttons, and rich preview with statistics.

use eframe::egui;
use rust_i18n::t;

use crate::app::UltraLogApp;
use crate::computed::ComputedChannelTemplate;
use crate::expression::{
    build_channel_bindings, extract_channel_references, generate_preview, validate_formula,
};
use crate::ui::components::{self, Kind, Status};
use crate::ui::fonts::{self, style};
use crate::ui::formula_syntax;
use crate::ui::theme::tokens::space;

impl UltraLogApp {
    /// Render the formula editor dialog
    pub fn render_formula_editor(&mut self, ctx: &egui::Context) {
        if !self.formula_editor_state.is_open {
            return;
        }

        let mut open = true;
        let mut should_save = false;

        let title = if self.formula_editor_state.is_editing() {
            t!("formula.edit_computed_channel")
        } else {
            t!("formula.new_computed_channel")
        };

        let window_response = components::window(title)
            .open(&mut open)
            .resizable(true)
            .default_width(550.0)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let t = self.tokens;
                let label = |text: std::borrow::Cow<'_, str>| {
                    egui::RichText::new(text)
                        .text_style(fonts::text_style(style::LABEL))
                        .color(t.ink_muted)
                };

                // Name field
                ui.horizontal(|ui| {
                    ui.label(label(t!("formula.name")));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.formula_editor_state.name)
                            .hint_text(t!("formula.name_hint"))
                            .desired_width(300.0),
                    );
                });

                ui.add_space(space::SPACE_2);

                // Formula field, syntax coloured
                ui.label(label(t!("formula.formula")));
                let data_font = fonts::text_style(style::DATA).resolve(ui.style());
                let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap_width: f32| {
                    let job =
                        formula_syntax::layout(text.as_str(), data_font.clone(), &t, wrap_width);
                    ui.fonts_mut(|f| f.layout_job(job))
                };
                let formula_response = ui.add(
                    egui::TextEdit::multiline(&mut self.formula_editor_state.formula)
                        .hint_text(t!("formula.formula_hint"))
                        .desired_width(ui.available_width())
                        .desired_rows(2)
                        .font(fonts::text_style(style::DATA))
                        .layouter(&mut layouter),
                );

                // Validate on formula change
                if formula_response.changed() {
                    self.validate_current_formula();
                }

                // Quick pattern buttons
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(t!("formula.insert"))
                            .small()
                            .color(t.ink_subtle),
                    );
                    ui.add_space(4.0);

                    // Math operators
                    for (label, insert) in [
                        ("+", " + "),
                        ("-", " - "),
                        ("*", " * "),
                        ("/", " / "),
                        ("()", "("),
                        ("abs", "abs("),
                        ("sqrt", "sqrt("),
                    ] {
                        if ui
                            .small_button(
                                egui::RichText::new(label)
                                    .text_style(fonts::text_style(style::DATA_SM))
                                    .color(t.ink_muted),
                            )
                            .on_hover_text(t!("formula.insert_tooltip", op = insert.trim()))
                            .clicked()
                        {
                            self.formula_editor_state.formula.push_str(insert);
                            self.validate_current_formula();
                        }
                    }

                    ui.separator();

                    // Time-shift operators
                    if ui
                        .small_button(
                            egui::RichText::new("[-1]")
                                .text_style(fonts::text_style(style::DATA_SM))
                                .color(t.orange),
                        )
                        .on_hover_text(t!("formula.prev_sample_tooltip"))
                        .clicked()
                    {
                        self.formula_editor_state.formula.push_str("[-1]");
                        self.validate_current_formula();
                    }
                    if ui
                        .small_button(
                            egui::RichText::new("@-0.1s")
                                .text_style(fonts::text_style(style::DATA_SM))
                                .color(t.orange),
                        )
                        .on_hover_text(t!("formula.time_ago_tooltip"))
                        .clicked()
                    {
                        self.formula_editor_state.formula.push_str("@-0.1s");
                        self.validate_current_formula();
                    }
                });

                // Show validation status
                ui.add_space(4.0);
                if let Some(error) = &self.formula_editor_state.validation_error {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                Status::Danger.icon(),
                                t!("formula.error")
                            ))
                            .color(t.danger),
                        );
                        ui.label(egui::RichText::new(error).color(t.ink));
                    });
                } else if !self.formula_editor_state.formula.is_empty() {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {}",
                            Status::Success.icon(),
                            t!("formula.formula_valid")
                        ))
                        .color(t.success),
                    );
                }

                ui.add_space(8.0);

                // Unit and description in a row
                ui.horizontal(|ui| {
                    ui.label(label(t!("formula.unit")));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.formula_editor_state.unit)
                            .hint_text(t!("formula.unit_hint"))
                            .desired_width(100.0),
                    );

                    ui.add_space(16.0);

                    ui.label(label(t!("formula.description")));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.formula_editor_state.description)
                            .hint_text(t!("formula.description_hint"))
                            .desired_width(ui.available_width() - 20.0),
                    );
                });

                ui.add_space(8.0);

                // Channel browser - expanded by default for discoverability
                if self.active_tab.is_some() {
                    egui::CollapsingHeader::new(t!("formula.available_channels"))
                        .default_open(true) // Expanded by default
                        .show(ui, |ui| {
                            let channels = self.get_available_channel_names();
                            if channels.is_empty() {
                                ui.label(
                                    egui::RichText::new(t!("formula.no_channels_available"))
                                        .color(t.ink_subtle),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new(t!("formula.click_to_insert"))
                                        .small()
                                        .color(t.ink_subtle),
                                );
                                egui::ScrollArea::vertical()
                                    .id_salt("channel_browser")
                                    .max_height(120.0)
                                    .show(ui, |ui| {
                                        ui.horizontal_wrapped(|ui| {
                                            for name in &channels {
                                                if ui
                                                    .small_button(
                                                        egui::RichText::new(name)
                                                            .text_style(fonts::text_style(
                                                                style::DATA_SM,
                                                            ))
                                                            .color(t.cyan),
                                                    )
                                                    .clicked()
                                                {
                                                    // Insert channel name into formula
                                                    let insert = if name.contains(' ') {
                                                        format!("\"{}\"", name)
                                                    } else {
                                                        name.clone()
                                                    };
                                                    self.formula_editor_state
                                                        .formula
                                                        .push_str(&insert);
                                                    self.validate_current_formula();
                                                }
                                            }
                                        });
                                    });
                            }
                        });
                }

                // Preview section with statistics
                if let Some(preview_values) = &self.formula_editor_state.preview_values
                    && !preview_values.is_empty()
                {
                    ui.add_space(8.0);
                    ui.separator();

                    // Calculate stats
                    let valid_values: Vec<f64> = preview_values
                        .iter()
                        .copied()
                        .filter(|v| v.is_finite())
                        .collect();

                    if !valid_values.is_empty() {
                        let min = valid_values.iter().copied().fold(f64::INFINITY, f64::min);
                        let max = valid_values
                            .iter()
                            .copied()
                            .fold(f64::NEG_INFINITY, f64::max);
                        let sum: f64 = valid_values.iter().sum();
                        let avg = sum / valid_values.len() as f64;

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(t!("formula.preview")).strong());
                            ui.add_space(8.0);

                            // Stats in a compact row
                            ui.label(
                                egui::RichText::new(t!("formula.min"))
                                    .text_style(fonts::text_style(style::DATA_SM))
                                    .color(t.ink_subtle),
                            );
                            ui.label(
                                egui::RichText::new(format!("{:.2}", min))
                                    .text_style(fonts::text_style(style::DATA))
                                    .color(t.ink),
                            );
                            ui.add_space(8.0);

                            ui.label(
                                egui::RichText::new(t!("formula.avg"))
                                    .text_style(fonts::text_style(style::DATA_SM))
                                    .color(t.ink_subtle),
                            );
                            ui.label(
                                egui::RichText::new(format!("{:.2}", avg))
                                    .text_style(fonts::text_style(style::DATA))
                                    .color(t.ink),
                            );
                            ui.add_space(8.0);

                            ui.label(
                                egui::RichText::new(t!("formula.max"))
                                    .text_style(fonts::text_style(style::DATA_SM))
                                    .color(t.ink_subtle),
                            );
                            ui.label(
                                egui::RichText::new(format!("{:.2}", max))
                                    .text_style(fonts::text_style(style::DATA))
                                    .color(t.ink),
                            );
                        });

                        // Sample values
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(t!("formula.sample"))
                                    .small()
                                    .color(t.ink_subtle),
                            );
                            for (i, val) in preview_values.iter().take(5).enumerate() {
                                if i > 0 {
                                    ui.label(egui::RichText::new(",").color(t.ink_subtle));
                                }
                                ui.label(
                                    egui::RichText::new(format!("{:.2}", val))
                                        .text_style(fonts::text_style(style::DATA))
                                        .color(t.ink_muted),
                                );
                            }
                            if preview_values.len() > 5 {
                                ui.label(egui::RichText::new("…").color(t.ink_subtle));
                            }
                        });
                    }
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                // Footer: actions right-aligned, ghost Cancel, secondary, one primary.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_save = !self.formula_editor_state.name.is_empty()
                        && !self.formula_editor_state.formula.is_empty()
                        && self.formula_editor_state.validation_error.is_none();

                    ui.add_enabled_ui(can_save, |ui| {
                        if components::button(ui, Kind::Primary, None, t!("formula.save")).clicked()
                        {
                            should_save = true;
                        }
                    });
                    if components::button(ui, Kind::Secondary, None, t!("formula.validate"))
                        .clicked()
                    {
                        self.validate_current_formula();
                    }
                    if components::button(ui, Kind::Ghost, None, t!("formula.cancel")).clicked() {
                        self.formula_editor_state.close();
                    }
                });
            });

        // Handle save action (outside of window closure to avoid borrow issues)
        if should_save {
            self.save_formula_editor();
        }

        components::finish_window(
            ctx,
            window_response.as_ref().map(|r| &r.response),
            &mut open,
        );
        if !open {
            self.formula_editor_state.close();
        }
    }

    /// Validate the current formula in the editor
    fn validate_current_formula(&mut self) {
        let formula = self.formula_editor_state.formula.clone();

        if formula.is_empty() {
            self.formula_editor_state.validation_error = None;
            self.formula_editor_state.preview_values = None;
            return;
        }

        // Get available channels
        let available_channels = self.get_available_channel_names();

        // Validate the formula
        match validate_formula(&formula, &available_channels) {
            Ok(()) => {
                self.formula_editor_state.validation_error = None;

                // Generate preview if we have data - get more samples for better stats
                if let Some(tab_idx) = self.active_tab {
                    let file_idx = self.tabs[tab_idx].file_index;
                    if file_idx < self.files.len() {
                        let file = &self.files[file_idx];
                        let refs = extract_channel_references(&formula);
                        if let Ok(bindings) = build_channel_bindings(&refs, &available_channels) {
                            // Get 100 samples for meaningful statistics
                            if let Ok(preview) = generate_preview(
                                &formula,
                                &bindings,
                                &file.log.data,
                                &file.log.times,
                                100,
                            ) {
                                self.formula_editor_state.preview_values = Some(preview);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                self.formula_editor_state.validation_error = Some(e);
                self.formula_editor_state.preview_values = None;
            }
        }
    }

    /// Save the formula from the editor to the library
    fn save_formula_editor(&mut self) {
        let state = &self.formula_editor_state;

        if state.is_editing() {
            // Update existing template
            if let Some(id) = &state.editing_template_id
                && let Some(template) = self.computed_library.find_template_mut(id)
            {
                template.name = state.name.clone();
                template.formula = state.formula.clone();
                template.unit = state.unit.clone();
                template.description = state.description.clone();
                template.touch();
            }
        } else {
            // Create new template
            let template = ComputedChannelTemplate::new(
                state.name.clone(),
                state.formula.clone(),
                state.unit.clone(),
                state.description.clone(),
            );
            self.computed_library.add_template(template);
        }

        // Save library
        if let Err(e) = self.computed_library.save() {
            self.show_toast_error(&t!("toast.failed_to_save", error = e));
        } else {
            self.show_toast_success(&t!("toast.channel_saved"));
        }

        self.formula_editor_state.close();
    }
}
