//! Histogram / 2D heatmap view for analyzing channel distributions.
//!
//! This module provides a histogram view where users can visualize
//! relationships between channels as a 2D grid with configurable
//! cell coloring based on average Z-value or hit count.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::normalize::sort_channels_by_priority;
use crate::state::HistogramMode;

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

/// Fixed grid size (16x16)
const GRID_SIZE: usize = 16;

/// Margin for axis labels
const AXIS_LABEL_MARGIN_LEFT: f32 = 60.0;
const AXIS_LABEL_MARGIN_BOTTOM: f32 = 30.0;

/// Height reserved for legend at bottom
const LEGEND_HEIGHT: f32 = 45.0;

/// Current position indicator color (cyan, matches chart cursor)
const CURSOR_COLOR: egui::Color32 = egui::Color32::from_rgb(0, 255, 255);

/// Cell highlight color for current position
const CELL_HIGHLIGHT_COLOR: egui::Color32 = egui::Color32::WHITE;

impl UltraLogApp {
    /// Main entry point: render the histogram view
    pub fn render_histogram_view(&mut self, ui: &mut egui::Ui) {
        if self.active_tab.is_none() || self.files.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Load a log file to use histogram")
                        .size(20.0)
                        .color(egui::Color32::GRAY),
                );
            });
            return;
        }

        // Render tab bar
        self.render_tab_bar(ui);
        ui.add_space(10.0);

        // Render axis selectors and mode toggle
        self.render_histogram_controls(ui);
        ui.add_space(8.0);

        // Render the histogram grid
        self.render_histogram_grid(ui);
    }

    /// Render the control panel with axis selectors and mode toggle
    fn render_histogram_controls(&mut self, ui: &mut egui::Ui) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };
        let file_idx = self.tabs[tab_idx].file_index;

        if file_idx >= self.files.len() {
            return;
        }

        let file = &self.files[file_idx];

        // Sort channels for dropdown
        let sorted_channels = sort_channels_by_priority(
            file.log.channels.len(),
            |idx| file.log.channels[idx].name(),
            self.field_normalization,
            Some(&self.custom_normalizations),
        );

        // Get current selections
        let config = &self.tabs[tab_idx].histogram_state.config;
        let current_x = config.x_channel;
        let current_y = config.y_channel;
        let current_z = config.z_channel;
        let current_mode = config.mode;

        // Build channel name lookup
        let channel_names: std::collections::HashMap<usize, String> = sorted_channels
            .iter()
            .map(|(idx, name, _)| (*idx, name.clone()))
            .collect();

        // Track selections for deferred updates
        let mut new_x: Option<usize> = None;
        let mut new_y: Option<usize> = None;
        let mut new_z: Option<usize> = None;
        let mut new_mode: Option<HistogramMode> = None;

        ui.horizontal(|ui| {
            // X Axis selector
            ui.label("X Axis:");
            egui::ComboBox::from_id_salt("histogram_x")
                .selected_text(
                    current_x
                        .and_then(|i| channel_names.get(&i).map(|n| n.as_str()))
                        .unwrap_or("Select..."),
                )
                .width(140.0)
                .show_ui(ui, |ui| {
                    for (idx, name, _) in &sorted_channels {
                        if ui.selectable_label(current_x == Some(*idx), name).clicked() {
                            new_x = Some(*idx);
                        }
                    }
                });

            ui.add_space(16.0);

            // Y Axis selector
            ui.label("Y Axis:");
            egui::ComboBox::from_id_salt("histogram_y")
                .selected_text(
                    current_y
                        .and_then(|i| channel_names.get(&i).map(|n| n.as_str()))
                        .unwrap_or("Select..."),
                )
                .width(140.0)
                .show_ui(ui, |ui| {
                    for (idx, name, _) in &sorted_channels {
                        if ui.selectable_label(current_y == Some(*idx), name).clicked() {
                            new_y = Some(*idx);
                        }
                    }
                });

            ui.add_space(16.0);

            // Z Axis selector (only enabled in AverageZ mode)
            let z_enabled = current_mode == HistogramMode::AverageZ;
            ui.add_enabled_ui(z_enabled, |ui| {
                ui.label("Z Axis:");
                egui::ComboBox::from_id_salt("histogram_z")
                    .selected_text(
                        current_z
                            .and_then(|i| channel_names.get(&i).map(|n| n.as_str()))
                            .unwrap_or("Select..."),
                    )
                    .width(140.0)
                    .show_ui(ui, |ui| {
                        for (idx, name, _) in &sorted_channels {
                            if ui.selectable_label(current_z == Some(*idx), name).clicked() {
                                new_z = Some(*idx);
                            }
                        }
                    });
            });

            ui.add_space(24.0);

            // Mode toggle
            ui.label("Mode:");
            if ui
                .selectable_label(current_mode == HistogramMode::AverageZ, "Average Z")
                .clicked()
            {
                new_mode = Some(HistogramMode::AverageZ);
            }
            if ui
                .selectable_label(current_mode == HistogramMode::HitCount, "Hit Count")
                .clicked()
            {
                new_mode = Some(HistogramMode::HitCount);
            }
        });

        // Apply deferred updates
        if let Some(x) = new_x {
            self.tabs[tab_idx].histogram_state.config.x_channel = Some(x);
        }
        if let Some(y) = new_y {
            self.tabs[tab_idx].histogram_state.config.y_channel = Some(y);
        }
        if let Some(z) = new_z {
            self.tabs[tab_idx].histogram_state.config.z_channel = Some(z);
        }
        if let Some(mode) = new_mode {
            self.tabs[tab_idx].histogram_state.config.mode = mode;
        }
    }

    /// Render the histogram grid with current position indicator
    fn render_histogram_grid(&mut self, ui: &mut egui::Ui) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };

        let config = &self.tabs[tab_idx].histogram_state.config;
        let file_idx = self.tabs[tab_idx].file_index;
        let mode = config.mode;

        // Check valid axis selections
        let (x_idx, y_idx) = match (config.x_channel, config.y_channel) {
            (Some(x), Some(y)) => (x, y),
            _ => {
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

        // Check Z axis for AverageZ mode
        let z_idx = if mode == HistogramMode::AverageZ {
            match config.z_channel {
                Some(z) => Some(z),
                None => {
                    let available = ui.available_size();
                    let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Select Z axis for Average mode",
                        egui::FontId::proportional(16.0),
                        egui::Color32::GRAY,
                    );
                    return;
                }
            }
        } else {
            None
        };

        if file_idx >= self.files.len() {
            return;
        }

        let file = &self.files[file_idx];
        let x_data = file.log.get_channel_data(x_idx);
        let y_data = file.log.get_channel_data(y_idx);
        let z_data = z_idx.map(|z| file.log.get_channel_data(z));

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

        // Build histogram grid
        // For hit count: just count hits per cell
        // For average Z: accumulate Z values and count, then divide
        let mut hit_counts = vec![vec![0u32; GRID_SIZE]; GRID_SIZE];
        let mut z_sums = vec![vec![0.0f64; GRID_SIZE]; GRID_SIZE];

        for i in 0..x_data.len() {
            let x_bin = (((x_data[i] - x_min) / x_range) * (GRID_SIZE - 1) as f64).round() as usize;
            let y_bin = (((y_data[i] - y_min) / y_range) * (GRID_SIZE - 1) as f64).round() as usize;
            let x_bin = x_bin.min(GRID_SIZE - 1);
            let y_bin = y_bin.min(GRID_SIZE - 1);

            hit_counts[y_bin][x_bin] += 1;
            if let Some(ref z) = z_data {
                z_sums[y_bin][x_bin] += z[i];
            }
        }

        // Calculate cell values and find min/max for color scaling
        let mut cell_values = vec![vec![None::<f64>; GRID_SIZE]; GRID_SIZE];
        let mut min_value: f64 = f64::MAX;
        let mut max_value: f64 = f64::MIN;

        for y_bin in 0..GRID_SIZE {
            for x_bin in 0..GRID_SIZE {
                let hits = hit_counts[y_bin][x_bin];
                if hits > 0 {
                    let value = match mode {
                        HistogramMode::HitCount => hits as f64,
                        HistogramMode::AverageZ => z_sums[y_bin][x_bin] / hits as f64,
                    };
                    cell_values[y_bin][x_bin] = Some(value);
                    min_value = min_value.min(value);
                    max_value = max_value.max(value);
                }
            }
        }

        // Handle case where all values are the same
        let value_range = if (max_value - min_value).abs() < f64::EPSILON {
            1.0
        } else {
            max_value - min_value
        };

        // Allocate space for the grid
        let available = ui.available_size();
        let chart_size = egui::vec2(available.x, (available.y - LEGEND_HEIGHT).max(200.0));
        let (full_rect, response) = ui.allocate_exact_size(chart_size, egui::Sense::hover());

        // Create inner plot rect with margins
        let plot_rect = egui::Rect::from_min_max(
            egui::pos2(full_rect.left() + AXIS_LABEL_MARGIN_LEFT, full_rect.top()),
            egui::pos2(
                full_rect.right(),
                full_rect.bottom() - AXIS_LABEL_MARGIN_BOTTOM,
            ),
        );

        let painter = ui.painter_at(full_rect);
        painter.rect_filled(plot_rect, 0.0, egui::Color32::BLACK);

        let cell_width = plot_rect.width() / GRID_SIZE as f32;
        let cell_height = plot_rect.height() / GRID_SIZE as f32;

        // Draw grid cells
        #[allow(clippy::needless_range_loop)]
        for y_bin in 0..GRID_SIZE {
            #[allow(clippy::needless_range_loop)]
            for x_bin in 0..GRID_SIZE {
                if let Some(value) = cell_values[y_bin][x_bin] {
                    // Normalize to 0-1 for color scaling
                    let normalized = if mode == HistogramMode::HitCount && max_value > 1.0 {
                        // Use log scale for hit counts
                        (value.ln() / max_value.ln()).clamp(0.0, 1.0)
                    } else {
                        // Linear scale for average Z values
                        ((value - min_value) / value_range).clamp(0.0, 1.0)
                    };
                    let color = Self::get_histogram_color(normalized);

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

        // Draw grid lines
        let grid_color = egui::Color32::from_rgb(60, 60, 60);
        for i in 0..=GRID_SIZE {
            let x = plot_rect.left() + i as f32 * cell_width;
            let y = plot_rect.top() + i as f32 * cell_height;
            painter.line_segment(
                [
                    egui::pos2(x, plot_rect.top()),
                    egui::pos2(x, plot_rect.bottom()),
                ],
                egui::Stroke::new(0.5, grid_color),
            );
            painter.line_segment(
                [
                    egui::pos2(plot_rect.left(), y),
                    egui::pos2(plot_rect.right(), y),
                ],
                egui::Stroke::new(0.5, grid_color),
            );
        }

        // Draw axis labels
        let text_color = egui::Color32::from_rgb(200, 200, 200);

        // Y axis labels
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let value = y_min + t * y_range;
            let y_pos = plot_rect.bottom() - t as f32 * plot_rect.height();
            painter.text(
                egui::pos2(plot_rect.left() - 5.0, y_pos),
                egui::Align2::RIGHT_CENTER,
                format!("{:.1}", value),
                egui::FontId::proportional(10.0),
                text_color,
            );
        }

        // X axis labels
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let value = x_min + t * x_range;
            let x_pos = plot_rect.left() + t as f32 * plot_rect.width();
            painter.text(
                egui::pos2(x_pos, plot_rect.bottom() + 5.0),
                egui::Align2::CENTER_TOP,
                format!("{:.0}", value),
                egui::FontId::proportional(10.0),
                text_color,
            );
        }

        // Draw current position indicator (cursor time)
        if let Some(cursor_record) = self.get_cursor_record() {
            if cursor_record < x_data.len() {
                let cursor_x = x_data[cursor_record];
                let cursor_y = y_data[cursor_record];

                // Calculate position in plot coordinates
                let rel_x = ((cursor_x - x_min) / x_range) as f32;
                let rel_y = ((cursor_y - y_min) / y_range) as f32;

                if (0.0..=1.0).contains(&rel_x) && (0.0..=1.0).contains(&rel_y) {
                    let pos_x = plot_rect.left() + rel_x * plot_rect.width();
                    let pos_y = plot_rect.bottom() - rel_y * plot_rect.height();

                    // Highlight the cell containing the cursor
                    let cell_x_bin = (rel_x * (GRID_SIZE - 1) as f32).round() as usize;
                    let cell_y_bin = (rel_y * (GRID_SIZE - 1) as f32).round() as usize;
                    let cell_x_bin = cell_x_bin.min(GRID_SIZE - 1);
                    let cell_y_bin = cell_y_bin.min(GRID_SIZE - 1);

                    let highlight_x = plot_rect.left() + cell_x_bin as f32 * cell_width;
                    let highlight_y = plot_rect.bottom() - (cell_y_bin + 1) as f32 * cell_height;

                    // Draw cell highlight using line segments
                    let highlight_rect = egui::Rect::from_min_size(
                        egui::pos2(highlight_x, highlight_y),
                        egui::vec2(cell_width, cell_height),
                    );
                    let stroke = egui::Stroke::new(2.0, CELL_HIGHLIGHT_COLOR);
                    // Top
                    painter.line_segment(
                        [highlight_rect.left_top(), highlight_rect.right_top()],
                        stroke,
                    );
                    // Bottom
                    painter.line_segment(
                        [highlight_rect.left_bottom(), highlight_rect.right_bottom()],
                        stroke,
                    );
                    // Left
                    painter.line_segment(
                        [highlight_rect.left_top(), highlight_rect.left_bottom()],
                        stroke,
                    );
                    // Right
                    painter.line_segment(
                        [highlight_rect.right_top(), highlight_rect.right_bottom()],
                        stroke,
                    );

                    // Draw small filled circle at exact position
                    painter.circle_filled(egui::pos2(pos_x, pos_y), 6.0, CURSOR_COLOR);
                    painter.circle_stroke(
                        egui::pos2(pos_x, pos_y),
                        6.0,
                        egui::Stroke::new(2.0, egui::Color32::WHITE),
                    );
                }
            }
        }

        // Handle hover tooltip
        if let Some(pos) = response.hover_pos() {
            if plot_rect.contains(pos) {
                let rel_x = (pos.x - plot_rect.left()) / plot_rect.width();
                let rel_y = 1.0 - (pos.y - plot_rect.top()) / plot_rect.height();

                if (0.0..=1.0).contains(&rel_x) && (0.0..=1.0).contains(&rel_y) {
                    let x_val = x_min + rel_x as f64 * x_range;
                    let y_val = y_min + rel_y as f64 * y_range;

                    let x_bin = (rel_x * (GRID_SIZE - 1) as f32).round() as usize;
                    let y_bin = (rel_y * (GRID_SIZE - 1) as f32).round() as usize;
                    let x_bin = x_bin.min(GRID_SIZE - 1);
                    let y_bin = y_bin.min(GRID_SIZE - 1);

                    let hits = hit_counts[y_bin][x_bin];
                    let cell_value = cell_values[y_bin][x_bin];

                    let tooltip_text = match mode {
                        HistogramMode::HitCount => {
                            format!("X: {:.1}\nY: {:.1}\nHits: {}", x_val, y_val, hits)
                        }
                        HistogramMode::AverageZ => {
                            let avg = cell_value
                                .map(|v| format!("{:.2}", v))
                                .unwrap_or("-".to_string());
                            format!(
                                "X: {:.1}\nY: {:.1}\nAvg Z: {}\nHits: {}",
                                x_val, y_val, avg, hits
                            )
                        }
                    };

                    // Draw hover crosshairs
                    let hover_x = plot_rect.left() + rel_x * plot_rect.width();
                    let hover_y = plot_rect.top() + (1.0 - rel_y) * plot_rect.height();
                    let crosshair_color = egui::Color32::from_rgb(255, 255, 0);

                    painter.line_segment(
                        [
                            egui::pos2(hover_x, plot_rect.top()),
                            egui::pos2(hover_x, plot_rect.bottom()),
                        ],
                        egui::Stroke::new(1.0, crosshair_color),
                    );
                    painter.line_segment(
                        [
                            egui::pos2(plot_rect.left(), hover_y),
                            egui::pos2(plot_rect.right(), hover_y),
                        ],
                        egui::Stroke::new(1.0, crosshair_color),
                    );

                    painter.text(
                        egui::pos2(plot_rect.right() - 10.0, plot_rect.top() + 15.0),
                        egui::Align2::RIGHT_TOP,
                        tooltip_text,
                        egui::FontId::proportional(11.0),
                        egui::Color32::WHITE,
                    );
                }
            }
        }

        // Render legend
        ui.add_space(8.0);
        self.render_histogram_legend(ui, min_value, max_value, x_data.len(), mode);
    }

    /// Get a color from the heat map gradient based on normalized value (0-1)
    fn get_histogram_color(normalized: f64) -> egui::Color32 {
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

    /// Render the legend with color scale and stats
    fn render_histogram_legend(
        &self,
        ui: &mut egui::Ui,
        min_value: f64,
        max_value: f64,
        total_points: usize,
        mode: HistogramMode,
    ) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);

            // Color scale legend
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220))
                .corner_radius(4)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let label = match mode {
                            HistogramMode::HitCount => "Hits:",
                            HistogramMode::AverageZ => "Value:",
                        };
                        ui.label(
                            egui::RichText::new(label)
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
                            let color = Self::get_histogram_color(t);
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
                        let range_text = if mode == HistogramMode::HitCount {
                            format!("0-{:.0}", max_value)
                        } else {
                            format!("{:.1}-{:.1}", min_value, max_value)
                        };
                        ui.label(
                            egui::RichText::new(range_text)
                                .size(10.0)
                                .color(egui::Color32::WHITE),
                        );
                    });
                });

            ui.add_space(16.0);

            // Stats panel
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220))
                .corner_radius(4)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Grid: {}x{}", GRID_SIZE, GRID_SIZE))
                                .size(11.0)
                                .color(egui::Color32::WHITE),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!("Points: {}", total_points))
                                .size(11.0)
                                .color(egui::Color32::WHITE),
                        );
                    });
                });
        });
    }
}
