//! Chart rendering and data processing utilities.

use eframe::egui;
use egui_plot::{Plot, PlotBounds};
use rust_i18n::t;

use crate::app::UltraLogApp;
use crate::normalize::normalize_channel_name_with_custom;
use crate::state::{
    DownsampleViewKey, MAX_CHART_POINTS, MIN_PLOT_HEIGHT, PLOT_RESIZE_HANDLE_HEIGHT, PlotArea,
    SelectedChannel,
};
use crate::ui::components::{self, icon};
use crate::ui::fonts::{self, style};
use crate::ui::plot_style::{self, PlotStyle, SeriesRow};

/// Sensitivity multiplier for scroll-to-zoom (higher = faster zoom per scroll tick).
/// Shared between the cursor-tracking pre-show helper and the post-show closure.
const SCROLL_ZOOM_SENSITIVITY: f64 = 0.003;

impl UltraLogApp {
    /// Render the main chart with cached downsampled data
    pub fn render_chart(&mut self, ui: &mut egui::Ui) {
        // "Align at cursor" handles its own Esc: the global shortcuts skip a frame where a
        // widget (the Align button) still has focus, and their Esc stops playback.
        if self.picking_alignment()
            && ui.input(|i| i.key_pressed(egui::Key::Escape))
            && let Some(tab_idx) = self.active_tab
        {
            self.cancel_alignment(tab_idx);
        }

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

    /// Translate pinch / cmd+wheel (and scroll if `scroll_to_zoom` is on) into
    /// changes of `current_view_window` while in cursor-tracking mode. Without
    /// this, the cursor-tracking branch in the plot closure forces bounds to a
    /// fixed-width window every frame and any zoom is immediately overridden.
    /// The persistent `view_window_seconds` (Settings slider) is intentionally
    /// not touched here — that's the user-set default; the slider write-path
    /// resets `current_view_window` back to it on every change.
    fn apply_zoom_to_view_window_if_tracking(&mut self, ui: &egui::Ui) {
        if !self.cursor_tracking {
            return;
        }
        // Don't react to scroll/pinch happening over other UI (Settings
        // panel, right-side data panel with the Track Map). `ui.input` is
        // global, so without this guard a wheel event over the data panel
        // would still resize the cursor-tracking window even though the
        // pointer is nowhere near the chart. `max_rect()` was used here
        // historically but it spans the entire central area *including*
        // any side panels carved out before us; `available_rect_before_wrap`
        // is exactly the chart's slot at this point in the layout pass.
        if !ui.rect_contains_pointer(ui.available_rect_before_wrap()) {
            return;
        }
        let Some((min_t, max_t)) = self.get_time_range() else {
            return;
        };
        let zoom_delta = ui.input(|i| i.zoom_delta()) as f64;
        let mut new_window = self.current_view_window;
        if zoom_delta != 1.0 {
            new_window /= zoom_delta;
        }
        if self.scroll_to_zoom {
            let scroll_y = ui.input(|i| i.smooth_scroll_delta.y) as f64;
            if scroll_y.abs() > 0.1 {
                let factor = (1.0 - scroll_y * SCROLL_ZOOM_SENSITIVITY).clamp(0.8, 1.25);
                new_window *= factor;
            }
        }
        let max_window = (max_t - min_t).max(0.1);
        self.current_view_window = new_window.clamp(0.1, max_window);
    }

    /// Render single-plot mode chart (original implementation)
    fn render_chart_single_mode(&mut self, ui: &mut egui::Ui) {
        self.apply_zoom_to_view_window_if_tracking(ui);

        // Get selected channels from active tab
        let selected_channels = self.get_selected_channels().to_vec();

        if selected_channels.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(t!("chart.select_channels"))
                        .heading()
                        .color(self.tokens.ink_subtle),
                );
            });
            return;
        }

        // Compute downsampled + normalized data sliced to the current viewport.
        // Detail scales with zoom level: a 1% viewport gets MAX_CHART_POINTS
        // over that 1%, not over the whole log.
        let viewport = self.chart_last_x_bounds.get(&0).copied();
        let chart_points: Vec<Option<Vec<[f64; 2]>>> = selected_channels
            .iter()
            .map(|selected| self.channel_plot_points(selected, 0, viewport))
            .collect();

        let rows = self.series_rows(&selected_channels);

        // Prepare data for the plot closure (can't borrow self mutably inside)
        let chart_points = &chart_points;
        let cursor_time = self.get_cursor_time();
        let cursor_tracking = self.cursor_tracking;
        let view_window = self.current_view_window;
        let time_range = self.get_time_range();
        let chart_interacted = self.get_chart_interacted();
        let initial_view_seconds = self.initial_view_seconds;
        let jump_to_time = self.get_jump_to_time();
        let scroll_to_zoom = self.scroll_to_zoom;
        let show_grid = self.show_grid;
        let grid_color = grid_color_with_opacity(self.tokens.plot_grid_major, self.grid_opacity);
        let plot_id = egui::Id::new("log_chart");
        let tokens = self.tokens;
        let style = PlotStyle {
            tokens: &tokens,
            cvd_like: self.color_blind_mode || self.high_contrast,
            solo: plot_style::solo_memory(ui.ctx(), plot_id, None),
        };

        // Read scroll input before plot consumes it (for scroll-to-zoom mode)
        let scroll_delta_y = if scroll_to_zoom && !cursor_tracking {
            ui.input(|i| i.smooth_scroll_delta.y)
        } else {
            0.0
        };

        // Fixed Y bounds for normalized data (0-1 with small padding)
        const Y_MIN: f64 = -0.05;
        const Y_MAX: f64 = 1.05;

        // Build the plot - X-axis zoom only, Y fixed
        // When scroll_to_zoom is enabled, disable scroll-to-pan so we handle scroll as zoom
        let plot = Plot::new("log_chart")
            .y_axis_label("") // Hide Y axis label since values are normalized
            .show_axes([true, false]) // Show X axis (time), hide Y axis (normalized 0-1)
            // The hover readout draws its own time line; egui_plot's crosshair adds a y line,
            // which means nothing on the normalized axis.
            .show_crosshair(!self.hover_readout)
            .show_grid([show_grid, show_grid])
            .grid_color(grid_color)
            .allow_zoom([true, false]) // Only allow X-axis zoom
            .allow_drag([!cursor_tracking, false]) // Only allow X-axis drag, never Y
            .allow_scroll([!cursor_tracking && !scroll_to_zoom, false]); // Disable scroll-pan when scroll-to-zoom enabled

        let response = with_plot_bg(ui, tokens.plot_bg, |ui| {
            plot.show(ui, |plot_ui| {
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

                // Apply scroll-to-zoom: use scroll delta to zoom centered on pointer
                if scroll_to_zoom
                    && scroll_delta_y.abs() > 0.1
                    && let Some((min_t, max_t)) = time_range
                {
                    let zoom_factor =
                        (1.0 - scroll_delta_y as f64 * SCROLL_ZOOM_SENSITIVITY).clamp(0.8, 1.25);
                    let width = x_max - x_min;
                    let new_width = (width * zoom_factor).clamp(0.01, max_t - min_t);

                    // Zoom around pointer position if hovering, otherwise center
                    let center = plot_ui
                        .pointer_coordinate()
                        .map(|p| p.x.clamp(x_min, x_max))
                        .unwrap_or((x_min + x_max) / 2.0);
                    let ratio = if width > 0.0 {
                        (center - x_min) / width
                    } else {
                        0.5
                    };

                    x_min = center - new_width * ratio;
                    x_max = center + new_width * (1.0 - ratio);

                    // Clamp to data range
                    if x_min < min_t {
                        x_min = min_t;
                        x_max = (min_t + new_width).min(max_t);
                    }
                    if x_max > max_t {
                        x_max = max_t;
                        x_min = (max_t - new_width).max(min_t);
                    }
                }

                // Always enforce bounds: X clamped to data, Y fixed to normalized range
                let new_bounds = PlotBounds::from_min_max([x_min, Y_MIN], [x_max, Y_MAX]);
                plot_ui.set_plot_bounds(new_bounds);

                plot_style::draw_series(plot_ui, &style, &rows, chart_points, cursor_time);

                // Return pointer position if hovering for click detection
                plot_ui.pointer_coordinate()
            })
        });

        // Remember the X-axis bounds we just rendered so the next frame can
        // slice raw data to this viewport before LTTB-downsampling.
        let final_bounds = response.transform.bounds();
        self.chart_last_x_bounds
            .insert(0, (final_bounds.min()[0], final_bounds.max()[0]));
        self.paint_plot_overlays(
            ui,
            &response,
            &style,
            &rows,
            chart_points,
            cursor_time,
            plot_id,
            &selected_channels,
        );

        // Detect user interaction with chart (drag, zoom, scroll)
        // This marks the chart as "interacted" so we stop using the initial zoomed view
        if response.response.dragged()
            || response.response.drag_started()
            || ui.input(|i| i.zoom_delta() != 1.0)
            || ui.input(|i| i.smooth_scroll_delta.x != 0.0)
            || (scroll_to_zoom && scroll_delta_y.abs() > 0.1)
        {
            self.set_chart_interacted(true);
        }

        // Clear jump-to-time request after it's been processed
        if self.get_jump_to_time().is_some() {
            self.clear_jump_to_time();
            // Mark chart as interacted so future jumps work correctly
            self.set_chart_interacted(true);
        }

        self.handle_chart_click(ui, &response.response, response.inner);
    }

    /// A click on a plot (single or stacked mode): finishes "Align at cursor" while it waits,
    /// otherwise sets the cursor. Shows a crosshair over the plot while aligning.
    fn handle_chart_click(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        pointer: Option<egui_plot::PlotPoint>,
    ) {
        let picking = self.picking_alignment();
        if picking && response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
        if !response.clicked() {
            return;
        }
        let Some(pos) = pointer else {
            return;
        };
        let clicked_time = pos.x;
        if picking {
            if let Some(tab_idx) = self.active_tab {
                self.finish_alignment(tab_idx, clicked_time);
            }
            ui.ctx().request_repaint();
            return;
        }
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

    /// Render stacked plot areas
    fn render_chart_stacked_mode(&mut self, ui: &mut egui::Ui) {
        self.apply_zoom_to_view_window_if_tracking(ui);

        let Some(tab_idx) = self.active_tab else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("No active tab")
                        .heading()
                        .color(self.tokens.ink_subtle),
                );
            });
            return;
        };

        let plot_areas = self.tabs[tab_idx].plot_areas.clone();
        let num_selected = self.tabs[tab_idx].selected_channels.len();

        if num_selected == 0 {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(t!("chart.select_channels"))
                        .heading()
                        .color(self.tokens.ink_subtle),
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
                        .filter_map(|&idx| self.tabs[tab_idx].selected_channels.get(idx).cloned())
                        .collect();

                    let plot_has_capacity = self.tabs[tab_idx].plot_has_capacity(plot_area.id);

                    if plot_channels.is_empty() {
                        // Empty plot area with drop zone
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), plot_height),
                            egui::Sense::hover(),
                        );

                        // Check for dropped channel
                        if let Some(payload) = response.dnd_release_payload::<(usize, usize)>()
                            && plot_has_capacity
                        {
                            let (dropped_file_idx, dropped_channel_idx) = *payload;
                            self.add_channel_to_plot(
                                dropped_file_idx,
                                dropped_channel_idx,
                                plot_area.id,
                            );
                        }

                        // Highlight if hovering with drag payload
                        let is_drop_target =
                            response.dnd_hover_payload::<(usize, usize)>().is_some();
                        let stroke_color = if is_drop_target && plot_has_capacity {
                            self.tokens.cyan
                        } else {
                            self.tokens.line_control
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
                                egui::RichText::new(if is_drop_target && plot_has_capacity {
                                    "Drop channel here"
                                } else {
                                    "No channels in this plot"
                                })
                                .italics()
                                .color(if is_drop_target {
                                    self.tokens.ink
                                } else {
                                    self.tokens.ink_subtle
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
        // Compute viewport-aware downsampled + normalized points for this plot area.
        let viewport = self.chart_last_x_bounds.get(&plot_area_id).copied();
        let chart_points: Vec<Option<Vec<[f64; 2]>>> = channels
            .iter()
            .map(|selected| self.channel_plot_points(selected, plot_area_id, viewport))
            .collect();

        let rows = self.series_rows(channels);

        // Prepare data for plot
        let chart_points = &chart_points;
        let cursor_time = self.get_cursor_time();
        let cursor_tracking = self.cursor_tracking;
        let view_window = self.current_view_window;
        let time_range = self.get_time_range();
        let chart_interacted = self.get_chart_interacted();
        let initial_view_seconds = self.initial_view_seconds;
        let jump_to_time = self.get_jump_to_time();

        // Fixed Y bounds
        const Y_MIN: f64 = -0.05;
        const Y_MAX: f64 = 1.05;

        let show_grid = self.show_grid;
        let grid_color = grid_color_with_opacity(self.tokens.plot_grid_major, self.grid_opacity);
        let plot_id = egui::Id::new(("plot_area", plot_area_id));
        let tokens = self.tokens;
        let style = PlotStyle {
            tokens: &tokens,
            cvd_like: self.color_blind_mode || self.high_contrast,
            solo: plot_style::solo_memory(ui.ctx(), plot_id, None),
        };

        // Build plot with fixed height
        let plot = Plot::new(format!("plot_{}", plot_area_id))
            .height(height)
            .y_axis_label("")
            .show_axes([true, false])
            // The hover readout draws its own time line; egui_plot's crosshair adds a y line,
            // which means nothing on the normalized axis.
            .show_crosshair(!self.hover_readout)
            .show_grid([show_grid, show_grid])
            .grid_color(grid_color)
            .allow_zoom([true, false])
            .allow_drag([!cursor_tracking, false])
            .allow_scroll([!cursor_tracking, false]);

        let response = with_plot_bg(ui, tokens.plot_bg, |ui| {
            plot.show(ui, |plot_ui| {
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

                plot_style::draw_series(plot_ui, &style, &rows, chart_points, cursor_time);

                plot_ui.pointer_coordinate()
            })
        });

        // Save the bounds we just rendered so the next frame's downsample
        // matches the visible viewport.
        let final_bounds = response.transform.bounds();
        self.chart_last_x_bounds
            .insert(plot_area_id, (final_bounds.min()[0], final_bounds.max()[0]));
        self.paint_plot_overlays(
            ui,
            &response,
            &style,
            &rows,
            chart_points,
            cursor_time,
            plot_id,
            channels,
        );

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

        self.handle_chart_click(ui, &response.response, response.inner);

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
                egui::Stroke::new(2.0, self.tokens.cyan),
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
        // Collapse/expand icon (custom drawn triangle)
        let (rect, response) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
        let center = rect.center();
        let color = if response.hovered() {
            self.tokens.ink
        } else {
            self.tokens.ink_muted
        };

        if plot_area.collapsed {
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                icon::CARET_RIGHT,
                egui::FontId::proportional(14.0),
                color,
            );
        } else {
            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                icon::CARET_DOWN,
                egui::FontId::proportional(14.0),
                color,
            );
        }

        if response.clicked()
            && let Some(tab_idx) = self.active_tab
        {
            self.tabs[tab_idx].plot_areas[plot_idx].collapsed = !plot_area.collapsed;
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // Plot title
        ui.label(
            egui::RichText::new(&plot_area.name)
                .text_style(egui::TextStyle::Heading)
                .color(self.tokens.ink),
        );

        // Channel count: a compare pair counts as one (R2)
        let units = self
            .active_tab
            .map_or(plot_area.channel_indices.len(), |idx| {
                self.tabs[idx].channel_units(&plot_area.channel_indices)
            });
        ui.label(
            egui::RichText::new(format!("({units}/{})", crate::state::MAX_CHANNELS_PER_PLOT))
                .text_style(fonts::text_style(style::DATA_SM))
                .color(self.tokens.ink_subtle),
        );

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Delete button
            if components::icon_button(ui, icon::TRASH, "Delete plot area").clicked() {
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
                self.tokens.ink
            } else {
                self.tokens.ink_muted
            };

            ui.painter().text(
                center,
                egui::Align2::CENTER_CENTER,
                icon::CARET_RIGHT,
                egui::FontId::proportional(14.0),
                color,
            );

            if response.clicked()
                && let Some(tab_idx) = self.active_tab
            {
                self.tabs[tab_idx].plot_areas[plot_idx].collapsed = false;
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.label(egui::RichText::new(&plot_area.name).color(self.tokens.ink_muted));
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
            self.tokens.cyan
        } else {
            self.tokens.line
        };

        ui.painter().rect_filled(rect, 2.0, color);

        // Change cursor on hover
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
        }

        response
    }

    /// Legend / line rows for `channels`: normalized name, value at the cursor (converted to the
    /// user's units, mono column), and the series colour and stroke.
    fn series_rows(&self, channels: &[SelectedChannel]) -> Vec<SeriesRow> {
        self.series_rows_at(channels, self.get_cursor_time())
    }

    /// Legend rows with each channel's value at tab time `time` (the cursor for the legend, the
    /// pointer for the hover readout).
    fn series_rows_at(&self, channels: &[SelectedChannel], time: Option<f64>) -> Vec<SeriesRow> {
        let cursor_time = time;
        channels
            .iter()
            .map(|selected| {
                let original_name = selected.channel.name();
                let name = if self.field_normalization {
                    normalize_channel_name_with_custom(
                        &original_name,
                        Some(&self.custom_normalizations),
                    )
                } else {
                    original_name
                };
                // Each log's own record at the cursor, and no value outside its span (R4).
                let value = cursor_time
                    .and_then(|t| self.record_at(selected.file_index, t))
                    .and_then(|r| {
                        self.get_value_at_record(selected.file_index, selected.channel_index, r)
                    })
                    .map(|value| {
                        let (converted, unit) = self
                            .unit_preferences
                            .convert_value(value, selected.channel.unit());
                        if unit.is_empty() {
                            format!("{converted:.2}")
                        } else {
                            format!("{converted:.2} {unit}")
                        }
                    });
                let (color, stroke) = self.channel_style(selected);
                SeriesRow {
                    name,
                    tag: self.log_tag(selected),
                    value,
                    color,
                    stroke,
                }
            })
            .collect()
    }

    /// Legend overlay, direct end labels (color-blind safe palettes) and the solo state for the
    /// next frame.
    #[allow(clippy::too_many_arguments)]
    fn paint_plot_overlays(
        &self,
        ui: &egui::Ui,
        response: &egui_plot::PlotResponse<Option<egui_plot::PlotPoint>>,
        style: &PlotStyle<'_>,
        rows: &[SeriesRow],
        points: &[Option<Vec<[f64; 2]>>],
        cursor_time: Option<f64>,
        plot_id: egui::Id,
        channels: &[SelectedChannel],
    ) {
        let plot_rect = *response.transform.frame();
        if style.cvd_like {
            plot_style::direct_labels(ui, &response.transform, rows, points);
        }
        let hovered = plot_style::legend_overlay(
            ui,
            plot_rect,
            style,
            rows,
            cursor_time.map(Self::format_time),
            plot_id,
        );
        // The hover readout (issue #98): values at the pointer, beside the cursor legend. Off
        // while panning, picking an alignment point or over the legend itself.
        if self.hover_readout
            && hovered.is_none()
            && !self.picking_alignment()
            && !response.response.dragged()
            && let Some(point) = response.inner
            && let Some(pointer) = ui.ctx().pointer_hover_pos()
            && response.response.hovered()
        {
            let hover_rows = self.series_rows_at(channels, Some(point.x));
            plot_style::hover_readout(
                ui,
                plot_rect,
                style,
                &hover_rows,
                &Self::format_time(point.x),
                pointer,
            );
        }
        if hovered != style.solo {
            ui.ctx().request_repaint();
        }
        plot_style::solo_memory(ui.ctx(), plot_id, Some(hovered));
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

    /// Compute the points to plot for one channel, sliced to the currently
    /// visible viewport before LTTB-downsampling. Y is normalized to [0, 1]
    /// against the channel's full-range min/max so heights stay stable when
    /// the user pans or zooms. `viewport` is the previous frame's X bounds;
    /// when `None` (e.g., first frame after load) the full data range is used.
    /// Chart points for one selected channel on the tab's time axis: log B's viewport is shifted
    /// back into its own time, and its points forward by the offset; a compare pair shares one
    /// y range (R1).
    fn channel_plot_points(
        &mut self,
        selected: &SelectedChannel,
        plot_area_id: usize,
        viewport: Option<(f64, f64)>,
    ) -> Option<Vec<[f64; 2]>> {
        let offset = self
            .active_tab
            .map_or(0.0, |idx| self.tabs[idx].time_offset(selected.file_index));
        let y_range = self.pair_y_range(selected).unwrap_or((0.0, 1.0));
        self.compute_viewport_points(
            selected.file_index,
            selected.channel_index,
            plot_area_id,
            viewport.map(|(lo, hi)| (lo - offset, hi - offset)),
            offset,
            y_range,
        )
    }

    /// Downsampled points of one channel over `viewport` (in the file's own time), scaled to
    /// 0..1 by `y_range` and moved by `offset` seconds onto the tab's time axis. The cache holds
    /// the raw file-time points: offset and scale are applied to the copy returned, so a nudge
    /// smaller than one bucket, or a new pair range, takes effect at once.
    fn compute_viewport_points(
        &mut self,
        file_index: usize,
        channel_index: usize,
        plot_area_id: usize,
        viewport: Option<(f64, f64)>,
        offset: f64,
        (min_y, max_y): (f64, f64),
    ) -> Option<Vec<[f64; 2]>> {
        let file = self.files.get(file_index)?;
        let times = file.log.get_times_as_f64();
        let data = self.get_channel_data_ref(file_index, channel_index);
        if times.is_empty() || times.len() != data.len() {
            return None;
        }

        // Derive the cache key for this viewport. The bucketed downsampler
        // below is anchored to a fixed grid, so its output only changes when
        // the first bucket index or the bucket width changes — steady frames
        // (idle, cursor moves within a bucket) reuse the cached points.
        let n_buckets = (MAX_CHART_POINTS / 2).max(1);
        let view_key = match viewport {
            Some((vmin, vmax)) if vmax > vmin => {
                let pad = (vmax - vmin) * 0.1;
                let padded_span = (vmax - vmin) + 2.0 * pad;
                let bucket_size = padded_span / n_buckets as f64;
                if bucket_size <= 0.0 {
                    DownsampleViewKey::Full
                } else {
                    let raw_lo = vmin - pad;
                    DownsampleViewKey::Bucketed {
                        k_lo: (raw_lo / bucket_size).floor() as i64,
                        bucket_bits: bucket_size.to_bits(),
                    }
                }
            }
            _ => DownsampleViewKey::Full,
        };

        let cache_key = (file_index, channel_index, plot_area_id);
        let place = |points: &[[f64; 2]]| -> Vec<[f64; 2]> {
            let range = (max_y - min_y).abs();
            // Constant channels (range ≈ 0) get parked at the middle of the
            // overlay strip so they remain visible instead of pinning to the
            // bottom edge — matches the prior `normalize_points` behavior.
            points
                .iter()
                .map(|p| {
                    let y = if range < f64::EPSILON {
                        0.5
                    } else {
                        (p[1] - min_y) / range
                    };
                    [p[0] + offset, y]
                })
                .collect()
        };
        if let Some((cached_key, points)) = self.downsample_cache.get(&cache_key)
            && *cached_key == view_key
        {
            return Some(place(points));
        }

        let downsampled = match view_key {
            DownsampleViewKey::Bucketed { k_lo, bucket_bits } => {
                // Anchored min/max-per-bucket downsampling. Bucket
                // boundaries are at multiples of `bucket_size` from t=0,
                // so during cursor-tracked playback samples slide through
                // a fixed grid instead of being re-bucketed every frame.
                // Without this anchoring, LTTB-by-index re-selects a
                // different "best peak" per frame and the curve jitters
                // at far zoom-out.
                let bucket_size = f64::from_bits(bucket_bits);
                let mut points: Vec<[f64; 2]> = Vec::with_capacity(MAX_CHART_POINTS);
                let mut idx = times.partition_point(|&t| t < k_lo as f64 * bucket_size);
                for k in 0..n_buckets as i64 {
                    let bucket_end = (k_lo + k + 1) as f64 * bucket_size;
                    let mut end_idx = idx;
                    while end_idx < times.len() && times[end_idx] < bucket_end {
                        end_idx += 1;
                    }
                    if end_idx > idx {
                        let mut min_i = idx;
                        let mut max_i = idx;
                        for i in idx..end_idx {
                            if data[i] < data[min_i] {
                                min_i = i;
                            }
                            if data[i] > data[max_i] {
                                max_i = i;
                            }
                        }
                        if min_i == max_i {
                            points.push([times[min_i], data[min_i]]);
                        } else if min_i < max_i {
                            points.push([times[min_i], data[min_i]]);
                            points.push([times[max_i], data[max_i]]);
                        } else {
                            points.push([times[max_i], data[max_i]]);
                            points.push([times[min_i], data[min_i]]);
                        }
                    }
                    idx = end_idx;
                }
                points
            }
            DownsampleViewKey::Full => Self::downsample_lttb(times, data, MAX_CHART_POINTS),
        };

        let points = place(&downsampled);
        self.downsample_cache
            .insert(cache_key, (view_key, downsampled));
        Some(points)
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

/// The grid colour scaled by the user's "Show grid" opacity. egui_plot fades finer grid levels
/// on top, so `plot_grid_major` as the base gives major and minor lines.
fn grid_color_with_opacity(base: egui::Color32, alpha: u8) -> egui::Color32 {
    base.gamma_multiply(f32::from(alpha) / 255.0)
}

/// Run `add` with the plot canvas set to `plot_bg` (egui_plot paints its background with
/// `extreme_bg_color`, which the theme sets to `bg_field` for text inputs).
fn with_plot_bg<R>(
    ui: &mut egui::Ui,
    plot_bg: egui::Color32,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.scope(|ui| {
        ui.visuals_mut().extreme_bg_color = plot_bg;
        add(ui)
    })
    .inner
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::app::UltraLogApp;
    use crate::parsers::aim::AimChannel;
    use crate::parsers::types::Meta;
    use crate::parsers::{Channel, EcuType, Log, Value};
    use crate::state::{LoadedFile, Tab};

    /// One "Boost" channel at 10 Hz for `secs` seconds, value `f(t)`.
    fn file(name: &str, secs: usize, f: impl Fn(f64) -> f64) -> LoadedFile {
        let times: Vec<f64> = (0..=secs * 10).map(|i| i as f64 * 0.1).collect();
        let log = Log {
            meta: Meta::Empty,
            channels: vec![Channel::Aim(AimChannel {
                name: "Boost".to_string(),
                unit: String::new(),
            })],
            data: times.iter().map(|&t| vec![Value::Float(f(t))]).collect(),
            times,
        };
        LoadedFile::new(PathBuf::from(name), name.to_string(), EcuType::Aim, log)
    }

    /// Tab 0: A = pull peaking at 1.5, B = pull peaking at 1.8, paired on colour slot 0.
    fn comparing_app() -> UltraLogApp {
        let mut app = UltraLogApp::default();
        app.files.push(file("a.csv", 20, |t| 1.5 * (t / 20.0)));
        app.files.push(file("b.csv", 20, |t| 1.8 * (t / 20.0)));
        app.tabs.push(Tab::new(0, "a.csv".to_string()));
        app.active_tab = Some(0);
        app.update_tab_time_range(0);
        app.set_compare_file(0, Some(1));
        app.add_channel(0, 0);
        app.add_channel(1, 0);
        let b = &mut app.tabs[0].selected_channels[1];
        b.color_index = 0; // pair it with A (step 3 does this when pairing)
        app
    }

    #[test]
    fn a_pair_shares_one_y_range() {
        let mut app = comparing_app();
        let [a, b] = [0, 1].map(|i| app.tabs[0].selected_channels[i].clone());
        assert_eq!(app.pair_y_range(&a), Some((0.0, 1.8)));
        assert_eq!(app.pair_y_range(&b), Some((0.0, 1.8)));

        // A's peak sits below B's on the shared scale.
        let pa = app.channel_plot_points(&a, 0, None).unwrap();
        let pb = app.channel_plot_points(&b, 0, None).unwrap();
        let top = |p: &[[f64; 2]]| p.iter().map(|q| q[1]).fold(f64::MIN, f64::max);
        assert!((top(&pa) - 1.5 / 1.8).abs() < 1e-9);
        assert!((top(&pb) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_nudge_smaller_than_a_bucket_moves_log_b() {
        let mut app = comparing_app();
        let b = app.tabs[0].selected_channels[1].clone();
        let viewport = Some((0.0, 20.0));
        let before = app.channel_plot_points(&b, 0, viewport).unwrap();

        // 0.01 s is far below the bucket width over a 20 s view, so the cache key may not
        // change; the points must move anyway.
        app.set_compare_offset(0, 0.01);
        let after = app.channel_plot_points(&b, 0, viewport).unwrap();
        assert_eq!(before.len(), after.len());
        for (p, q) in before.iter().zip(&after) {
            assert!((q[0] - p[0] - 0.01).abs() < 1e-9);
            assert_eq!(p[1], q[1]);
        }
    }

    #[test]
    fn hover_rows_read_the_pointer_time_not_the_cursor() {
        let mut app = comparing_app();
        app.set_compare_offset(0, -5.0);
        app.tabs[0].cursor_time = Some(10.0);
        let channels = app.tabs[0].selected_channels.clone();
        // The cursor legend stays at 10 s; the pointer at 4 s reads A at 4 s and B at 9 s.
        let cursor = app.series_rows(&channels);
        let hover = app.series_rows_at(&channels, Some(4.0));
        assert_eq!(cursor[0].value.as_deref(), Some("0.75"));
        assert_eq!(hover[0].value.as_deref(), Some("0.30"));
        assert_eq!(hover[1].value.as_deref(), Some("0.81"));
        assert_eq!(hover[1].tag, Some("B"));
        // Past log B's end (file time 25 s > 20 s): a dash, not a stale value.
        assert_eq!(app.series_rows_at(&channels, Some(19.5))[1].value, None);
    }

    #[test]
    fn legend_values_come_from_each_logs_own_record() {
        let mut app = comparing_app();
        app.set_compare_offset(0, -5.0);
        app.tabs[0].cursor_time = Some(10.0);
        let channels = app.tabs[0].selected_channels.clone();
        let rows = app.series_rows(&channels);
        assert_eq!(rows[0].tag, Some("A"));
        assert_eq!(rows[1].tag, Some("B"));
        // A at t = 10: 0.75. B at file time 15: 1.35.
        assert_eq!(rows[0].value.as_deref(), Some("0.75"));
        assert_eq!(rows[1].value.as_deref(), Some("1.35"));

        // Past B's end (file time 20 + 5 = 25 > 20): no B value.
        app.tabs[0].cursor_time = Some(19.0);
        app.set_compare_offset(0, -5.0);
        let rows = app.series_rows(&channels);
        assert!(rows[0].value.is_some());
        assert_eq!(rows[1].value, None);
    }
}
