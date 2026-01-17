//! Chart rendering and data processing utilities.

use eframe::egui;
use egui_plot::{Line, Plot, PlotBounds, PlotPoints, VLine};
use rust_i18n::t;

use crate::app::UltraLogApp;
use crate::normalize::normalize_channel_name_with_custom;
use crate::state::{
    CacheKey, PlotArea, SelectedChannel, CHART_COLORS, COLORBLIND_COLORS, MAX_CHART_POINTS,
    MIN_PLOT_HEIGHT, PLOT_RESIZE_HANDLE_HEIGHT,
};

impl UltraLogApp {
    /// Render the main chart with cached downsampled data
    pub fn render_chart(&mut self, ui: &mut egui::Ui) {
        // Check if stacked mode is enabled
        let stacked_mode = self
            .active_tab
            .map(|idx| self.tabs[idx].stacked_mode)
            .unwrap_or(false);

        if stacked_mode {
            self.render_chart_stacked_mode(ui);
        } else {
            self.render_chart_single_mode(ui);
        }
    }

    /// Render single-plot mode chart (original implementation)
    fn render_chart_single_mode(&mut self, ui: &mut egui::Ui) {
        // Get selected channels from active tab
        let selected_channels = self.get_selected_channels().to_vec();

        if selected_channels.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(t!("chart.select_channels"))
                        .size(self.scaled_font(20.0))
                        .color(egui::Color32::GRAY),
                );
            });
            return;
        }

        // Pre-compute and cache downsampled + normalized data for all selected channels
        for selected in &selected_channels {
            if selected.file_index >= self.files.len() {
                continue;
            }

            let cache_key = CacheKey {
                file_index: selected.file_index,
                channel_index: selected.channel_index,
                plot_area_id: 0, // Single-plot mode uses plot_area_id 0
            };

            if !self.downsample_cache.contains_key(&cache_key) {
                let file = &self.files[selected.file_index];
                let times = file.log.get_times_as_f64();
                // Use app method to get channel data (handles both regular and computed channels)
                let data = self.get_channel_data(selected.file_index, selected.channel_index);

                if times.len() == data.len() && !times.is_empty() {
                    let downsampled = Self::downsample_lttb(times, &data, MAX_CHART_POINTS);
                    // Normalize Y values to 0-1 range so all channels overlay
                    let normalized = Self::normalize_points(&downsampled);
                    self.downsample_cache.insert(cache_key, normalized);
                }
            }
        }

        // Pre-compute legend names with current values at cursor position
        let use_normalization = self.field_normalization;
        let custom_mappings = &self.custom_normalizations;
        let legend_names: Vec<String> = selected_channels
            .iter()
            .map(|selected| {
                let original_name = selected.channel.name();
                let base_name = if use_normalization {
                    normalize_channel_name_with_custom(&original_name, Some(custom_mappings))
                } else {
                    original_name
                };
                if let Some(record) = self.get_cursor_record() {
                    if let Some(value) = self.get_value_at_record(
                        selected.file_index,
                        selected.channel_index,
                        record,
                    ) {
                        let source_unit = selected.channel.unit();
                        let (converted_value, display_unit) =
                            self.unit_preferences.convert_value(value, source_unit);
                        if display_unit.is_empty() {
                            format!("{}: {:.2}", base_name, converted_value)
                        } else {
                            format!("{}: {:.2} {}", base_name, converted_value, display_unit)
                        }
                    } else {
                        base_name
                    }
                } else {
                    base_name
                }
            })
            .collect();

        // Prepare data for the plot closure (can't borrow self mutably inside)
        let cache = &self.downsample_cache;
        let files = &self.files;
        // selected_channels already defined at top of function from get_selected_channels()
        let cursor_time = self.get_cursor_time();
        let cursor_tracking = self.cursor_tracking;
        let view_window = self.view_window_seconds;
        let time_range = self.get_time_range();
        let color_blind_mode = self.color_blind_mode;
        let chart_interacted = self.get_chart_interacted();
        let initial_view_seconds = self.initial_view_seconds;
        let jump_to_time = self.get_jump_to_time();

        // Fixed Y bounds for normalized data (0-1 with small padding)
        const Y_MIN: f64 = -0.05;
        const Y_MAX: f64 = 1.05;

        // Build the plot - X-axis zoom only, Y fixed
        let plot = Plot::new("log_chart")
            .legend(egui_plot::Legend::default())
            .y_axis_label("") // Hide Y axis label since values are normalized
            .show_axes([true, false]) // Show X axis (time), hide Y axis (normalized 0-1)
            .allow_zoom([true, false]) // Only allow X-axis zoom
            .allow_drag([!cursor_tracking, false]) // Only allow X-axis drag, never Y
            .allow_scroll([!cursor_tracking, false]); // Only allow X-axis scroll, never Y

        let response = plot.show(ui, |plot_ui| {
            // Get current bounds
            let current_bounds = plot_ui.plot_bounds();
            let mut x_min = current_bounds.min()[0];
            let mut x_max = current_bounds.max()[0];

            // Handle jump-to-time request (from min/max jump buttons)
            if let (Some(jump_time), Some((min_t, max_t))) = (jump_to_time, time_range) {
                // Center the view on the jump target time
                let current_width = (x_max - x_min).max(view_window);
                let half_width = current_width / 2.0;
                x_min = (jump_time - half_width).max(min_t);
                x_max = (jump_time + half_width).min(max_t);
                // Adjust if we hit a boundary
                if x_max - x_min < current_width {
                    if x_min == min_t {
                        x_max = (min_t + current_width).min(max_t);
                    } else {
                        x_min = (max_t - current_width).max(min_t);
                    }
                }
            } else if cursor_tracking {
                // In cursor tracking mode, center on cursor
                if let (Some(cursor), Some((min_t, max_t))) = (cursor_time, time_range) {
                    let half_window = view_window / 2.0;
                    x_min = (cursor - half_window).max(min_t);
                    x_max = (cursor + half_window).min(max_t);
                }
            } else if let Some((min_t, max_t)) = time_range {
                let data_width = max_t - min_t;

                // If chart hasn't been interacted with yet, use initial zoomed view
                if !chart_interacted && data_width > initial_view_seconds {
                    // Show initial view window starting from the beginning
                    x_min = min_t;
                    x_max = min_t + initial_view_seconds;
                } else {
                    // Clamp X bounds to data range - prevent zooming out beyond data
                    let current_width = x_max - x_min;

                    // Don't allow view wider than data range
                    if current_width > data_width {
                        x_min = min_t;
                        x_max = max_t;
                    } else {
                        // Keep view within data bounds
                        if x_min < min_t {
                            x_min = min_t;
                            x_max = min_t + current_width;
                        }
                        if x_max > max_t {
                            x_max = max_t;
                            x_min = max_t - current_width;
                        }
                    }
                }
            }

            // Always enforce bounds: X clamped to data, Y fixed to normalized range
            let new_bounds = PlotBounds::from_min_max([x_min, Y_MIN], [x_max, Y_MAX]);
            plot_ui.set_plot_bounds(new_bounds);

            // Draw channel data lines with values in legend
            for (i, selected) in selected_channels.iter().enumerate() {
                if selected.file_index >= files.len() {
                    continue;
                }

                let cache_key = CacheKey {
                    file_index: selected.file_index,
                    channel_index: selected.channel_index,
                    plot_area_id: 0, // Single-plot mode uses plot_area_id 0
                };

                if let Some(points) = cache.get(&cache_key) {
                    let plot_points: PlotPoints = points.iter().copied().collect();
                    let palette = if color_blind_mode {
                        COLORBLIND_COLORS
                    } else {
                        CHART_COLORS
                    };
                    let color = palette[selected.color_index % palette.len()];

                    // Use legend name with value if available
                    let name = &legend_names[i];

                    plot_ui.line(
                        Line::new(name.clone(), plot_points)
                            .color(egui::Color32::from_rgb(color[0], color[1], color[2]))
                            .width(1.5),
                    );
                }
            }

            // Draw vertical cursor line
            if let Some(time) = cursor_time {
                plot_ui.vline(
                    VLine::new("Cursor", time)
                        .color(egui::Color32::from_rgb(0, 255, 255)) // Cyan cursor
                        .width(2.0),
                );
            }

            // Return pointer position if hovering for click detection
            plot_ui.pointer_coordinate()
        });

        // Detect user interaction with chart (drag, zoom, scroll)
        // This marks the chart as "interacted" so we stop using the initial zoomed view
        if response.response.dragged()
            || response.response.drag_started()
            || ui.input(|i| i.zoom_delta() != 1.0)
            || ui.input(|i| i.smooth_scroll_delta.x != 0.0)
        {
            self.set_chart_interacted(true);
        }

        // Clear jump-to-time request after it's been processed
        if self.get_jump_to_time().is_some() {
            self.clear_jump_to_time();
            // Mark chart as interacted so future jumps work correctly
            self.set_chart_interacted(true);
        }

        // Handle click on chart to set cursor position
        if response.response.clicked() {
            if let Some(pos) = response.inner {
                let clicked_time = pos.x;
                // Clamp to time range
                if let Some((min, max)) = self.get_time_range() {
                    // Stop playback when user clicks on chart
                    self.is_playing = false;
                    self.last_frame_time = None;

                    let clamped_time = clicked_time.clamp(min, max);
                    self.set_cursor_time(Some(clamped_time));
                    let record = self.find_record_at_time(clamped_time);
                    self.set_cursor_record(record);
                    // Force repaint to update legend values immediately
                    ui.ctx().request_repaint();
                }
            }
        }
    }

    /// Render stacked plot areas
    fn render_chart_stacked_mode(&mut self, ui: &mut egui::Ui) {
        let Some(tab_idx) = self.active_tab else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("No active tab")
                        .size(self.scaled_font(20.0))
                        .color(egui::Color32::GRAY),
                );
            });
            return;
        };

        let plot_areas = self.tabs[tab_idx].plot_areas.clone();
        let selected_channels = self.tabs[tab_idx].selected_channels.clone();

        if selected_channels.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(t!("chart.select_channels"))
                        .size(self.scaled_font(20.0))
                        .color(egui::Color32::GRAY),
                );
            });
            return;
        }

        // Track resize drag
        let mut resize_drag: Option<(usize, f32)> = None;

        // Get available height to constrain scroll area
        let max_scroll_height = ui.available_height();

        // Wrap in scroll area to allow vertical scrolling when plots don't fit
        egui::ScrollArea::vertical()
            .id_salt("stacked_plots_scroll")
            .max_height(max_scroll_height)
            .auto_shrink([false; 2])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show(ui, |ui| {
                // Render each plot area
                for (plot_idx, plot_area) in plot_areas.iter().enumerate() {
                    // Skip collapsed plots (just show header)
                    if plot_area.collapsed {
                        self.render_plot_area_header_collapsed(ui, plot_area, plot_idx);
                        ui.add_space(5.0);
                        continue;
                    }

                    // Use the plot's own pixel height
                    let plot_height = plot_area.height_pixels.max(MIN_PLOT_HEIGHT);

                    // Render plot area header
                    ui.horizontal(|ui| {
                        self.render_plot_area_header(ui, plot_area, plot_idx);
                    });

                    ui.add_space(5.0);

                    // Get channels for this plot
                    let plot_channels: Vec<SelectedChannel> = plot_area
                        .channel_indices
                        .iter()
                        .filter_map(|&idx| selected_channels.get(idx).cloned())
                        .collect();

                    if plot_channels.is_empty() {
                        // Empty plot area with drop zone
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), plot_height),
                            egui::Sense::hover(),
                        );

                        // Check for dropped channel
                        if let Some(payload) = response.dnd_release_payload::<(usize, usize)>() {
                            if plot_area.has_capacity() {
                                let (dropped_file_idx, dropped_channel_idx) = *payload;
                                self.add_channel_to_plot(
                                    dropped_file_idx,
                                    dropped_channel_idx,
                                    plot_area.id,
                                );
                            }
                        }

                        // Highlight if hovering with drag payload
                        let is_drop_target =
                            response.dnd_hover_payload::<(usize, usize)>().is_some();
                        let stroke_color = if is_drop_target && plot_area.has_capacity() {
                            egui::Color32::from_rgb(71, 108, 155)
                        } else {
                            egui::Color32::from_gray(100)
                        };
                        let stroke_width = if is_drop_target { 2.0 } else { 1.0 };

                        ui.painter().rect_stroke(
                            rect,
                            egui::CornerRadius::same(4),
                            egui::Stroke::new(stroke_width, stroke_color),
                            egui::StrokeKind::Outside,
                        );
                        ui.put(
                            rect,
                            egui::Label::new(
                                egui::RichText::new(
                                    if is_drop_target && plot_area.has_capacity() {
                                        "Drop channel here"
                                    } else {
                                        "No channels in this plot"
                                    },
                                )
                                .italics()
                                .color(if is_drop_target {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::GRAY
                                }),
                            ),
                        );
                    } else {
                        // Render the plot with drop zone support
                        self.render_single_plot(ui, &plot_channels, plot_area.id, plot_height);
                    }

                    ui.add_space(5.0);

                    // Resize handle (except after last plot)
                    if plot_idx < plot_areas.len() - 1 {
                        let handle_response = self.render_resize_handle(ui);
                        if handle_response.dragged() {
                            let delta_pixels = handle_response.drag_delta().y;
                            resize_drag = Some((plot_idx, delta_pixels));
                        }
                    }
                }

                // Apply resize if drag occurred
                if let Some((plot_idx, delta_pixels)) = resize_drag {
                    self.adjust_plot_heights(plot_idx, delta_pixels);
                }
            });
    }

    /// Render a single plot within a plot area
    fn render_single_plot(
        &mut self,
        ui: &mut egui::Ui,
        channels: &[SelectedChannel],
        plot_area_id: usize,
        height: f32,
    ) {
        // Pre-compute and cache data for these channels
        for selected in channels {
            if selected.file_index >= self.files.len() {
                continue;
            }

            let cache_key = CacheKey {
                file_index: selected.file_index,
                channel_index: selected.channel_index,
                plot_area_id,
            };

            if !self.downsample_cache.contains_key(&cache_key) {
                let file = &self.files[selected.file_index];
                let times = file.log.get_times_as_f64();
                let data = self.get_channel_data(selected.file_index, selected.channel_index);

                if times.len() == data.len() && !times.is_empty() {
                    let downsampled = Self::downsample_lttb(times, &data, MAX_CHART_POINTS);
                    let normalized = Self::normalize_points(&downsampled);
                    self.downsample_cache.insert(cache_key, normalized);
                }
            }
        }

        // Build legend names with values
        let use_normalization = self.field_normalization;
        let custom_mappings = &self.custom_normalizations;
        let legend_names: Vec<String> = channels
            .iter()
            .map(|selected| {
                let original_name = selected.channel.name();
                let base_name = if use_normalization {
                    normalize_channel_name_with_custom(&original_name, Some(custom_mappings))
                } else {
                    original_name
                };
                if let Some(record) = self.get_cursor_record() {
                    if let Some(value) = self.get_value_at_record(
                        selected.file_index,
                        selected.channel_index,
                        record,
                    ) {
                        let source_unit = selected.channel.unit();
                        let (converted_value, display_unit) =
                            self.unit_preferences.convert_value(value, source_unit);
                        if display_unit.is_empty() {
                            format!("{}: {:.2}", base_name, converted_value)
                        } else {
                            format!("{}: {:.2} {}", base_name, converted_value, display_unit)
                        }
                    } else {
                        base_name
                    }
                } else {
                    base_name
                }
            })
            .collect();

        // Prepare data for plot
        let cache = &self.downsample_cache;
        let files = &self.files;
        let cursor_time = self.get_cursor_time();
        let cursor_tracking = self.cursor_tracking;
        let view_window = self.view_window_seconds;
        let time_range = self.get_time_range();
        let color_blind_mode = self.color_blind_mode;
        let chart_interacted = self.get_chart_interacted();
        let initial_view_seconds = self.initial_view_seconds;
        let jump_to_time = self.get_jump_to_time();

        // Fixed Y bounds
        const Y_MIN: f64 = -0.05;
        const Y_MAX: f64 = 1.05;

        // Build plot with fixed height
        let plot = Plot::new(format!("plot_{}", plot_area_id))
            .height(height)
            .legend(egui_plot::Legend::default())
            .y_axis_label("")
            .show_axes([true, false])
            .allow_zoom([true, false])
            .allow_drag([!cursor_tracking, false])
            .allow_scroll([!cursor_tracking, false]);

        let response = plot.show(ui, |plot_ui| {
            // Get current bounds
            let current_bounds = plot_ui.plot_bounds();
            let mut x_min = current_bounds.min()[0];
            let mut x_max = current_bounds.max()[0];

            // Handle jump-to-time request
            if let (Some(jump_time), Some((min_t, max_t))) = (jump_to_time, time_range) {
                let current_width = (x_max - x_min).max(view_window);
                let half_width = current_width / 2.0;
                x_min = (jump_time - half_width).max(min_t);
                x_max = (jump_time + half_width).min(max_t);
                if x_max - x_min < current_width {
                    if x_min == min_t {
                        x_max = (min_t + current_width).min(max_t);
                    } else {
                        x_min = (max_t - current_width).max(min_t);
                    }
                }
            } else if cursor_tracking {
                if let (Some(cursor), Some((min_t, max_t))) = (cursor_time, time_range) {
                    let half_window = view_window / 2.0;
                    x_min = (cursor - half_window).max(min_t);
                    x_max = (cursor + half_window).min(max_t);
                }
            } else if let Some((min_t, max_t)) = time_range {
                let data_width = max_t - min_t;

                if !chart_interacted && data_width > initial_view_seconds {
                    x_min = min_t;
                    x_max = min_t + initial_view_seconds;
                } else {
                    let current_width = x_max - x_min;

                    if current_width > data_width {
                        x_min = min_t;
                        x_max = max_t;
                    } else {
                        if x_min < min_t {
                            x_min = min_t;
                            x_max = min_t + current_width;
                        }
                        if x_max > max_t {
                            x_max = max_t;
                            x_min = max_t - current_width;
                        }
                    }
                }
            }

            // Set bounds
            let new_bounds = PlotBounds::from_min_max([x_min, Y_MIN], [x_max, Y_MAX]);
            plot_ui.set_plot_bounds(new_bounds);

            // Draw channel lines
            for (i, selected) in channels.iter().enumerate() {
                if selected.file_index >= files.len() {
                    continue;
                }

                let cache_key = CacheKey {
                    file_index: selected.file_index,
                    channel_index: selected.channel_index,
                    plot_area_id,
                };

                if let Some(points) = cache.get(&cache_key) {
                    let plot_points: PlotPoints = points.iter().copied().collect();
                    let palette = if color_blind_mode {
                        COLORBLIND_COLORS
                    } else {
                        CHART_COLORS
                    };
                    let color = palette[selected.color_index % palette.len()];
                    let name = &legend_names[i];

                    plot_ui.line(
                        Line::new(name.clone(), plot_points)
                            .color(egui::Color32::from_rgb(color[0], color[1], color[2]))
                            .width(1.5),
                    );
                }
            }

            // Draw cursor line
            if let Some(time) = cursor_time {
                plot_ui.vline(
                    VLine::new("Cursor", time)
                        .color(egui::Color32::from_rgb(0, 255, 255))
                        .width(2.0),
                );
            }

            plot_ui.pointer_coordinate()
        });

        // Detect interaction
        if response.response.dragged()
            || response.response.drag_started()
            || ui.input(|i| i.zoom_delta() != 1.0)
            || ui.input(|i| i.smooth_scroll_delta.x != 0.0)
        {
            self.set_chart_interacted(true);
        }

        // Clear jump-to-time
        if self.get_jump_to_time().is_some() {
            self.clear_jump_to_time();
            self.set_chart_interacted(true);
        }

        // Handle click
        if response.response.clicked() {
            if let Some(pos) = response.inner {
                let clicked_time = pos.x;
                if let Some((min, max)) = self.get_time_range() {
                    self.is_playing = false;
                    self.last_frame_time = None;
                    let clamped_time = clicked_time.clamp(min, max);
                    self.set_cursor_time(Some(clamped_time));
                    let record = self.find_record_at_time(clamped_time);
                    self.set_cursor_record(record);
                    ui.ctx().request_repaint();
                }
            }
        }

        // Handle dropped channel on the plot
        if let Some(payload) = response.response.dnd_release_payload::<(usize, usize)>() {
            let (dropped_file_idx, dropped_channel_idx) = *payload;
            self.add_channel_to_plot(dropped_file_idx, dropped_channel_idx, plot_area_id);
        }

        // Highlight plot when hovering with drag payload
        if response
            .response
            .dnd_hover_payload::<(usize, usize)>()
            .is_some()
        {
            ui.painter().rect_stroke(
                response.response.rect,
                egui::CornerRadius::same(4),
                egui::Stroke::new(3.0, egui::Color32::from_rgb(71, 108, 155)),
                egui::StrokeKind::Outside,
            );
        }
    }

    /// Render plot area header with title and controls
    fn render_plot_area_header(
        &mut self,
        ui: &mut egui::Ui,
        plot_area: &PlotArea,
        plot_idx: usize,
    ) {
        let font_14 = self.scaled_font(14.0);

        // Collapse/expand icon (custom drawn triangle)
        let (rect, response) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
        let center = rect.center();
        let color = if response.hovered() {
            egui::Color32::WHITE
        } else {
            egui::Color32::LIGHT_GRAY
        };

        if plot_area.collapsed {
            crate::ui::icons::draw_triangle_right(ui, center, 12.0, color);
        } else {
            crate::ui::icons::draw_triangle_down(ui, center, 12.0, color);
        }

        if response.clicked() {
            if let Some(tab_idx) = self.active_tab {
                self.tabs[tab_idx].plot_areas[plot_idx].collapsed = !plot_area.collapsed;
            }
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // Plot title
        ui.label(egui::RichText::new(&plot_area.name).strong().size(font_14));

        // Channel count
        ui.label(
            egui::RichText::new(format!(
                "({}/{})",
                plot_area.channel_count(),
                10 // MAX_CHANNELS_PER_PLOT
            ))
            .color(egui::Color32::GRAY)
            .size(font_14),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Delete button
            if ui.button("🗑").on_hover_text("Delete plot area").clicked() {
                self.delete_plot_area(plot_area.id);
            }
        });
    }

    /// Render collapsed plot area header
    fn render_plot_area_header_collapsed(
        &mut self,
        ui: &mut egui::Ui,
        plot_area: &PlotArea,
        plot_idx: usize,
    ) {
        ui.horizontal(|ui| {
            // Expand icon (custom drawn triangle)
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
            let center = rect.center();
            let color = if response.hovered() {
                egui::Color32::WHITE
            } else {
                egui::Color32::GRAY
            };

            crate::ui::icons::draw_triangle_right(ui, center, 12.0, color);

            if response.clicked() {
                if let Some(tab_idx) = self.active_tab {
                    self.tabs[tab_idx].plot_areas[plot_idx].collapsed = false;
                }
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.label(egui::RichText::new(&plot_area.name).color(egui::Color32::GRAY));
        });
    }

    /// Render resize handle between plots
    fn render_resize_handle(&self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), PLOT_RESIZE_HANDLE_HEIGHT),
            egui::Sense::drag(),
        );

        // Visual indicator
        let color = if response.hovered() || response.dragged() {
            egui::Color32::from_rgb(100, 150, 255)
        } else {
            egui::Color32::from_rgb(60, 60, 60)
        };

        ui.painter().rect_filled(rect, 2.0, color);

        // Change cursor on hover
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }

        response
    }

    /// Format time in seconds to a human-readable string (h:mm:ss.xxx or m:ss.xxx or s.xxx)
    pub fn format_time(seconds: f64) -> String {
        let total_seconds = seconds.abs();
        let hours = (total_seconds / 3600.0).floor() as u32;
        let minutes = ((total_seconds % 3600.0) / 60.0).floor() as u32;
        let secs = total_seconds % 60.0;

        let sign = if seconds < 0.0 { "-" } else { "" };

        if hours > 0 {
            // h:mm:ss.xxx format
            format!("{}{}:{:02}:{:06.3}", sign, hours, minutes, secs)
        } else if minutes > 0 {
            // m:ss.xxx format
            format!("{}{}:{:06.3}", sign, minutes, secs)
        } else {
            // s.xxxs format
            format!("{}{:.3}s", sign, secs)
        }
    }

    /// Normalize values to 0-1 range for overlay display
    pub fn normalize_points(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
        if points.is_empty() {
            return Vec::new();
        }

        // Find min and max Y values
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for point in points {
            min_y = min_y.min(point[1]);
            max_y = max_y.max(point[1]);
        }

        // Handle case where all values are the same
        let range = max_y - min_y;
        if range.abs() < f64::EPSILON {
            // All values are the same, put at 0.5
            return points.iter().map(|p| [p[0], 0.5]).collect();
        }

        // Normalize to 0-1 range
        points
            .iter()
            .map(|p| [p[0], (p[1] - min_y) / range])
            .collect()
    }

    /// Downsample data using the LTTB (Largest Triangle Three Buckets) algorithm.
    /// This preserves visual characteristics while reducing point count for performance.
    pub fn downsample_lttb(times: &[f64], values: &[f64], target_points: usize) -> Vec<[f64; 2]> {
        let n = times.len();

        if n <= target_points || target_points < 3 {
            // No downsampling needed
            return times
                .iter()
                .zip(values.iter())
                .map(|(t, v)| [*t, *v])
                .collect();
        }

        let mut result = Vec::with_capacity(target_points);

        // Always include first point
        result.push([times[0], values[0]]);

        // Bucket size
        let bucket_size = (n - 2) as f64 / (target_points - 2) as f64;

        let mut a_index = 0usize;

        for i in 0..(target_points - 2) {
            // Calculate bucket range
            let bucket_start = ((i as f64 + 1.0) * bucket_size).floor() as usize + 1;
            let bucket_end = (((i + 2) as f64) * bucket_size).floor() as usize + 1;
            let bucket_end = bucket_end.min(n - 1);

            // Calculate average point for next bucket (for triangle calculation)
            let next_bucket_start = bucket_end;
            let next_bucket_end = (((i + 3) as f64) * bucket_size).floor() as usize + 1;
            let next_bucket_end = next_bucket_end.min(n);

            let (avg_x, avg_y) = if next_bucket_start < next_bucket_end {
                let count = (next_bucket_end - next_bucket_start) as f64;
                let sum_x: f64 = times[next_bucket_start..next_bucket_end].iter().sum();
                let sum_y: f64 = values[next_bucket_start..next_bucket_end].iter().sum();
                (sum_x / count, sum_y / count)
            } else {
                (times[n - 1], values[n - 1])
            };

            // Find point in current bucket with largest triangle area
            let mut max_area = -1.0f64;
            let mut max_index = bucket_start;

            let a_x = times[a_index];
            let a_y = values[a_index];

            for j in bucket_start..bucket_end {
                // Calculate triangle area
                let area =
                    ((a_x - avg_x) * (values[j] - a_y) - (a_x - times[j]) * (avg_y - a_y)).abs();

                if area > max_area {
                    max_area = area;
                    max_index = j;
                }
            }

            result.push([times[max_index], values[max_index]]);
            a_index = max_index;
        }

        // Always include last point
        result.push([times[n - 1], values[n - 1]]);

        result
    }
}
