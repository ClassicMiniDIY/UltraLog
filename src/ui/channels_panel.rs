//! Channels panel - channel selection and selected channel cards.
//!
//! This panel provides channel selection functionality that works across all tool modes.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::normalize::sort_channels_by_priority;
use crate::state::MAX_CHANNELS;

impl UltraLogApp {
    /// Render the channels panel content (called from side_panel.rs)
    pub fn render_channels_panel_content(&mut self, ui: &mut egui::Ui) {
        // Pre-compute scaled font sizes
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);
        let _font_16 = self.scaled_font(16.0);

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

            // Computed Channels button
            let primary_color = egui::Color32::from_rgb(113, 120, 78);
            let computed_btn = egui::Frame::NONE
                .fill(primary_color)
                .corner_radius(4)
                .inner_margin(egui::vec2(10.0, 6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("ƒ")
                                .color(egui::Color32::WHITE)
                                .size(font_14),
                        );
                        ui.label(
                            egui::RichText::new("Computed Channels")
                                .color(egui::Color32::WHITE)
                                .size(font_14),
                        );
                    });
                });

            if computed_btn
                .response
                .interact(egui::Sense::click())
                .on_hover_text("Create virtual channels from mathematical formulas")
                .clicked()
            {
                self.show_computed_channels_manager = true;
            }

            if computed_btn.response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            ui.add_space(8.0);

            // Search box
            let mut search_text = current_search;
            let mut search_changed = false;
            egui::Frame::NONE
                .fill(egui::Color32::from_rgb(50, 50, 50))
                .corner_radius(4)
                .inner_margin(egui::vec2(8.0, 6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("🔍")
                                .size(font_14)
                                .color(egui::Color32::GRAY),
                        );
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut search_text)
                                .hint_text("Search channels...")
                                .desired_width(f32::INFINITY)
                                .frame(false),
                        );
                        search_changed = response.changed();
                    });
                });

            if search_changed {
                self.set_channel_search(search_text.clone());
            }

            ui.add_space(4.0);

            // Channel count
            ui.label(
                egui::RichText::new(format!(
                    "Selected: {} / {} | Total: {}",
                    selected_count, MAX_CHANNELS, channel_count
                ))
                .size(font_12)
                .color(egui::Color32::GRAY),
            );

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Channel list
            self.render_channel_list_compact(ui, file_index, &search_text);
        } else {
            // No file selected
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("📊")
                        .size(32.0)
                        .color(egui::Color32::from_rgb(100, 100, 100)),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("No file selected")
                        .size(font_14)
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Load a file to view channels")
                        .size(font_12)
                        .color(egui::Color32::from_rgb(100, 100, 100)),
                );
            });
        }
    }

    /// Render the channel list in compact form
    fn render_channel_list_compact(&mut self, ui: &mut egui::Ui, file_index: usize, search: &str) {
        let font_14 = self.scaled_font(14.0);
        let search_lower = search.to_lowercase();

        let mut channel_to_add: Option<(usize, usize)> = None;
        let mut channel_to_remove: Option<usize> = None;

        let file = &self.files[file_index];
        let sorted_channels = sort_channels_by_priority(
            file.log.channels.len(),
            |idx| file.log.channels[idx].name(),
            self.field_normalization,
            Some(&self.custom_normalizations),
        );

        let channel_names: Vec<String> = (0..file.log.channels.len())
            .map(|idx| file.log.channels[idx].name())
            .collect();

        let channels_with_data = &file.channels_with_data;
        let (channels_with, channels_without): (Vec<_>, Vec<_>) = sorted_channels
            .into_iter()
            .partition(|(idx, _, _)| channels_with_data[*idx]);

        let selected_channels = self.get_selected_channels().to_vec();

        let render_channel = |ui: &mut egui::Ui,
                              channel_index: usize,
                              display_name: &str,
                              is_empty: bool,
                              channel_to_add: &mut Option<(usize, usize)>,
                              channel_to_remove: &mut Option<usize>| {
            let original_name = &channel_names[channel_index];

            if !search_lower.is_empty()
                && !original_name.to_lowercase().contains(&search_lower)
                && !display_name.to_lowercase().contains(&search_lower)
            {
                return;
            }

            let selected_idx = selected_channels
                .iter()
                .position(|c| c.file_index == file_index && c.channel_index == channel_index);
            let is_selected = selected_idx.is_some();

            let text_color = if is_empty {
                egui::Color32::from_rgb(100, 100, 100)
            } else if is_selected {
                egui::Color32::WHITE
            } else {
                egui::Color32::LIGHT_GRAY
            };

            let bg_color = if is_selected {
                egui::Color32::from_rgb(55, 60, 50)
            } else {
                egui::Color32::TRANSPARENT
            };

            let frame = egui::Frame::NONE
                .fill(bg_color)
                .corner_radius(3)
                .inner_margin(egui::Margin::symmetric(6, 3));

            let response = frame
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let check = if is_selected { "☑" } else { "☐" };
                        ui.label(egui::RichText::new(check).size(font_14).color(text_color));
                        ui.label(
                            egui::RichText::new(display_name)
                                .size(font_14)
                                .color(text_color),
                        );
                    });
                })
                .response
                .interact(egui::Sense::click());

            if response.clicked() {
                if let Some(idx) = selected_idx {
                    *channel_to_remove = Some(idx);
                } else {
                    *channel_to_add = Some((file_index, channel_index));
                }
            }

            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        };

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Channels with data
                if !channels_with.is_empty() {
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!("📊 With Data ({})", channels_with.len()))
                            .size(font_14)
                            .strong(),
                    )
                    .default_open(true)
                    .show(ui, |ui| {
                        for (channel_index, display_name, _) in &channels_with {
                            render_channel(
                                ui,
                                *channel_index,
                                display_name,
                                false,
                                &mut channel_to_add,
                                &mut channel_to_remove,
                            );
                        }
                    });
                }

                ui.add_space(4.0);

                // Empty channels
                if !channels_without.is_empty() {
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!("📭 Empty ({})", channels_without.len()))
                            .size(font_14)
                            .color(egui::Color32::GRAY),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        for (channel_index, display_name, _) in &channels_without {
                            render_channel(
                                ui,
                                *channel_index,
                                display_name,
                                true,
                                &mut channel_to_add,
                                &mut channel_to_remove,
                            );
                        }
                    });
                }
            });

        if let Some(idx) = channel_to_remove {
            self.remove_channel(idx);
        }

        if let Some((file_idx, channel_idx)) = channel_to_add {
            self.add_channel(file_idx, channel_idx);
        }
    }
}
