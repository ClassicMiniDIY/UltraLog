//! Tool Properties panel - dynamic panel that shows controls for the current tool.
//!
//! For Log Viewer: Shows channel selection (former channels panel)
//! For Histogram: Shows histogram controls (axes, grid, ranges, filters, etc.)
//! For Scatter Plot: Shows scatter plot controls

use eframe::egui;
use rust_i18n::t;

use crate::app::UltraLogApp;
use crate::normalize::sort_channels_by_priority;
use crate::state::{ActiveTool, MAX_CHANNELS};

impl UltraLogApp {
    /// Render the tool properties panel content (called from side_panel.rs)
    /// Routes to the appropriate sub-panel based on active_tool
    pub fn render_tool_properties_panel_content(&mut self, ui: &mut egui::Ui) {
        match self.active_tool {
            ActiveTool::LogViewer => self.render_log_viewer_properties(ui),
            ActiveTool::Histogram => self.render_histogram_properties(ui),
            ActiveTool::ScatterPlot => self.render_scatter_plot_properties(ui),
        }
    }

    /// Render Log Viewer properties (channel selection)
    fn render_log_viewer_properties(&mut self, ui: &mut egui::Ui) {
        // Pre-compute scaled font sizes
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);
        let _font_16 = self.scaled_font(16.0);

        // Get active tab info including stacked mode
        let tab_info = self.active_tab.and_then(|tab_idx| {
            let tab = &self.tabs[tab_idx];
            if tab.file_index < self.files.len() {
                Some((
                    tab.file_index,
                    tab.channel_search.clone(),
                    tab.selected_channels.len(),
                    tab.stacked_mode,
                ))
            } else {
                None
            }
        });

