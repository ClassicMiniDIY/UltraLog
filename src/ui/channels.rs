//! Channel selection and display UI components.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::normalize::{normalize_channel_name_with_custom, sort_channels_by_priority};
use crate::state::MAX_CHANNELS;

impl UltraLogApp {
    /// Render channel selection panel - fills available space
    pub fn render_channel_selection(&mut self, ui: &mut egui::Ui) {
        ui.heading("Channels");
        ui.separator();

        // Get active tab info
        let tab_info = self.active_tab.and_then(|tab_idx| {
            let tab = &self.tabs[tab_idx];
            if tab.file_index < self.files.len() {
                Some((
                    tab.file_index,
                    tab.channel_search.clone(),
                    tab.selected_channels.len(),
                ))
            } else {
                None
            }
        });

        if let Some((file_index, current_search, selected_count)) = tab_info {
            let channel_count = self.files[file_index].log.channels.len();

            // Search box - use a temporary string that we'll update
            let mut search_text = current_search;
            let mut search_changed = false;
            ui.horizontal(|ui| {
                ui.label("Search:");
                let response = ui
                    .add(egui::TextEdit::singleline(&mut search_text).desired_width(f32::INFINITY));
                search_changed = response.changed();
            });

            // Defer the set_channel_search call to avoid borrow issues
            if search_changed {
                self.set_channel_search(search_text.clone());
            }

            ui.add_space(5.0);

            // Channel count
            ui.label(format!(
                "Selected: {} / {} | Total: {}",
                selected_count, MAX_CHANNELS, channel_count
            ));

            ui.separator();

            // Channel list - use all remaining vertical space
            let search_lower = search_text.to_lowercase();
            let mut channel_to_add: Option<(usize, usize)> = None;
            let mut channel_to_remove: Option<usize> = None;

            // Sort channels: normalized fields first, then alphabetically
            // Collect channel names upfront to avoid borrow issues
            let file = &self.files[file_index];
            let sorted_channels = sort_channels_by_priority(
                file.log.channels.len(),
                |idx| file.log.channels[idx].name(),
                self.field_normalization,
                Some(&self.custom_normalizations),
            );

            // Get original names for all channels (needed for search)
            let channel_names: Vec<String> = (0..file.log.channels.len())
                .map(|idx| file.log.channels[idx].name())
                .collect();

            // Use cached channel data flags from the loaded file
            let channels_with_data = &file.channels_with_data;

            // Split channels into two groups: with data and without data
            let (channels_with, channels_without): (Vec<_>, Vec<_>) = sorted_channels
                .into_iter()
                .partition(|(idx, _, _)| channels_with_data[*idx]);

            // Get selected channels for comparison
            let selected_channels = self.get_selected_channels().to_vec();

            // Count how many channels match search in each group
            let count_matching = |channels: &[(usize, String, bool)]| -> usize {
                if search_lower.is_empty() {
                    channels.len()
                } else {
                    channels
                        .iter()
                        .filter(|(idx, display_name, _)| {
                            let original_name = &channel_names[*idx];
                            original_name.to_lowercase().contains(&search_lower)
                                || display_name.to_lowercase().contains(&search_lower)
                        })
                        .count()
                }
            };

            let with_data_count = count_matching(&channels_with);
            let without_data_count = count_matching(&channels_without);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    // Helper closure to render a channel item
                    let render_channel =
                        |ui: &mut egui::Ui,
                         channel_index: usize,
                         display_name: &str,
                         channel_to_add: &mut Option<(usize, usize)>,
                         channel_to_remove: &mut Option<usize>| {
                            let original_name = &channel_names[channel_index];

                            // Filter by search (search both original and normalized names)
                            if !search_lower.is_empty()
                                && !original_name.to_lowercase().contains(&search_lower)
                                && !display_name.to_lowercase().contains(&search_lower)
                            {
                                return;
                            }

                            // Check if already selected and get its index in selected_channels
                            let selected_idx = selected_channels.iter().position(|c| {
                                c.file_index == file_index && c.channel_index == channel_index
                            });
                            let is_selected = selected_idx.is_some();

                            // Build the label with checkmark prefix if selected
                            let label_text = if is_selected {
                                format!("[*] {}", display_name)
                            } else {
                                format!("[ ] {}", display_name)
                            };

                            let response = ui.selectable_label(is_selected, label_text);

                            if response.clicked() {
                                if let Some(idx) = selected_idx {
                                    // Already selected - remove it
                                    *channel_to_remove = Some(idx);
                                } else {
                                    // Not selected - add it
                                    *channel_to_add = Some((file_index, channel_index));
                                }
                            }
                            if response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        };

                    // Section 1: Channels with Data
                    if with_data_count > 0 || search_lower.is_empty() {
                        let header = egui::CollapsingHeader::new(
                            egui::RichText::new(format!(
                                "📊 Channels with Data ({})",
                                channels_with.len()
                            ))
                            .strong(),
                        )
                        .default_open(true)
                        .show(ui, |ui| {
                            for (channel_index, display_name, _is_normalized) in &channels_with {
                                render_channel(
                                    ui,
                                    *channel_index,
                                    display_name,
                                    &mut channel_to_add,
                                    &mut channel_to_remove,
                                );
                            }
                        });

                        // Show count of filtered results when searching
                        if !search_lower.is_empty() && header.body_returned.is_some() {
                            // Already showing filtered results inside
                        }
                    }

                    ui.add_space(5.0);

                    // Section 2: Channels without Data
                    if without_data_count > 0 || search_lower.is_empty() {
                        egui::CollapsingHeader::new(
                            egui::RichText::new(format!(
                                "📭 Empty Channels ({})",
                                channels_without.len()
                            ))
                            .color(egui::Color32::GRAY),
                        )
                        .default_open(false) // Collapsed by default
                        .show(ui, |ui| {
                            for (channel_index, display_name, _is_normalized) in &channels_without {
                                render_channel(
                                    ui,
                                    *channel_index,
                                    display_name,
                                    &mut channel_to_add,
                                    &mut channel_to_remove,
                                );
                            }
                        });
                    }
                });

