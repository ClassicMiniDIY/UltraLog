//! Channel selection and display UI components.

use eframe::egui;

use rust_i18n::t;

use crate::app::UltraLogApp;
use crate::normalize::normalize_channel_name_with_custom;
use crate::ui::components::{self, icon};
use crate::ui::fonts::{self, style};
use crate::ui::plot_style;
use crate::ui::theme::{
    SeriesStroke,
    tokens::{radius, space, stroke},
};

impl UltraLogApp {
    /// Render selected channel cards
    pub fn render_selected_channels(&mut self, ui: &mut egui::Ui) {
        let t = self.tokens;
        ui.label(
            egui::RichText::new("Selected Channels")
                .text_style(fonts::text_style(style::TITLE))
                .color(t.ink),
        );
        ui.separator();

        let use_normalization = self.field_normalization;

        // Get selected channels from the active tab
        let selected_channels = self.get_selected_channels().to_vec();

        // Pre-compute all display data to avoid borrow conflicts in closure
        struct ChannelCardData {
            color: egui::Color32,
            stroke: SeriesStroke,
            unit: String,
            display_name: String,
            is_computed: bool,
            min_str: Option<String>,
            max_str: Option<String>,
            min_record: Option<usize>,
            max_record: Option<usize>,
            min_time: Option<f64>,
            max_time: Option<f64>,
            file_index: usize,
            /// "A" / "B" while the tab compares two logs.
            tag: Option<&'static str>,
            /// Log B's file name, shown under the name on B cards.
            source: Option<String>,
            /// A log-A card with no partner while comparing ("not in B").
            not_in_b: bool,
            has_partner: bool,
        }

        let tab = self.active_tab.map(|idx| &self.tabs[idx]);

        let mut channel_cards: Vec<ChannelCardData> = Vec::with_capacity(selected_channels.len());

        for (idx, selected) in selected_channels.iter().enumerate() {
            let (color32, series_stroke) = self.channel_style(selected);
            let tag = self.log_tag(selected);
            let has_partner = tab.is_some_and(|t| t.partner(idx).is_some());
            let source = (tag == Some("B"))
                .then(|| self.files.get(selected.file_index).map(|f| f.name.clone()))
                .flatten();
            let not_in_b = tag == Some("A") && !has_partner && !selected.channel.is_computed();
            let mut card_unit = String::new();

            // Get display name
            let channel_name = selected.channel.name();
            let display_name = if use_normalization {
                normalize_channel_name_with_custom(&channel_name, Some(&self.custom_normalizations))
            } else {
                channel_name
            };

            // Get actual data min/max with record indices
            let (min_str, max_str, min_record, max_record, min_time, max_time) =
                if selected.file_index < self.files.len() {
                    let file = &self.files[selected.file_index];
                    let times = file.log.get_times_as_f64();

                    // Get data from either regular channel or computed channel
                    let data: Vec<f64> = if selected.channel.is_computed() {
                        // For computed channels, get data from file_computed_channels
                        let regular_count = file.log.channels.len();
                        if selected.channel_index >= regular_count {
                            let computed_idx = selected.channel_index - regular_count;
                            self.file_computed_channels
                                .get(&selected.file_index)
                                .and_then(|channels| channels.get(computed_idx))
                                .and_then(|c| c.cached_data.clone())
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        }
                    } else {
                        // Regular channel data
                        file.log.get_channel_data(selected.channel_index)
                    };

                    if !data.is_empty() {
                        // Find min and max with their indices (filter out NaN values)
                        let valid_data: Vec<(usize, f64)> = data
                            .iter()
                            .enumerate()
                            .filter(|(_, v)| v.is_finite())
                            .map(|(i, v)| (i, *v))
                            .collect();

                        if valid_data.is_empty() {
                            (None, None, None, None, None, None)
                        } else {
                            let (min_idx, min_val) = valid_data
                                .iter()
                                .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                                .map(|(i, v)| (*i, *v))
                                .unwrap();
                            let (max_idx, max_val) = valid_data
                                .iter()
                                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                                .map(|(i, v)| (*i, *v))
                                .unwrap();

                            let source_unit = selected.channel.unit();
                            let (conv_min, display_unit) =
                                self.unit_preferences.convert_value(min_val, source_unit);
                            let (conv_max, _) =
                                self.unit_preferences.convert_value(max_val, source_unit);
                            card_unit = display_unit.to_string();

                            (
                                Some(format!("{conv_min:.1}")),
                                Some(format!("{conv_max:.1}")),
                                Some(min_idx),
                                Some(max_idx),
                                times.get(min_idx).copied(),
                                times.get(max_idx).copied(),
                            )
                        }
                    } else {
                        (None, None, None, None, None, None)
                    }
                } else {
                    (None, None, None, None, None, None)
                };

            channel_cards.push(ChannelCardData {
                color: color32,
                stroke: series_stroke,
                unit: card_unit,
                display_name,
                is_computed: selected.channel.is_computed(),
                min_str,
                max_str,
                min_record,
                max_record,
                min_time,
                max_time,
                file_index: selected.file_index,
                tag,
                source,
                not_in_b,
                has_partner,
            });
        }

        let mut channel_to_remove: Option<usize> = None;
        let mut pair_to_remove: Option<usize> = None;
        // (file, record in that file, time in that file's own time)
        let mut jump_to: Option<(usize, usize, f64)> = None;

        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal_top(|ui| {
                for (i, card) in channel_cards.iter().enumerate() {
                    let frame = egui::Frame::NONE
                        .fill(t.bg_raised)
                        .stroke(egui::Stroke::new(stroke::STROKE_HAIRLINE, t.line))
                        .corner_radius(egui::CornerRadius::same(radius::RADIUS_MD))
                        .inner_margin(egui::Margin {
                            left: 12,
                            right: 6,
                            top: 10,
                            bottom: 8,
                        })
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    // Series swatch: the line glyph, dashed when the line is
                                    let (sw, _) = ui.allocate_exact_size(
                                        egui::vec2(14.0, 14.0),
                                        egui::Sense::hover(),
                                    );
                                    plot_style::swatch(
                                        ui.painter(),
                                        sw.left_center(),
                                        sw.width(),
                                        card.color,
                                        card.stroke,
                                    );
                                    if card.is_computed {
                                        ui.label(
                                            egui::RichText::new("\u{0192}")
                                                .family(egui::FontFamily::Name(
                                                    fonts::DISPLAY_BOLD_ITALIC.into(),
                                                ))
                                                .color(t.magenta),
                                        )
                                        .on_hover_text("Computed channel (formula-based)");
                                    }
                                    if let Some(tag) = card.tag {
                                        ui.label(
                                            egui::RichText::new(tag)
                                                .text_style(fonts::text_style(style::DATA))
                                                .color(t.ink_muted),
                                        )
                                        .on_hover_text(
                                            card.source.as_deref().unwrap_or(&card.display_name),
                                        );
                                    }
                                    ui.label(
                                        egui::RichText::new(&card.display_name)
                                            .text_style(egui::TextStyle::Heading)
                                            .color(t.ink),
                                    );
                                    ui.add_space(space::SPACE_4);
                                    if card.has_partner
                                        && components::icon_button(
                                            ui,
                                            icon::TRASH,
                                            t!("compare.remove_pair"),
                                        )
                                        .clicked()
                                    {
                                        pair_to_remove = Some(i);
                                    }
                                    if components::icon_button(ui, icon::X, "Remove channel")
                                        .clicked()
                                    {
                                        channel_to_remove = Some(i);
                                    }
                                });
                                if let Some(source) = &card.source {
                                    ui.label(
                                        egui::RichText::new(source)
                                            .text_style(fonts::text_style(style::DATA_SM))
                                            .color(t.ink_subtle),
                                    );
                                } else if card.not_in_b {
                                    ui.label(
                                        egui::RichText::new(t!("compare.not_in_b"))
                                            .text_style(fonts::text_style(style::DATA_SM))
                                            .color(t.ink_subtle),
                                    );
                                }

                                for (label, value, record, time, hint) in [
                                    (
                                        "Min",
                                        &card.min_str,
                                        card.min_record,
                                        card.min_time,
                                        "Jump to minimum",
                                    ),
                                    (
                                        "Max",
                                        &card.max_str,
                                        card.max_record,
                                        card.max_time,
                                        "Jump to maximum",
                                    ),
                                ] {
                                    let Some(value) = value else { continue };
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(label)
                                                .text_style(fonts::text_style(style::LABEL))
                                                .color(t.ink_subtle),
                                        );
                                        ui.label(
                                            egui::RichText::new(value)
                                                .text_style(egui::TextStyle::Monospace)
                                                .color(t.ink),
                                        );
                                        if !card.unit.is_empty() {
                                            ui.label(
                                                egui::RichText::new(&card.unit)
                                                    .text_style(fonts::text_style(style::DATA_SM))
                                                    .color(t.ink_subtle),
                                            );
                                        }
                                        if let (Some(record), Some(time)) = (record, time)
                                            && components::icon_button(ui, icon::CARET_RIGHT, hint)
                                                .clicked()
                                        {
                                            jump_to = Some((card.file_index, record, time));
                                        }
                                    });
                                }
                            });
                        });

                    // 2px series strip along the top edge
                    let r = frame.response.rect;
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(
                            r.left_top(),
                            egui::vec2(r.width(), stroke::STROKE_INDICATOR),
                        ),
                        egui::CornerRadius {
                            nw: radius::RADIUS_MD,
                            ne: radius::RADIUS_MD,
                            sw: 0,
                            se: 0,
                        },
                        card.color,
                    );

                    ui.add_space(space::SPACE_1);
                }
            });
        });

        // Handle jump to min/max
        if let Some((file_index, record, time)) = jump_to {
            let (time, record) = self.cursor_for_file_sample(file_index, record, time);
            self.set_cursor_time(Some(time));
            self.set_cursor_record(Some(record));
            // Request the chart to center on this time
            self.set_jump_to_time(Some(time));
            // Stop playback when jumping
            self.is_playing = false;
            self.last_frame_time = None;
        }

        if let Some(index) = pair_to_remove {
            self.remove_pair(index);
        } else if let Some(index) = channel_to_remove {
            self.remove_channel(index);
        }

        if selected_channels.is_empty() {
            ui.label(
                egui::RichText::new("Click channels to add them to the chart")
                    .italics()
                    .color(t.ink_subtle),
            );
        }
    }
}
