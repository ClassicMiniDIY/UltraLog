//! Histogram / 2D heatmap view for analyzing channel distributions.
//!
//! This module provides a histogram view where users can visualize
//! relationships between channels as a 2D grid with configurable
//! cell coloring based on average Z-value or hit count.

use eframe::egui;
use rust_i18n::t;

use crate::app::UltraLogApp;
use crate::normalize::sort_channels_by_priority;
use crate::state::{HistogramGridSize, HistogramMode, SelectedHistogramCell};

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

/// Margin for axis labels and titles
const AXIS_LABEL_MARGIN_LEFT: f32 = 75.0;
const AXIS_LABEL_MARGIN_BOTTOM: f32 = 45.0;

/// Height reserved for legend at bottom
const LEGEND_HEIGHT: f32 = 55.0;

/// Current position indicator color (cyan, matches chart cursor)
const CURSOR_COLOR: egui::Color32 = egui::Color32::from_rgb(0, 255, 255);

/// Cell highlight color for current position
const CELL_HIGHLIGHT_COLOR: egui::Color32 = egui::Color32::WHITE;

/// Selected cell highlight color
const SELECTED_CELL_COLOR: egui::Color32 = egui::Color32::from_rgb(255, 165, 0); // Orange

/// Crosshair color for cursor tracking during playback
const CURSOR_CROSSHAIR_COLOR: egui::Color32 = egui::Color32::from_rgb(128, 128, 128); // Grey

