//! Scatter plot / heatmap view for comparing two variables.
//!
//! This module provides a dual heatmap view where users can visualize
//! relationships between channels as a 2D histogram with hit count coloring.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::normalize::{normalize_channel_name_with_custom, sort_channels_by_priority};
use crate::state::{ScatterPlotConfig, SelectedHeatmapPoint};

/// Heat map color gradient from blue (low) to red (high)
const HEAT_COLORS: &[[u8; 3]] = &[
    [0, 0, 80],    // Dark blue (0.0)
    [0, 0, 180],   // Blue
    [0, 100, 255], // Light blue
    [0, 200, 255], // Cyan
    [0, 255, 200], // Cyan-green
    [0, 255, 100], // Green
    [100, 255, 0], // Yellow-green
    [200, 255, 0], // Yellow
    [255, 200, 0], // Orange
    [255, 100, 0], // Red-orange
    [255, 0, 0],   // Red (1.0)
];

/// Number of bins in each dimension for the heatmap grid (higher = more detail)
const HEATMAP_BINS: usize = 512;

/// Margin for axis labels
const AXIS_LABEL_MARGIN_LEFT: f32 = 50.0;
const AXIS_LABEL_MARGIN_BOTTOM: f32 = 25.0;

/// Height reserved for the legend at the bottom
const LEGEND_HEIGHT: f32 = 35.0;

/// Crosshair color
const CROSSHAIR_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 255, 0); // Yellow