        if let Some((file_index, current_search, selected_count, stacked_mode)) = tab_info {
            let channel_count = self.files[file_index].log.channels.len();

            // Stacked Mode Toggle Section - Pill Style
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Plot Mode:").size(font_14).strong());
                });
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    // Single Plot button
                    let is_single = !stacked_mode;
                    let single_fill = if is_single {
                        egui::Color32::from_rgb(70, 70, 70)
                    } else {
                        egui::Color32::from_rgb(45, 45, 45)
                    };
                    let single_text_color = if is_single {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_rgb(180, 180, 180)
                    };
                    let single_stroke = if is_single {
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(113, 120, 78))
                    } else {
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 80))
                    };

                    let single_btn = ui.add(
                        egui::Button::new(
                            egui::RichText::new("Single Plot")
                                .size(font_14)
                                .color(single_text_color),
                        )
                        .fill(single_fill)
                        .stroke(single_stroke)
                        .corner_radius(egui::CornerRadius::same(16))
                        .min_size(egui::vec2(100.0, 32.0)),
                    );

                    if single_btn.clicked() && stacked_mode {
                        self.toggle_stacked_mode();
                    }
                    if single_btn.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    ui.add_space(4.0);

                    // Stacked Plots button
                    let is_stacked = stacked_mode;
                    let stacked_fill = if is_stacked {
                        egui::Color32::from_rgb(70, 70, 70)
                    } else {
                        egui::Color32::from_rgb(45, 45, 45)
                    };
                    let stacked_text_color = if is_stacked {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_rgb(180, 180, 180)
                    };
                    let stacked_stroke = if is_stacked {
                        egui::Stroke::new(1.5, egui::Color32::from_rgb(113, 120, 78))
                    } else {
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 80))
                    };

                    let stacked_btn = ui.add(
                        egui::Button::new(
                            egui::RichText::new("Stacked Plots")
                                .size(font_14)
                                .color(stacked_text_color),
                        )
                        .fill(stacked_fill)
                        .stroke(stacked_stroke)
                        .corner_radius(egui::CornerRadius::same(16))
                        .min_size(egui::vec2(100.0, 32.0)),
                    );

                    if stacked_btn.clicked() && !stacked_mode {
                        self.toggle_stacked_mode();
                    }
                    if stacked_btn.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Computed Channels button
            let primary_color = egui::Color32::from_rgb(113, 120, 78);
            let computed_btn = egui::Frame::NONE
                .fill(primary_color)
                .corner_radius(4)
                .inner_margin(egui::vec2(10.0, 6.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("\u{0192}")
                                .color(egui::Color32::WHITE)
                                .size(font_14),
                        );
                        ui.label(
                            egui::RichText::new(t!("channels.computed_channels"))
                                .color(egui::Color32::WHITE)
                                .size(font_14),
                        );
                    });
                });

            if computed_btn
                .response
                .interact(egui::Sense::click())
                .on_hover_text(t!("channels.computed_channels_tooltip"))
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
                            egui::RichText::new("\u{1F50D}")
                                .size(font_14)
                                .color(egui::Color32::GRAY),
                        );
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut search_text)
                                .hint_text(t!("channels.search_hint"))
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
                egui::RichText::new(t!(
                    "channels.selected_count",
                    selected = selected_count,
                    max = MAX_CHANNELS,
                    total = channel_count
                ))
                .size(font_12)
                .color(egui::Color32::GRAY),
            );

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // Render appropriate view based on mode
            if stacked_mode {
                self.render_stacked_plot_list(ui, file_index, &search_text);
            } else {
                self.render_channel_list_compact(ui, file_index, &search_text);
            }
        } else {
            // No file selected
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("\u{1F4CA}")
                        .size(32.0)
                        .color(egui::Color32::from_rgb(100, 100, 100)),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(t!("channels.no_file_selected"))
                        .size(font_14)
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(t!("channels.load_file_to_view"))
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
                        let check = if is_selected { "\u{2611}" } else { "\u{2610}" };
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
                        egui::RichText::new(format!(
                            "\u{1F4CA} {}",
                            t!("channels.with_data", count = channels_with.len())
                        ))
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
                        egui::RichText::new(format!(
                            "\u{1F4ED} {}",
                            t!("channels.empty", count = channels_without.len())
                        ))
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

    /// Render stacked plot list with channels organized by plot
    fn render_stacked_plot_list(&mut self, ui: &mut egui::Ui, file_index: usize, search: &str) {
        let font_14 = self.scaled_font(14.0);
        let search_lower = search.to_lowercase();

        let Some(tab_idx) = self.active_tab else {
            return;
        };

        let plot_areas = self.tabs[tab_idx].plot_areas.clone();
        let selected_channels = self.tabs[tab_idx].selected_channels.clone();

        // Clone needed data to avoid borrow checker issues
        let channel_count = self.files[file_index].log.channels.len();
        let channel_names: Vec<String> = (0..channel_count)
            .map(|idx| self.files[file_index].log.channels[idx].name())
            .collect();
        let channels_with_data = self.files[file_index].channels_with_data.clone();

        let mut channel_to_add: Option<(usize, usize, usize)> = None; // (file_idx, channel_idx, plot_id)
        let mut channel_to_remove: Option<usize> = None;
        let mut plot_to_delete: Option<usize> = None;

        // "+ New Plot" button
        let new_plot_btn = egui::Frame::NONE
            .fill(egui::Color32::from_rgb(71, 108, 155))
            .corner_radius(4)
            .inner_margin(egui::vec2(10.0, 6.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("+ New Plot")
                            .color(egui::Color32::WHITE)
                            .size(font_14),
                    );
                });
            });

        if new_plot_btn
            .response
            .interact(egui::Sense::click())
            .on_hover_text("Create a new plot area")
            .clicked()
        {
            let next_id = self.tabs[tab_idx].next_plot_area_id;
            let name = format!("Plot {}", next_id + 1);
            self.create_plot_area(name);
        }

        if new_plot_btn.response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        ui.add_space(8.0);

        // Render each plot area with its channels
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for plot_area in &plot_areas {
                    let plot_channels: Vec<(usize, String)> = plot_area
                        .channel_indices
                        .iter()
                        .filter_map(|&idx| {
                            selected_channels.get(idx).map(|ch| {
                                let name = ch.channel.name();
                                (idx, name)
                            })
                        })
                        .collect();

                    let plot_id = plot_area.id;
                    let has_capacity = plot_area.has_capacity();

                    ui.push_id(format!("plot_{}", plot_id), |ui| {
                        // Custom header with delete button
                        let header_response = ui
                            .horizontal(|ui| {
                                let header_text =
                                    format!("{} ({}/10)", plot_area.name, plot_channels.len());

                                // Collapsing state
                                let id = ui.id().with(plot_id);
                                let mut open = ui.data(|d| d.get_temp::<bool>(id).unwrap_or(true));

                                // Arrow icon (custom drawn triangle)
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(16.0, 16.0),
                                    egui::Sense::click(),
                                );
                                let center = rect.center();
                                let color = if response.hovered() {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::LIGHT_GRAY
                                };

                                if open {
                                    crate::ui::icons::draw_triangle_down(ui, center, 12.0, color);
                                } else {
                                    crate::ui::icons::draw_triangle_right(ui, center, 12.0, color);
                                }

                                if response.clicked() {
                                    open = !open;
                                    ui.data_mut(|d| d.insert_temp(id, open));
                                }
                                if response.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }

                                // Header label
                                ui.label(
                                    egui::RichText::new(&header_text)
                                        .size(font_14)
                                        .strong()
                                        .color(egui::Color32::from_rgb(159, 166, 119)),
                                );

                                // Right-aligned delete button (only if not last plot)
                                if plot_areas.len() > 1 {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let delete_btn = ui.small_button("🗑");
                                            if delete_btn.clicked() {
                                                plot_to_delete = Some(plot_id);
                                            }
                                            if delete_btn.hovered() {
                                                ui.ctx().set_cursor_icon(
                                                    egui::CursorIcon::PointingHand,
                                                );
                                            }
                                            delete_btn.on_hover_text("Delete this plot");
                                        },
                                    );
                                }

                                open
                            })
                            .inner;

                        // Body content (if open)
                        if header_response {
                            ui.indent(format!("plot_body_{}", plot_id), |ui| {
                                // Allocate full width for drop zone
                                let available_width = ui.available_width();
                                let desired_height = if plot_channels.is_empty() {
                                    80.0
                                } else {
                                    (plot_channels.len() as f32 * 30.0) + 20.0
                                };

                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(available_width, desired_height),
                                    egui::Sense::hover(),
                                );

                                // Check for dropped channel
                                if let Some(payload) =
                                    response.dnd_release_payload::<(usize, usize)>()
                                {
                                    if has_capacity {
                                        let (dropped_file_idx, dropped_channel_idx) = *payload;
                                        channel_to_add =
                                            Some((dropped_file_idx, dropped_channel_idx, plot_id));
                                    }
                                }

                                // Highlight as drop zone when hovering with drag payload
                                if response.dnd_hover_payload::<(usize, usize)>().is_some()
                                    && has_capacity
                                {
                                    ui.painter().rect_stroke(
                                        rect,
                                        egui::CornerRadius::same(4),
                                        egui::Stroke::new(
                                            2.0,
                                            egui::Color32::from_rgb(71, 108, 155),
                                        ),
                                        egui::StrokeKind::Inside,
                                    );
                                }

                                // Draw content on top of the drop zone
                                let mut child_ui =
                                    ui.new_child(egui::UiBuilder::new().max_rect(rect));

                                if plot_channels.is_empty() {
                                    child_ui.vertical_centered(|ui| {
                                        ui.add_space(30.0);
                                        ui.label(
                                            egui::RichText::new("Drag channels here")
                                                .size(font_14)
                                                .color(egui::Color32::GRAY)
                                                .italics(),
                                        );
                                    });
                                } else {
                                    child_ui.add_space(10.0);
                                    for (channel_idx, channel_name) in &plot_channels {
                                        let frame = egui::Frame::NONE
                                            .fill(egui::Color32::from_rgb(55, 60, 50))
                                            .corner_radius(3)
                                            .inner_margin(egui::Margin::symmetric(6, 3));

                                        let response = frame
                                            .show(&mut child_ui, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("✓")
                                                            .size(font_14)
                                                            .color(egui::Color32::WHITE),
                                                    );
                                                    ui.label(
                                                        egui::RichText::new(channel_name)
                                                            .size(font_14)
                                                            .color(egui::Color32::WHITE),
                                                    );
                                                });
                                            })
                                            .response
                                            .interact(egui::Sense::click())
                                            .on_hover_text("Click to remove channel");

                                        if response.clicked() {
                                            channel_to_remove = Some(*channel_idx);
                                        }

                                        if response.hovered() {
                                            child_ui
                                                .ctx()
                                                .set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                    }
                                }
                            });
                        }
                    });

                    ui.add_space(6.0);
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Available channels section
                ui.label(
                    egui::RichText::new("Available Channels")
                        .size(font_14)
                        .strong(),
                );
                ui.add_space(4.0);

                // Get all available channels not yet selected
                let sorted_channels = sort_channels_by_priority(
                    channel_count,
                    |idx| channel_names[idx].clone(),
                    self.field_normalization,
                    Some(&self.custom_normalizations),
                );

                // Partition into channels with and without data
                let (channels_with, channels_without): (Vec<_>, Vec<_>) = sorted_channels
                    .into_iter()
                    .filter(|(idx, _, _)| {
                        // Filter out already selected channels
                        !selected_channels
                            .iter()
                            .any(|c| c.file_index == file_index && c.channel_index == *idx)
                    })
                    .partition(|(idx, _, _)| channels_with_data[*idx]);

                // Render function for draggable channels
                let render_draggable_channel =
                    |ui: &mut egui::Ui,
                     channel_index: usize,
                     display_name: &str,
                     is_empty: bool| {
                        let original_name = &channel_names[channel_index];

                        // Skip if doesn't match search
                        if !search_lower.is_empty()
                            && !original_name.to_lowercase().contains(&search_lower)
                            && !display_name.to_lowercase().contains(&search_lower)
                        {
                            return;
                        }

                        let text_color = if is_empty {
                            egui::Color32::from_rgb(100, 100, 100)
                        } else {
                            egui::Color32::LIGHT_GRAY
                        };

                        // Make channel draggable
                        let id =
                            egui::Id::new(format!("drag_channel_{}_{}", file_index, channel_index));
                        let response = ui
                            .scope(|ui| {
                                ui.dnd_drag_source(id, (file_index, channel_index), |ui| {
                                    // Sense hover to change background
                                    let is_hovered = ui.ui_contains_pointer();

                                    let fill_color = if is_hovered {
                                        egui::Color32::from_rgb(60, 60, 60)
                                    } else {
                                        egui::Color32::from_rgb(45, 45, 45)
                                    };

                                    let frame = egui::Frame::NONE
                                        .fill(fill_color)
                                        .corner_radius(4)
                                        .inner_margin(egui::Margin::symmetric(10, 8));

                                    frame.show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            // Larger, more prominent grab handle icon
                                            ui.label(egui::RichText::new("☰").size(font_14).color(
                                                if is_hovered {
                                                    egui::Color32::WHITE
                                                } else {
                                                    text_color
                                                },
                                            ));
                                            ui.add_space(4.0);
                                            ui.label(
                                                egui::RichText::new(display_name)
                                                    .size(font_14)
                                                    .color(if is_hovered {
                                                        egui::Color32::WHITE
                                                    } else {
                                                        text_color
                                                    }),
                                            );
                                        });
                                    });
                                });
                            })
                            .response;

                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                        }
                    };

                // Channels with data
                if !channels_with.is_empty() {
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!(
                            "\u{1F4CA} Channels with Data ({})",
                            channels_with.len()
                        ))
                        .size(font_14)
                        .strong(),
                    )
                    .default_open(true)
                    .show(ui, |ui| {
                        for (channel_index, display_name, _) in &channels_with {
                            render_draggable_channel(ui, *channel_index, display_name, false);
                        }
                    });
                }

                ui.add_space(4.0);

                // Empty channels
                if !channels_without.is_empty() {
                    egui::CollapsingHeader::new(
                        egui::RichText::new(format!(
                            "\u{1F4ED} Empty Channels ({})",
                            channels_without.len()
                        ))
                        .size(font_14)
                        .color(egui::Color32::GRAY),
                    )
                    .default_open(false)
                    .show(ui, |ui| {
                        for (channel_index, display_name, _) in &channels_without {
                            render_draggable_channel(ui, *channel_index, display_name, true);
                        }
                    });
                }
            });

        // Apply deferred actions
        if let Some(idx) = channel_to_remove {
            self.remove_channel(idx);
        }

        if let Some((file_idx, channel_idx, plot_id)) = channel_to_add {
            self.add_channel_to_plot(file_idx, channel_idx, plot_id);
        }

        if let Some(plot_id) = plot_to_delete {
            self.delete_plot_area(plot_id);
        }
    }

    /// Render Histogram properties (controls for histogram tool)
    fn render_histogram_properties(&mut self, ui: &mut egui::Ui) {
        // Call the histogram controls function from histogram.rs
        self.render_histogram_controls(ui);
    }

    /// Render Scatter Plot properties (controls for scatter plot tool)
    fn render_scatter_plot_properties(&mut self, ui: &mut egui::Ui) {
        // Call the scatter plot controls function from scatter_plot.rs
        self.render_scatter_plot_controls(ui);
    }
}