/// Calculate relative luminance for WCAG contrast ratio
/// Uses the sRGB colorspace formula from WCAG 2.1
fn calculate_luminance(color: egui::Color32) -> f64 {
    let r = linearize_channel(color.r());
    let g = linearize_channel(color.g());
    let b = linearize_channel(color.b());
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Linearize an sRGB channel value (0-255) to linear RGB
fn linearize_channel(value: u8) -> f64 {
    let v = value as f64 / 255.0;
    if v <= 0.03928 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Calculate WCAG contrast ratio between two colors
fn contrast_ratio(color1: egui::Color32, color2: egui::Color32) -> f64 {
    let l1 = calculate_luminance(color1);
    let l2 = calculate_luminance(color2);
    let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Get the best text color (black or white) for AAA compliance on given background
/// Returns the color that provides the highest contrast ratio
fn get_aaa_text_color(background: egui::Color32) -> egui::Color32 {
    let white_contrast = contrast_ratio(egui::Color32::WHITE, background);
    let black_contrast = contrast_ratio(egui::Color32::BLACK, background);

    // Choose whichever provides better contrast
    // AAA requires 7:1 for normal text, but we pick the better option regardless
    if white_contrast >= black_contrast {
        egui::Color32::WHITE
    } else {
        egui::Color32::BLACK
    }
}

impl UltraLogApp {
    /// Main entry point: render the histogram view
    pub fn render_histogram_view(&mut self, ui: &mut egui::Ui) {
        if self.active_tab.is_none() || self.files.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(t!("histogram.no_file_loaded"))
                        .size(self.scaled_font(20.0))
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

    /// Render the control panel with axis selectors, mode toggle, and grid size
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
        let current_grid_size = config.grid_size;

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
        let mut new_grid_size: Option<HistogramGridSize> = None;

        // Pre-compute scaled font sizes
        let font_14 = self.scaled_font(14.0);
        let font_15 = self.scaled_font(15.0);

        ui.horizontal(|ui| {
            // X Axis selector
            ui.label(egui::RichText::new(t!("histogram.x_axis")).size(font_15));
            egui::ComboBox::from_id_salt("histogram_x")
                .selected_text(
                    egui::RichText::new(
                        current_x
                            .and_then(|i| channel_names.get(&i).map(|n| n.as_str()))
                            .unwrap_or("Select..."),
                    )
                    .size(font_14),
                )
                .width(160.0)
                .show_ui(ui, |ui| {
                    for (idx, name, _) in &sorted_channels {
                        if ui
                            .selectable_label(
                                current_x == Some(*idx),
                                egui::RichText::new(name).size(font_14),
                            )
                            .clicked()
                        {
                            new_x = Some(*idx);
                        }
                    }
                });

            ui.add_space(16.0);

            // Y Axis selector
            ui.label(egui::RichText::new(t!("histogram.y_axis")).size(font_15));
            egui::ComboBox::from_id_salt("histogram_y")
                .selected_text(
                    egui::RichText::new(
                        current_y
                            .and_then(|i| channel_names.get(&i).map(|n| n.as_str()))
                            .unwrap_or("Select..."),
                    )
                    .size(font_14),
                )
                .width(160.0)
                .show_ui(ui, |ui| {
                    for (idx, name, _) in &sorted_channels {
                        if ui
                            .selectable_label(
                                current_y == Some(*idx),
                                egui::RichText::new(name).size(font_14),
                            )
                            .clicked()
                        {
                            new_y = Some(*idx);
                        }
                    }
                });

            ui.add_space(16.0);

            // Z Axis selector (only enabled in AverageZ mode)
            let z_enabled = current_mode == HistogramMode::AverageZ;
            ui.add_enabled_ui(z_enabled, |ui| {
                ui.label(egui::RichText::new(t!("histogram.z_axis")).size(font_15));
                egui::ComboBox::from_id_salt("histogram_z")
                    .selected_text(
                        egui::RichText::new(
                            current_z
                                .and_then(|i| channel_names.get(&i).map(|n| n.as_str()))
                                .unwrap_or("Select..."),
                        )
                        .size(font_14),
                    )
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for (idx, name, _) in &sorted_channels {
                            if ui
                                .selectable_label(
                                    current_z == Some(*idx),
                                    egui::RichText::new(name).size(font_14),
                                )
                                .clicked()
                            {
                                new_z = Some(*idx);
                            }
                        }
                    });
            });

            ui.add_space(20.0);

            // Grid size selector
            ui.label(egui::RichText::new(t!("histogram.grid")).size(font_15));
            egui::ComboBox::from_id_salt("histogram_grid_size")
                .selected_text(egui::RichText::new(current_grid_size.name()).size(font_14))
                .width(80.0)
                .show_ui(ui, |ui| {
                    let sizes = [
                        HistogramGridSize::Size16,
                        HistogramGridSize::Size32,
                        HistogramGridSize::Size64,
                    ];
                    for size in sizes {
                        if ui
                            .selectable_label(
                                current_grid_size == size,
                                egui::RichText::new(size.name()).size(font_14),
                            )
                            .clicked()
                        {
                            new_grid_size = Some(size);
                        }
                    }
                });

            ui.add_space(20.0);

            // Mode toggle
            ui.label(egui::RichText::new(t!("histogram.mode")).size(font_15));
            if ui
                .selectable_label(
                    current_mode == HistogramMode::AverageZ,
                    egui::RichText::new(t!("histogram.average_z")).size(font_14),
                )
                .clicked()
            {
                new_mode = Some(HistogramMode::AverageZ);
            }
            if ui
                .selectable_label(
                    current_mode == HistogramMode::HitCount,
                    egui::RichText::new(t!("histogram.hit_count")).size(font_14),
                )
                .clicked()
            {
                new_mode = Some(HistogramMode::HitCount);
            }
        });

        // Apply deferred updates
        let config = &mut self.tabs[tab_idx].histogram_state.config;
        if let Some(x) = new_x {
            config.x_channel = Some(x);
            config.selected_cell = None; // Clear selection on axis change
        }
        if let Some(y) = new_y {
            config.y_channel = Some(y);
            config.selected_cell = None;
        }
        if let Some(z) = new_z {
            config.z_channel = Some(z);
            config.selected_cell = None;
        }
        if let Some(mode) = new_mode {
            config.mode = mode;
            config.selected_cell = None;
        }
        if let Some(size) = new_grid_size {
            config.grid_size = size;
            config.selected_cell = None;
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
        let grid_size = config.grid_size.size();

        // Pre-compute scaled font sizes for use in closures
        let font_10 = self.scaled_font(10.0);
        let font_12 = self.scaled_font(12.0);
        let font_13 = self.scaled_font(13.0);
        let font_16 = self.scaled_font(16.0);

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
                    egui::FontId::proportional(font_16),
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
                        egui::FontId::proportional(font_16),
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

        // Build histogram grid with full statistics
        let mut hit_counts = vec![vec![0u32; grid_size]; grid_size];
        let mut z_sums = vec![vec![0.0f64; grid_size]; grid_size];
        let mut z_sum_sq = vec![vec![0.0f64; grid_size]; grid_size];
        let mut z_mins = vec![vec![f64::MAX; grid_size]; grid_size];
        let mut z_maxs = vec![vec![f64::MIN; grid_size]; grid_size];

        for i in 0..x_data.len() {
            let x_bin = (((x_data[i] - x_min) / x_range) * (grid_size - 1) as f64).round() as usize;
            let y_bin = (((y_data[i] - y_min) / y_range) * (grid_size - 1) as f64).round() as usize;
            let x_bin = x_bin.min(grid_size - 1);
            let y_bin = y_bin.min(grid_size - 1);

            hit_counts[y_bin][x_bin] += 1;
            if let Some(ref z) = z_data {
                let z_val = z[i];
                z_sums[y_bin][x_bin] += z_val;
                z_sum_sq[y_bin][x_bin] += z_val * z_val;
                z_mins[y_bin][x_bin] = z_mins[y_bin][x_bin].min(z_val);
                z_maxs[y_bin][x_bin] = z_maxs[y_bin][x_bin].max(z_val);
            }
        }

        // Calculate cell values and find min/max for color scaling
        let mut cell_values = vec![vec![None::<f64>; grid_size]; grid_size];
        let mut min_value: f64 = f64::MAX;
        let mut max_value: f64 = f64::MIN;

        for y_bin in 0..grid_size {
            for x_bin in 0..grid_size {
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
        let (full_rect, response) = ui.allocate_exact_size(chart_size, egui::Sense::click());

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

        let cell_width = plot_rect.width() / grid_size as f32;
        let cell_height = plot_rect.height() / grid_size as f32;

        // Get selected cell for highlighting
        let selected_cell = self.tabs[tab_idx]
            .histogram_state
            .config
            .selected_cell
            .clone();

        // Draw grid cells with values
        for y_bin in 0..grid_size {
            for x_bin in 0..grid_size {
                let cell_x = plot_rect.left() + x_bin as f32 * cell_width;
                let cell_y = plot_rect.bottom() - (y_bin + 1) as f32 * cell_height;
                let cell_rect = egui::Rect::from_min_size(
                    egui::pos2(cell_x, cell_y),
                    egui::vec2(cell_width, cell_height),
                );

                if let Some(value) = cell_values[y_bin][x_bin] {
                    // Normalize to 0-1 for color scaling
                    let normalized = if mode == HistogramMode::HitCount && max_value > 1.0 {
                        (value.ln() / max_value.ln()).clamp(0.0, 1.0)
                    } else {
                        ((value - min_value) / value_range).clamp(0.0, 1.0)
                    };
                    let color = Self::get_histogram_color(normalized);

                    painter.rect_filled(cell_rect, 0.0, color);

                    // Draw value text in center of cell (only if cell is large enough)
                    if cell_width > 25.0 && cell_height > 18.0 {
                        let text = if mode == HistogramMode::HitCount {
                            format!("{}", hit_counts[y_bin][x_bin])
                        } else {
                            format!("{:.1}", value)
                        };

                        // Choose text color for AAA contrast compliance
                        let text_color = get_aaa_text_color(color);

                        let base_font_size = if grid_size <= 16 {
                            11.0
                        } else if grid_size <= 32 {
                            9.0
                        } else {
                            7.0
                        };
                        let cell_font_size = self.scaled_font(base_font_size);

                        painter.text(
                            cell_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            text,
                            egui::FontId::proportional(cell_font_size),
                            text_color,
                        );
                    }
                }
            }
        }

        // Draw grid lines
        let grid_color = egui::Color32::from_rgb(60, 60, 60);
        for i in 0..=grid_size {
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

        // Get channel names for axis labels
        let x_channel_name = file.log.channels[x_idx].name();
        let y_channel_name = file.log.channels[y_idx].name();

        // Draw axis labels
        let text_color = egui::Color32::from_rgb(200, 200, 200);
        let axis_title_color = egui::Color32::from_rgb(255, 255, 255);

        // Y axis value labels
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let value = y_min + t * y_range;
            let y_pos = plot_rect.bottom() - t as f32 * plot_rect.height();
            painter.text(
                egui::pos2(plot_rect.left() - 8.0, y_pos),
                egui::Align2::RIGHT_CENTER,
                format!("{:.1}", value),
                egui::FontId::proportional(font_10),
                text_color,
            );
        }

        // Y axis title (rotated text simulation - draw vertically)
        let y_title_x = full_rect.left() + 12.0;
        let y_title_y = plot_rect.center().y;
        painter.text(
            egui::pos2(y_title_x, y_title_y),
            egui::Align2::CENTER_CENTER,
            &y_channel_name,
            egui::FontId::proportional(font_13),
            axis_title_color,
        );

        // X axis value labels
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let value = x_min + t * x_range;
            let x_pos = plot_rect.left() + t as f32 * plot_rect.width();
            painter.text(
                egui::pos2(x_pos, plot_rect.bottom() + 5.0),
                egui::Align2::CENTER_TOP,
                format!("{:.0}", value),
                egui::FontId::proportional(font_10),
                text_color,
            );
        }

        // X axis title
        let x_title_x = plot_rect.center().x;
        let x_title_y = full_rect.bottom() - 8.0;
        painter.text(
            egui::pos2(x_title_x, x_title_y),
            egui::Align2::CENTER_CENTER,
            &x_channel_name,
            egui::FontId::proportional(font_13),
            axis_title_color,
        );

        // Draw selected cell highlight
        if let Some(ref sel) = selected_cell {
            if sel.x_bin < grid_size && sel.y_bin < grid_size {
                let sel_x = plot_rect.left() + sel.x_bin as f32 * cell_width;
                let sel_y = plot_rect.bottom() - (sel.y_bin + 1) as f32 * cell_height;
                let sel_rect = egui::Rect::from_min_size(
                    egui::pos2(sel_x, sel_y),
                    egui::vec2(cell_width, cell_height),
                );
                let stroke = egui::Stroke::new(3.0, SELECTED_CELL_COLOR);
                painter.line_segment([sel_rect.left_top(), sel_rect.right_top()], stroke);
                painter.line_segment([sel_rect.left_bottom(), sel_rect.right_bottom()], stroke);
                painter.line_segment([sel_rect.left_top(), sel_rect.left_bottom()], stroke);
                painter.line_segment([sel_rect.right_top(), sel_rect.right_bottom()], stroke);
            }
        }

        // Draw current position indicator (cursor time)
        if let Some(cursor_record) = self.get_cursor_record() {
            if cursor_record < x_data.len() {
                let cursor_x = x_data[cursor_record];
                let cursor_y = y_data[cursor_record];

                let rel_x = ((cursor_x - x_min) / x_range) as f32;
                let rel_y = ((cursor_y - y_min) / y_range) as f32;

                if (0.0..=1.0).contains(&rel_x) && (0.0..=1.0).contains(&rel_y) {
                    let pos_x = plot_rect.left() + rel_x * plot_rect.width();
                    let pos_y = plot_rect.bottom() - rel_y * plot_rect.height();

                    // Draw grey crosshairs tracking the cursor position
                    painter.line_segment(
                        [
                            egui::pos2(pos_x, plot_rect.top()),
                            egui::pos2(pos_x, plot_rect.bottom()),
                        ],
                        egui::Stroke::new(1.0, CURSOR_CROSSHAIR_COLOR),
                    );
                    painter.line_segment(
                        [
                            egui::pos2(plot_rect.left(), pos_y),
                            egui::pos2(plot_rect.right(), pos_y),
                        ],
                        egui::Stroke::new(1.0, CURSOR_CROSSHAIR_COLOR),
                    );

                    // Highlight the cell containing the cursor
                    let cell_x_bin = (rel_x * (grid_size - 1) as f32).round() as usize;
                    let cell_y_bin = (rel_y * (grid_size - 1) as f32).round() as usize;
                    let cell_x_bin = cell_x_bin.min(grid_size - 1);
                    let cell_y_bin = cell_y_bin.min(grid_size - 1);

                    let highlight_x = plot_rect.left() + cell_x_bin as f32 * cell_width;
                    let highlight_y = plot_rect.bottom() - (cell_y_bin + 1) as f32 * cell_height;

                    let highlight_rect = egui::Rect::from_min_size(
                        egui::pos2(highlight_x, highlight_y),
                        egui::vec2(cell_width, cell_height),
                    );
                    let stroke = egui::Stroke::new(2.0, CELL_HIGHLIGHT_COLOR);
                    painter.line_segment(
                        [highlight_rect.left_top(), highlight_rect.right_top()],
                        stroke,
                    );
                    painter.line_segment(
                        [highlight_rect.left_bottom(), highlight_rect.right_bottom()],
                        stroke,
                    );
                    painter.line_segment(
                        [highlight_rect.left_top(), highlight_rect.left_bottom()],
                        stroke,
                    );
                    painter.line_segment(
                        [highlight_rect.right_top(), highlight_rect.right_bottom()],
                        stroke,
                    );

                    // Draw circle at exact position
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

                    let x_bin = (rel_x * (grid_size - 1) as f32).round() as usize;
                    let y_bin = (rel_y * (grid_size - 1) as f32).round() as usize;
                    let x_bin = x_bin.min(grid_size - 1);
                    let y_bin = y_bin.min(grid_size - 1);

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
                        egui::FontId::proportional(font_12),
                        egui::Color32::WHITE,
                    );
                }
            }
        }

        // Handle click to select cell
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                if plot_rect.contains(pos) {
                    let rel_x = (pos.x - plot_rect.left()) / plot_rect.width();
                    let rel_y = 1.0 - (pos.y - plot_rect.top()) / plot_rect.height();

                    if (0.0..=1.0).contains(&rel_x) && (0.0..=1.0).contains(&rel_y) {
                        let x_bin = (rel_x * (grid_size - 1) as f32).round() as usize;
                        let y_bin = (rel_y * (grid_size - 1) as f32).round() as usize;
                        let x_bin = x_bin.min(grid_size - 1);
                        let y_bin = y_bin.min(grid_size - 1);

                        let hits = hit_counts[y_bin][x_bin];

                        // Calculate cell value ranges
                        let bin_width_x = x_range / grid_size as f64;
                        let bin_width_y = y_range / grid_size as f64;
                        let cell_x_min = x_min + x_bin as f64 * bin_width_x;
                        let cell_x_max = cell_x_min + bin_width_x;
                        let cell_y_min = y_min + y_bin as f64 * bin_width_y;
                        let cell_y_max = cell_y_min + bin_width_y;

                        // Calculate statistics
                        let mean = if hits > 0 {
                            z_sums[y_bin][x_bin] / hits as f64
                        } else {
                            0.0
                        };

                        let variance = if hits > 1 {
                            let n = hits as f64;
                            (z_sum_sq[y_bin][x_bin] - (z_sums[y_bin][x_bin].powi(2) / n))
                                / (n - 1.0)
                        } else {
                            0.0
                        };

                        let std_dev = variance.sqrt();
                        let cell_weight = z_sums[y_bin][x_bin];

                        let minimum = if hits > 0 && z_mins[y_bin][x_bin] != f64::MAX {
                            z_mins[y_bin][x_bin]
                        } else {
                            0.0
                        };

                        let maximum = if hits > 0 && z_maxs[y_bin][x_bin] != f64::MIN {
                            z_maxs[y_bin][x_bin]
                        } else {
                            0.0
                        };

                        let selected = SelectedHistogramCell {
                            x_bin,
                            y_bin,
                            x_range: (cell_x_min, cell_x_max),
                            y_range: (cell_y_min, cell_y_max),
                            hit_count: hits,
                            cell_weight,
                            variance,
                            std_dev,
                            minimum,
                            mean,
                            maximum,
                        };

                        self.tabs[tab_idx].histogram_state.config.selected_cell = Some(selected);
                    }
                }
            }
        }

        // Render legend with selected cell info
        ui.add_space(8.0);
        let selected_cell = self.tabs[tab_idx]
            .histogram_state
            .config
            .selected_cell
            .as_ref();
        self.render_histogram_legend(ui, min_value, max_value, mode, selected_cell);
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

    /// Render the legend with color scale and cell statistics
    fn render_histogram_legend(
        &self,
        ui: &mut egui::Ui,
        min_value: f64,
        max_value: f64,
        mode: HistogramMode,
        selected_cell: Option<&SelectedHistogramCell>,
    ) {
        let font_12 = self.scaled_font(12.0);
        let font_13 = self.scaled_font(13.0);

        ui.horizontal(|ui| {
            ui.add_space(4.0);

            // Color scale legend
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220))
                .corner_radius(4)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let label = match mode {
                            HistogramMode::HitCount => "Hits:",
                            HistogramMode::AverageZ => "Value:",
                        };
                        ui.label(
                            egui::RichText::new(label)
                                .size(font_13)
                                .color(egui::Color32::WHITE),
                        );

                        // Color gradient bar
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(120.0, 18.0), egui::Sense::hover());

                        let painter = ui.painter();
                        let steps = 30;
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

                        ui.add_space(6.0);
                        let range_text = if mode == HistogramMode::HitCount {
                            format!("0-{:.0}", max_value)
                        } else {
                            format!("{:.1}-{:.1}", min_value, max_value)
                        };
                        ui.label(
                            egui::RichText::new(range_text)
                                .size(font_13)
                                .color(egui::Color32::WHITE),
                        );
                    });
                });

            ui.add_space(16.0);

            // Cell statistics panel (only shown when a cell is selected)
            if let Some(cell) = selected_cell {
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220))
                    .corner_radius(4)
                    .inner_margin(8.0)
                    .stroke(egui::Stroke::new(1.0, SELECTED_CELL_COLOR))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            // Cell identifier
                            ui.label(
                                egui::RichText::new(format!(
                                    "Cell [{}, {}]",
                                    cell.x_bin, cell.y_bin
                                ))
                                .size(font_13)
                                .color(SELECTED_CELL_COLOR),
                            );
                            ui.add_space(12.0);

                            // Key statistics inline
                            let stat_color = egui::Color32::from_rgb(180, 180, 180);
                            let value_color = egui::Color32::WHITE;

                            ui.label(egui::RichText::new("Hits:").size(font_12).color(stat_color));
                            ui.label(
                                egui::RichText::new(format!("{}", cell.hit_count))
                                    .size(font_12)
                                    .color(value_color),
                            );
                            ui.add_space(8.0);

                            ui.label(egui::RichText::new("Mean:").size(font_12).color(stat_color));
                            ui.label(
                                egui::RichText::new(format!("{:.2}", cell.mean))
                                    .size(font_12)
                                    .color(value_color),
                            );
                            ui.add_space(8.0);

                            ui.label(egui::RichText::new("Min:").size(font_12).color(stat_color));
                            ui.label(
                                egui::RichText::new(format!("{:.2}", cell.minimum))
                                    .size(font_12)
                                    .color(value_color),
                            );
                            ui.add_space(8.0);

                            ui.label(egui::RichText::new("Max:").size(font_12).color(stat_color));
                            ui.label(
                                egui::RichText::new(format!("{:.2}", cell.maximum))
                                    .size(font_12)
                                    .color(value_color),
                            );
                            ui.add_space(8.0);

                            ui.label(egui::RichText::new("σ:").size(font_12).color(stat_color));
                            ui.label(
                                egui::RichText::new(format!("{:.2}", cell.std_dev))
                                    .size(font_12)
                                    .color(value_color),
                            );
                        });
                    });
            } else {
                // Hint when no cell is selected
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220))
                    .corner_radius(4)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Click a cell to view statistics")
                                .size(font_12)
                                .italics()
                                .color(egui::Color32::GRAY),
                        );
                    });
            }
        });
    }

    /// Render histogram cell statistics in sidebar
    pub fn render_histogram_stats(&self, ui: &mut egui::Ui) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };

        let selected = &self.tabs[tab_idx].histogram_state.config.selected_cell;

        // Pre-compute scaled font sizes
        let font_12 = self.scaled_font(12.0);
        let font_13 = self.scaled_font(13.0);
        let font_14 = self.scaled_font(14.0);

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(5.0);

        ui.label(
            egui::RichText::new("Cell Statistics")
                .size(font_14)
                .strong()
                .color(egui::Color32::WHITE),
        );

        ui.add_space(5.0);

        if let Some(cell) = selected {
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 200))
                .corner_radius(4)
                .inner_margin(8.0)
                .stroke(egui::Stroke::new(1.0, SELECTED_CELL_COLOR))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Cell [{}, {}]", cell.x_bin, cell.y_bin))
                                .size(font_13)
                                .color(SELECTED_CELL_COLOR),
                        );
                    });

                    ui.add_space(4.0);

                    let stats = [
                        (
                            "X Range",
                            format!("{:.2} - {:.2}", cell.x_range.0, cell.x_range.1),
                        ),
                        (
                            "Y Range",
                            format!("{:.2} - {:.2}", cell.y_range.0, cell.y_range.1),
                        ),
                        ("Hit Count", format!("{}", cell.hit_count)),
                        ("Cell Weight", format!("{:.4}", cell.cell_weight)),
                        ("Mean", format!("{:.4}", cell.mean)),
                        ("Minimum", format!("{:.4}", cell.minimum)),
                        ("Maximum", format!("{:.4}", cell.maximum)),
                        ("Variance", format!("{:.4}", cell.variance)),
                        ("Std Dev", format!("{:.4}", cell.std_dev)),
                    ];

                    for (label, value) in stats {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{}:", label))
                                    .size(font_12)
                                    .color(egui::Color32::from_rgb(180, 180, 180)),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(&value)
                                            .size(font_12)
                                            .color(egui::Color32::WHITE),
                                    );
                                },
                            );
                        });
                    }

                    ui.add_space(4.0);

                    if ui.small_button("Clear Selection").clicked() {
                        // We can't mutate here, set a flag instead
                    }
                });
        } else {
            ui.label(
                egui::RichText::new("Click a cell to view statistics")
                    .size(font_12)
                    .italics()
                    .color(egui::Color32::GRAY),
            );
        }
    }
}