            // Handle deferred channel removal (must happen before addition to keep indices valid)
            if let Some(idx) = channel_to_remove {
                self.remove_channel(idx);
            }

            // Handle deferred channel addition
            if let Some((file_idx, channel_idx)) = channel_to_add {
                self.add_channel(file_idx, channel_idx);
            }
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Select a file to view channels")
                        .italics()
                        .color(egui::Color32::GRAY),
                );
            });
        }
    }

    /// Render selected channel cards
    pub fn render_selected_channels(&mut self, ui: &mut egui::Ui) {
        ui.heading("Selected Channels");
        ui.separator();

        let use_normalization = self.field_normalization;

        // Get selected channels from the active tab
        let selected_channels = self.get_selected_channels().to_vec();

        // Pre-compute all display data to avoid borrow conflicts in closure
        struct ChannelCardData {
            color: egui::Color32,
            display_name: String,
            min_str: Option<String>,
            max_str: Option<String>,
            min_record: Option<usize>,
            max_record: Option<usize>,
            min_time: Option<f64>,
            max_time: Option<f64>,
        }

        let mut channel_cards: Vec<ChannelCardData> = Vec::with_capacity(selected_channels.len());

        for selected in &selected_channels {
            let color = self.get_channel_color(selected.color_index);
            let color32 = egui::Color32::from_rgb(color[0], color[1], color[2]);

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
                    let data = file.log.get_channel_data(selected.channel_index);
                    let times = file.log.get_times_as_f64();

                    if !data.is_empty() {
                        // Find min and max with their indices
                        let (min_idx, min_val) = data
                            .iter()
                            .enumerate()
                            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                            .map(|(i, v)| (i, *v))
                            .unwrap();
                        let (max_idx, max_val) = data
                            .iter()
                            .enumerate()
                            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                            .map(|(i, v)| (i, *v))
                            .unwrap();

                        let source_unit = selected.channel.unit();
                        let (conv_min, display_unit) =
                            self.unit_preferences.convert_value(min_val, source_unit);
                        let (conv_max, _) =
                            self.unit_preferences.convert_value(max_val, source_unit);
                        let unit_str = if display_unit.is_empty() {
                            String::new()
                        } else {
                            format!(" {}", display_unit)
                        };

                        (
                            Some(format!("{:.1}{}", conv_min, unit_str)),
                            Some(format!("{:.1}{}", conv_max, unit_str)),
                            Some(min_idx),
                            Some(max_idx),
                            times.get(min_idx).copied(),
                            times.get(max_idx).copied(),
                        )
                    } else {
                        (None, None, None, None, None, None)
                    }
                } else {
                    (None, None, None, None, None, None)
                };

            channel_cards.push(ChannelCardData {
                color: color32,
                display_name,
                min_str,
                max_str,
                min_record,
                max_record,
                min_time,
                max_time,
            });
        }

        let mut channel_to_remove: Option<usize> = None;
        let mut jump_to: Option<(usize, f64)> = None; // (record, time)

        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, card) in channel_cards.iter().enumerate() {
                    egui::Frame::NONE
                        .fill(egui::Color32::from_rgb(40, 40, 40))
                        .stroke(egui::Stroke::new(2.0, card.color))
                        .corner_radius(5)
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(&card.display_name)
                                            .strong()
                                            .color(card.color),
                                    );
                                    let close_btn = ui.small_button("x");
                                    if close_btn.clicked() {
                                        channel_to_remove = Some(i);
                                    }
                                    if close_btn.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                });

                                // Show min with jump button
                                if let Some(min_str) = &card.min_str {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("Min:")
                                                .color(egui::Color32::GRAY)
                                                .small(),
                                        );
                                        ui.label(
                                            egui::RichText::new(min_str)
                                                .color(egui::Color32::LIGHT_GRAY),
                                        );
                                        if let (Some(record), Some(time)) =
                                            (card.min_record, card.min_time)
                                        {
                                            let btn = ui
                                                .small_button("⏵")
                                                .on_hover_text("Jump to minimum");
                                            if btn.clicked() {
                                                jump_to = Some((record, time));
                                            }
                                            if btn.hovered() {
                                                ui.ctx().set_cursor_icon(
                                                    egui::CursorIcon::PointingHand,
                                                );
                                            }
                                        }
                                    });
                                }

                                // Show max with jump button
                                if let Some(max_str) = &card.max_str {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("Max:")
                                                .color(egui::Color32::GRAY)
                                                .small(),
                                        );
                                        ui.label(
                                            egui::RichText::new(max_str)
                                                .color(egui::Color32::LIGHT_GRAY),
                                        );
                                        if let (Some(record), Some(time)) =
                                            (card.max_record, card.max_time)
                                        {
                                            let btn = ui
                                                .small_button("⏵")
                                                .on_hover_text("Jump to maximum");
                                            if btn.clicked() {
                                                jump_to = Some((record, time));
                                            }
                                            if btn.hovered() {
                                                ui.ctx().set_cursor_icon(
                                                    egui::CursorIcon::PointingHand,
                                                );
                                            }
                                        }
                                    });
                                }
                            });
                        });

                    ui.add_space(5.0);
                }
            });
        });

        // Handle jump to min/max
        if let Some((record, time)) = jump_to {
            self.set_cursor_time(Some(time));
            self.set_cursor_record(Some(record));
            // Request the chart to center on this time
            self.set_jump_to_time(Some(time));
            // Stop playback when jumping
            self.is_playing = false;
            self.last_frame_time = None;
        }

        if let Some(index) = channel_to_remove {
            self.remove_channel(index);
        }

        if selected_channels.is_empty() {
            ui.label(
                egui::RichText::new("Click channels to add them to the chart")
                    .italics()
                    .color(egui::Color32::GRAY),
            );
        }
    }
}