impl UltraLogApp {
    /// Render the scatter plot view with two side-by-side plots
    pub fn render_scatter_plot_view(&mut self, ui: &mut egui::Ui) {
        // Check if we have an active tab with valid file
        if self.active_tab.is_none() || self.files.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Load a log file to use scatter plots")
                        .size(20.0)
                        .color(egui::Color32::GRAY),
                );
            });
            return;
        }

        // Render the tab bar first (same as log viewer)
        self.render_tab_bar(ui);
        ui.add_space(10.0);

        // Get available size for layout
        let available_width = ui.available_width();
        let available_height = ui.available_height();
        let plot_width = (available_width - 20.0) / 2.0; // 20px gap between plots

        // Use columns to ensure full height is used
        ui.columns(2, |columns| {
            // Left plot
            columns[0].set_width(plot_width);
            columns[0].set_height(available_height);
            self.render_scatter_plot_panel(&mut columns[0], true);

            // Right plot
            columns[1].set_width(plot_width);
            columns[1].set_height(available_height);
            self.render_scatter_plot_panel(&mut columns[1], false);
        });
    }

    /// Render a single scatter plot panel with controls
    fn render_scatter_plot_panel(&mut self, ui: &mut egui::Ui, is_left: bool) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };

        let config = if is_left {
            &self.tabs[tab_idx].scatter_plot_state.left
        } else {
            &self.tabs[tab_idx].scatter_plot_state.right
        };

        // Get channel names for the title
        let file_idx = config.file_index.unwrap_or(self.tabs[tab_idx].file_index);
        let title = self.get_scatter_plot_title(config, file_idx);

        // Title
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(&title)
                    .size(16.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            );
        });

        ui.add_space(8.0);

        // Axis selectors
        self.render_axis_selectors(ui, is_left);

        ui.add_space(8.0);

        // The actual scatter plot
        self.render_scatter_plot_chart(ui, is_left);
    }

    /// Get the title for a heatmap based on selected axes
    fn get_scatter_plot_title(&self, config: &ScatterPlotConfig, file_idx: usize) -> String {
        if file_idx >= self.files.len() {
            return "No Data".to_string();
        }

        let file = &self.files[file_idx];
        let use_normalization = self.field_normalization;
        let custom_mappings = &self.custom_normalizations;

        let get_name = |channel_idx: Option<usize>| -> String {
            channel_idx
                .and_then(|idx| file.log.channels.get(idx))
                .map(|ch| {
                    let name = ch.name();
                    if use_normalization {
                        normalize_channel_name_with_custom(&name, Some(custom_mappings))
                    } else {
                        name
                    }
                })
                .unwrap_or_else(|| "---".to_string())
        };

        let x_name = get_name(config.x_channel);
        let y_name = get_name(config.y_channel);

        // Always show "vs Hits" since that's the computed density
        format!("{} vs {} vs Hits", y_name, x_name)
    }

    /// Render axis selector dropdowns
    fn render_axis_selectors(&mut self, ui: &mut egui::Ui, is_left: bool) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };

        // Get values upfront to avoid borrow issues
        let tab_file_index = self.tabs[tab_idx].file_index;
        let (file_idx, current_x, current_y) = if is_left {
            let config = &self.tabs[tab_idx].scatter_plot_state.left;
            (
                config.file_index.unwrap_or(tab_file_index),
                config.x_channel,
                config.y_channel,
            )
        } else {
            let config = &self.tabs[tab_idx].scatter_plot_state.right;
            (
                config.file_index.unwrap_or(tab_file_index),
                config.x_channel,
                config.y_channel,
            )
        };

        if file_idx >= self.files.len() {
            return;
        }

        let file = &self.files[file_idx];

        // Sort channels: normalized fields first, then alphabetically
        let sorted_channels = sort_channels_by_priority(
            file.log.channels.len(),
            |idx| file.log.channels[idx].name(),
            self.field_normalization,
            Some(&self.custom_normalizations),
        );

        // Create a lookup map from channel index to display name for selected_text
        let channel_names: std::collections::HashMap<usize, String> = sorted_channels
            .iter()
            .map(|(idx, name, _)| (*idx, name.clone()))
            .collect();

        // Track which channel was selected
        let mut new_x_channel: Option<usize> = None;
        let mut new_y_channel: Option<usize> = None;

        ui.horizontal(|ui| {
            // X Axis selector
            ui.label("X Axis:");
            egui::ComboBox::from_id_salt(if is_left { "left_x" } else { "right_x" })
                .selected_text(
                    current_x
                        .and_then(|i| channel_names.get(&i).map(|n| n.as_str()))
                        .unwrap_or("Select..."),
                )
                .width(140.0)
                .show_ui(ui, |ui| {
                    for (idx, name, _is_normalized) in &sorted_channels {
                        if ui.selectable_label(current_x == Some(*idx), name).clicked() {
                            new_x_channel = Some(*idx);
                        }
                    }
                });

            ui.add_space(16.0);

            // Y Axis selector
            ui.label("Y Axis:");
            egui::ComboBox::from_id_salt(if is_left { "left_y" } else { "right_y" })
                .selected_text(
                    current_y
                        .and_then(|i| channel_names.get(&i).map(|n| n.as_str()))
                        .unwrap_or("Select..."),
                )
                .width(140.0)
                .show_ui(ui, |ui| {
                    for (idx, name, _is_normalized) in &sorted_channels {
                        if ui.selectable_label(current_y == Some(*idx), name).clicked() {
                            new_y_channel = Some(*idx);
                        }
                    }
                });

            ui.add_space(16.0);

            // Z Axis is always "Hits" (density)
            ui.label("Z Axis:");
            ui.label(egui::RichText::new("Hits").color(egui::Color32::from_rgb(150, 150, 150)));
        });

        // Apply channel updates after UI is rendered
        if let Some(x) = new_x_channel {
            if is_left {
                self.tabs[tab_idx].scatter_plot_state.left.x_channel = Some(x);
            } else {
                self.tabs[tab_idx].scatter_plot_state.right.x_channel = Some(x);
            }
        }
        if let Some(y) = new_y_channel {
            if is_left {
                self.tabs[tab_idx].scatter_plot_state.left.y_channel = Some(y);
            } else {
                self.tabs[tab_idx].scatter_plot_state.right.y_channel = Some(y);
            }
        }
    }

    /// Render the actual heatmap chart
    fn render_scatter_plot_chart(&mut self, ui: &mut egui::Ui, is_left: bool) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };

        let config = if is_left {
            &self.tabs[tab_idx].scatter_plot_state.left
        } else {
            &self.tabs[tab_idx].scatter_plot_state.right
        };

        let file_idx = config.file_index.unwrap_or(self.tabs[tab_idx].file_index);

        // Check if we have valid axis selections
        let (x_idx, y_idx) = match (config.x_channel, config.y_channel) {
            (Some(x), Some(y)) => (x, y),
            _ => {
                // Show placeholder message in a centered frame
                let available = ui.available_size();
                let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Select X and Y axes",
                    egui::FontId::proportional(16.0),
                    egui::Color32::GRAY,
                );
                return;
            }
        };

        if file_idx >= self.files.len() {
            return;
        }

        let file = &self.files[file_idx];
        let x_data = file.log.get_channel_data(x_idx);
        let y_data = file.log.get_channel_data(y_idx);

        if x_data.is_empty() || y_data.is_empty() || x_data.len() != y_data.len() {
            return;
        }

        // Calculate data bounds
        let x_min = x_data.iter().cloned().fold(f64::MAX, f64::min);
        let x_max = x_data.iter().cloned().fold(f64::MIN, f64::max);
        let y_min = y_data.iter().cloned().fold(f64::MAX, f64::min);
        let y_max = y_data.iter().cloned().fold(f64::MIN, f64::max);

        let x_range = if (x_max - x_min).abs() < f64::EPSILON {
            1.0
        } else {
            x_max - x_min
        };
        let y_range = if (y_max - y_min).abs() < f64::EPSILON {
            1.0
        } else {
            y_max - y_min
        };

        // Build 2D histogram (count hits in each bin)
        let mut histogram = vec![vec![0u32; HEATMAP_BINS]; HEATMAP_BINS];
        let mut max_hits: u32 = 0;

        for (&x, &y) in x_data.iter().zip(y_data.iter()) {
            let x_bin = (((x - x_min) / x_range) * (HEATMAP_BINS - 1) as f64).round() as usize;
            let y_bin = (((y - y_min) / y_range) * (HEATMAP_BINS - 1) as f64).round() as usize;

            let x_bin = x_bin.min(HEATMAP_BINS - 1);
            let y_bin = y_bin.min(HEATMAP_BINS - 1);

            histogram[y_bin][x_bin] += 1;
            max_hits = max_hits.max(histogram[y_bin][x_bin]);
        }

        // Allocate space for the heatmap (with click detection), reserving space for legend
        let available = ui.available_size();
        let chart_size = egui::vec2(available.x, (available.y - LEGEND_HEIGHT).max(100.0));
        let (full_rect, response) =
            ui.allocate_exact_size(chart_size, egui::Sense::click_and_drag());

        // Create inner plot rect with margins for axis labels
        let plot_rect = egui::Rect::from_min_max(
            egui::pos2(full_rect.left() + AXIS_LABEL_MARGIN_LEFT, full_rect.top()),
            egui::pos2(
                full_rect.right(),
                full_rect.bottom() - AXIS_LABEL_MARGIN_BOTTOM,
            ),
        );

        // Fill background with black
        let painter = ui.painter_at(full_rect);
        painter.rect_filled(plot_rect, 0.0, egui::Color32::BLACK);

        // Calculate cell size based on plot rect (not full rect)
        let cell_width = plot_rect.width() / HEATMAP_BINS as f32;
        let cell_height = plot_rect.height() / HEATMAP_BINS as f32;

        // Draw heatmap cells
        #[allow(clippy::needless_range_loop)]
        for y_bin in 0..HEATMAP_BINS {
            #[allow(clippy::needless_range_loop)]
            for x_bin in 0..HEATMAP_BINS {
                let hits = histogram[y_bin][x_bin];
                if hits > 0 {
                    // Normalize hits to 0-1 using log scale for better visualization
                    let normalized = if max_hits > 1 {
                        (hits as f64).ln() / (max_hits as f64).ln()
                    } else {
                        1.0
                    };
                    let color = Self::get_heat_color(normalized);

                    // Calculate cell position (Y is inverted - higher values at top)
                    let cell_x = plot_rect.left() + x_bin as f32 * cell_width;
                    let cell_y = plot_rect.bottom() - (y_bin + 1) as f32 * cell_height;

                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(cell_x, cell_y),
                            egui::vec2(cell_width + 0.5, cell_height + 0.5),
                        ),
                        0.0,
                        color,
                    );
                }
            }
        }

        // Draw axes labels
        let text_color = egui::Color32::from_rgb(200, 200, 200);

        // Y axis labels (left margin, outside plot area)
        let y_labels = 5;
        for i in 0..=y_labels {
            let t = i as f64 / y_labels as f64;
            let value = y_min + t * y_range;
            let y_pos = plot_rect.bottom() - t as f32 * plot_rect.height();

            painter.text(
                egui::pos2(plot_rect.left() - 5.0, y_pos),
                egui::Align2::RIGHT_CENTER,
                format!("{:.1}", value),
                egui::FontId::proportional(10.0),
                text_color,
            );

            // Grid line (inside plot area)
            painter.line_segment(
                [
                    egui::pos2(plot_rect.left(), y_pos),
                    egui::pos2(plot_rect.right(), y_pos),
                ],
                egui::Stroke::new(0.5, egui::Color32::from_rgb(60, 60, 60)),
            );
        }

        // X axis labels (bottom margin, outside plot area)
        let x_labels = 5;
        for i in 0..=x_labels {
            let t = i as f64 / x_labels as f64;
            let value = x_min + t * x_range;
            let x_pos = plot_rect.left() + t as f32 * plot_rect.width();

            painter.text(
                egui::pos2(x_pos, plot_rect.bottom() + 5.0),
                egui::Align2::CENTER_TOP,
                format!("{:.0}", value),
                egui::FontId::proportional(10.0),
                text_color,
            );

            // Grid line (inside plot area)
            painter.line_segment(
                [
                    egui::pos2(x_pos, plot_rect.top()),
                    egui::pos2(x_pos, plot_rect.bottom()),
                ],
                egui::Stroke::new(0.5, egui::Color32::from_rgb(60, 60, 60)),
            );
        }

        // Get mutable config for click handling
        let config = if is_left {
            &mut self.tabs[tab_idx].scatter_plot_state.left
        } else {
            &mut self.tabs[tab_idx].scatter_plot_state.right
        };

        // Draw selected point crosshairs (persistent)
        if let Some(ref selected) = config.selected_point {
            let sel_rel_x = ((selected.x_value - x_min) / x_range) as f32;
            let sel_rel_y = ((selected.y_value - y_min) / y_range) as f32;

            if (0.0..=1.0).contains(&sel_rel_x) && (0.0..=1.0).contains(&sel_rel_y) {
                let sel_x = plot_rect.left() + sel_rel_x * plot_rect.width();
                let sel_y = plot_rect.bottom() - sel_rel_y * plot_rect.height();

                // Draw persistent crosshairs (cyan for selected)
                let selected_color = egui::Color32::from_rgb(0, 255, 255);
                painter.line_segment(
                    [
                        egui::pos2(sel_x, plot_rect.top()),
                        egui::pos2(sel_x, plot_rect.bottom()),
                    ],
                    egui::Stroke::new(1.5, selected_color),
                );
                painter.line_segment(
                    [
                        egui::pos2(plot_rect.left(), sel_y),
                        egui::pos2(plot_rect.right(), sel_y),
                    ],
                    egui::Stroke::new(1.5, selected_color),
                );

                // Draw a small circle at intersection
                painter.circle_filled(egui::pos2(sel_x, sel_y), 4.0, selected_color);
            }
        }

        // Show hover crosshairs and handle click (only within plot area)
        if let Some(pos) = response.hover_pos() {
            // Check if hover position is within the plot rect
            if plot_rect.contains(pos) {
                let rel_x = (pos.x - plot_rect.left()) / plot_rect.width();
                let rel_y = 1.0 - (pos.y - plot_rect.top()) / plot_rect.height();

                if (0.0..=1.0).contains(&rel_x) && (0.0..=1.0).contains(&rel_y) {
                    let x_val = x_min + rel_x as f64 * x_range;
                    let y_val = y_min + rel_y as f64 * y_range;

                    let x_bin = (rel_x * (HEATMAP_BINS - 1) as f32).round() as usize;
                    let y_bin = (rel_y * (HEATMAP_BINS - 1) as f32).round() as usize;
                    let hits = histogram[y_bin.min(HEATMAP_BINS - 1)][x_bin.min(HEATMAP_BINS - 1)];

                    // Draw hover crosshairs (yellow, thinner than selected)
                    painter.line_segment(
                        [
                            egui::pos2(pos.x, plot_rect.top()),
                            egui::pos2(pos.x, plot_rect.bottom()),
                        ],
                        egui::Stroke::new(1.0, CROSSHAIR_COLOR),
                    );
                    painter.line_segment(
                        [
                            egui::pos2(plot_rect.left(), pos.y),
                            egui::pos2(plot_rect.right(), pos.y),
                        ],
                        egui::Stroke::new(1.0, CROSSHAIR_COLOR),
                    );

                    // Draw tooltip in corner
                    let tooltip_text = format!("X: {:.1}\nY: {:.1}\nHits: {}", x_val, y_val, hits);
                    painter.text(
                        egui::pos2(plot_rect.right() - 10.0, plot_rect.top() + 15.0),
                        egui::Align2::RIGHT_TOP,
                        tooltip_text,
                        egui::FontId::proportional(11.0),
                        egui::Color32::WHITE,
                    );

                    // Handle click to select point
                    if response.clicked() {
                        config.selected_point = Some(SelectedHeatmapPoint {
                            x_value: x_val,
                            y_value: y_val,
                            hits,
                        });
                    }
                }
            }
        }

        // Add spacing before legend
        ui.add_space(8.0);

        // Render color scale legend and selected point info
        self.render_heatmap_legend(ui, max_hits, is_left, x_min, x_max, y_min, y_max);
    }

    /// Get a color from the heat map gradient based on normalized value (0-1)
    fn get_heat_color(normalized: f64) -> egui::Color32 {
        let t = normalized.clamp(0.0, 1.0);
        let scaled = t * (HEAT_COLORS.len() - 1) as f64;
        let idx = scaled.floor() as usize;
        let frac = scaled - idx as f64;

        if idx >= HEAT_COLORS.len() - 1 {
            let c = HEAT_COLORS[HEAT_COLORS.len() - 1];
            return egui::Color32::from_rgb(c[0], c[1], c[2]);
        }

        let c1 = HEAT_COLORS[idx];
        let c2 = HEAT_COLORS[idx + 1];

        let r = (c1[0] as f64 + (c2[0] as f64 - c1[0] as f64) * frac) as u8;
        let g = (c1[1] as f64 + (c2[1] as f64 - c1[1] as f64) * frac) as u8;
        let b = (c1[2] as f64 + (c2[2] as f64 - c1[2] as f64) * frac) as u8;

        egui::Color32::from_rgb(r, g, b)
    }

    /// Render the heatmap legend with color scale and selected point info
    #[allow(clippy::too_many_arguments)]
    fn render_heatmap_legend(
        &mut self,
        ui: &mut egui::Ui,
        max_hits: u32,
        is_left: bool,
        _x_min: f64,
        _x_max: f64,
        _y_min: f64,
        _y_max: f64,
    ) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };

        // First, gather the data we need (immutable borrow)
        let config = if is_left {
            &self.tabs[tab_idx].scatter_plot_state.left
        } else {
            &self.tabs[tab_idx].scatter_plot_state.right
        };

        let selected_point = config.selected_point.clone();
        let file_idx = config.file_index.unwrap_or(self.tabs[tab_idx].file_index);
        let x_channel = config.x_channel;
        let y_channel = config.y_channel;

        // Get channel names
        let (x_name, y_name) = if file_idx < self.files.len() {
            let file = &self.files[file_idx];
            let x_name = x_channel
                .and_then(|i| file.log.channels.get(i))
                .map(|c| {
                    if self.field_normalization {
                        normalize_channel_name_with_custom(
                            &c.name(),
                            Some(&self.custom_normalizations),
                        )
                    } else {
                        c.name()
                    }
                })
                .unwrap_or_else(|| "X".to_string());
            let y_name = y_channel
                .and_then(|i| file.log.channels.get(i))
                .map(|c| {
                    if self.field_normalization {
                        normalize_channel_name_with_custom(
                            &c.name(),
                            Some(&self.custom_normalizations),
                        )
                    } else {
                        c.name()
                    }
                })
                .unwrap_or_else(|| "Y".to_string());
            (x_name, y_name)
        } else {
            ("X".to_string(), "Y".to_string())
        };

        let mut should_clear = false;

        ui.horizontal(|ui| {
            ui.add_space(4.0);

            // Color scale legend
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220))
                .corner_radius(4)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Hits:")
                                .size(11.0)
                                .color(egui::Color32::WHITE),
                        );

                        // Color gradient bar
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(100.0, 14.0), egui::Sense::hover());

                        let painter = ui.painter();
                        let steps = 25;
                        let step_width = rect.width() / steps as f32;

                        for i in 0..steps {
                            let t = i as f64 / steps as f64;
                            let color = Self::get_heat_color(t);
                            let x = rect.left() + i as f32 * step_width;
                            painter.rect_filled(
                                egui::Rect::from_min_size(
                                    egui::pos2(x, rect.top()),
                                    egui::vec2(step_width + 1.0, rect.height()),
                                ),
                                0.0,
                                color,
                            );
                        }

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("0-{}", max_hits))
                                .size(10.0)
                                .color(egui::Color32::WHITE),
                        );
                    });
                });

            ui.add_space(8.0);

            // Selected point legend (similar to log viewer channel display)
            if let Some(ref selected) = selected_point {
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220))
                    .corner_radius(4)
                    .inner_margin(6.0)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 255, 255)))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Cyan indicator dot
                            let (dot_rect, _) = ui
                                .allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter().circle_filled(
                                dot_rect.center(),
                                5.0,
                                egui::Color32::from_rgb(0, 255, 255),
                            );

                            ui.add_space(4.0);

                            ui.label(
                                egui::RichText::new(format!(
                                    "{}: {:.1}  |  {}: {:.1}  |  Hits: {}",
                                    x_name,
                                    selected.x_value,
                                    y_name,
                                    selected.y_value,
                                    selected.hits
                                ))
                                .size(11.0)
                                .color(egui::Color32::WHITE),
                            );

                            ui.add_space(8.0);

                            // Clear button
                            if ui.small_button("X").clicked() {
                                should_clear = true;
                            }
                        });
                    });
            }
        });

        // Clear selection if button was clicked
        if should_clear {
            if is_left {
                self.tabs[tab_idx].scatter_plot_state.left.selected_point = None;
            } else {
                self.tabs[tab_idx].scatter_plot_state.right.selected_point = None;
            }
        }
    }
}
