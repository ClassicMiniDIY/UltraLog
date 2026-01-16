//! Histogram / 2D heatmap view for analyzing channel distributions.
//!
//! This module provides a histogram view where users can visualize
//! relationships between channels as a 2D grid with configurable
//! cell coloring based on average Z-value or hit count.

use eframe::egui;
use rust_i18n::t;

use crate::app::UltraLogApp;
use crate::normalize::sort_channels_by_priority;
use crate::state::{
    HistogramMode, PastedTable, SampleFilter, SelectedHistogramCell, TableOperation,
};

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
const AXIS_LABEL_MARGIN_TOP: f32 = 10.0;
const AXIS_LABEL_MARGIN_RIGHT: f32 = 25.0;

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

/// Maximum length for axis labels before truncation
const MAX_AXIS_LABEL_LENGTH: usize = 20;

/// Calculate which bin a normalized value (0.0 to 1.0) falls into
/// Uses floor-based calculation for consistent cell boundaries
#[inline]
fn calculate_bin(normalized: f32, grid_size: usize) -> usize {
    ((normalized * grid_size as f32).floor() as usize).min(grid_size - 1)
}

/// Calculate which bin a data value falls into given the data range
#[inline]
fn calculate_data_bin(value: f64, min: f64, range: f64, grid_size: usize) -> usize {
    let normalized = ((value - min) / range) as f32;
    calculate_bin(normalized.clamp(0.0, 1.0), grid_size)
}

/// Truncate a string to max length with ellipsis
fn truncate_label(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}

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

        // Controls are now in the sidebar (tool properties panel)
        // Render the histogram grid
        self.render_histogram_grid(ui);
    }

    /// Render the control panel with axis selectors, mode toggle, and grid size
    /// This is now called from the tool properties sidebar panel
    pub(crate) fn render_histogram_controls(&mut self, ui: &mut egui::Ui) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };
        let file_idx = self.tabs[tab_idx].file_index;

        if file_idx >= self.files.len() {
            return;
        }

        let file = &self.files[file_idx];
        let base_channel_count = file.log.channels.len();

        // Sort channels for dropdown (including computed channels)
        let mut sorted_channels = sort_channels_by_priority(
            base_channel_count,
            |idx| file.log.channels[idx].name(),
            self.field_normalization,
            Some(&self.custom_normalizations),
        );

        // Add computed channels to the dropdown
        if let Some(computed_channels) = self.file_computed_channels.get(&file_idx) {
            for (computed_idx, computed) in computed_channels.iter().enumerate() {
                let channel_idx = base_channel_count + computed_idx;
                // Add with is_priority=false (computed channels listed after prioritized channels)
                sorted_channels.push((
                    channel_idx,
                    format!("📊 {}", &computed.template.name),
                    false,
                ));
            }
        }

        // Get current selections
        let config = &self.tabs[tab_idx].histogram_state.config;
        let current_x = config.x_channel;
        let current_y = config.y_channel;
        let current_z = config.z_channel;
        let current_mode = config.mode;
        let current_grid_size = config.grid_size;
        let current_custom_grid_cols = config.custom_grid_columns;
        let current_custom_grid_rows = config.custom_grid_rows;
        let current_min_hits = config.min_hits_filter;
        let current_custom_x = config.custom_x_range;
        let current_custom_y = config.custom_y_range;
        let has_pasted_table = config.pasted_table.is_some();
        let current_table_op = config.table_operation;
        let current_show_compare = config.show_comparison_view;

        // Calculate dynamic data bounds for use as defaults when unchecking Auto
        let (dynamic_x_min, dynamic_x_max) = if let Some(x_idx) = current_x {
            let x_data = self.get_channel_data(file_idx, x_idx);
            if !x_data.is_empty() {
                let min = x_data.iter().cloned().fold(f64::MAX, f64::min);
                let max = x_data.iter().cloned().fold(f64::MIN, f64::max);
                (min, max)
            } else {
                (0.0, 100.0)
            }
        } else {
            (0.0, 100.0)
        };
        let (dynamic_y_min, dynamic_y_max) = if let Some(y_idx) = current_y {
            let y_data = self.get_channel_data(file_idx, y_idx);
            if !y_data.is_empty() {
                let min = y_data.iter().cloned().fold(f64::MAX, f64::min);
                let max = y_data.iter().cloned().fold(f64::MIN, f64::max);
                (min, max)
            } else {
                (0.0, 100.0)
            }
        } else {
            (0.0, 100.0)
        };

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
        let mut new_custom_grid_cols: Option<usize> = None;
        let mut new_custom_grid_rows: Option<usize> = None;
        let mut new_min_hits: Option<u32> = None;
        let mut new_custom_x: Option<Option<(f64, f64)>> = None;
        let mut new_custom_y: Option<Option<(f64, f64)>> = None;
        let mut new_table_op: Option<TableOperation> = None;
        let mut new_show_compare: Option<bool> = None;
        let mut clear_pasted_table = false;
        let mut do_paste = false;
        let mut sample_filter_updates: Vec<(usize, SampleFilter)> = Vec::new();
        let mut sample_filters_to_remove: Vec<usize> = Vec::new();
        let mut new_sample_filter: Option<SampleFilter> = None;

        // Pre-compute scaled font sizes
        let font_14 = self.scaled_font(14.0);
        let font_15 = self.scaled_font(15.0);

        // ============================================================================
        // AXES SECTION
        // ============================================================================
        ui.label(egui::RichText::new("📊 Axes").size(font_15).strong());
        ui.add_space(8.0);

        // X Axis
        ui.label(egui::RichText::new(t!("histogram.x_axis")).size(font_14));
        egui::ComboBox::from_id_salt("histogram_x")
            .selected_text(
                current_x
                    .and_then(|i| channel_names.get(&i).map(|n| n.as_str()))
                    .unwrap_or("Select..."),
            )
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (idx, name, _) in &sorted_channels {
                    if ui.selectable_label(current_x == Some(*idx), name).clicked() {
                        new_x = Some(*idx);
                    }
                }
            });

        ui.add_space(8.0);

        // Y Axis
        ui.label(egui::RichText::new(t!("histogram.y_axis")).size(font_14));
        egui::ComboBox::from_id_salt("histogram_y")
            .selected_text(
                current_y
                    .and_then(|i| channel_names.get(&i).map(|n| n.as_str()))
                    .unwrap_or("Select..."),
            )
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for (idx, name, _) in &sorted_channels {
                    if ui.selectable_label(current_y == Some(*idx), name).clicked() {
                        new_y = Some(*idx);
                    }
                }
            });

        ui.add_space(8.0);

        // Z Axis (only for Average Z mode)
        let z_enabled = current_mode == HistogramMode::AverageZ;
        ui.add_enabled_ui(z_enabled, |ui| {
            ui.label(
                egui::RichText::new(t!("histogram.z_axis"))
                    .size(font_14)
                    .color(if z_enabled {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::GRAY
                    }),
            );
            egui::ComboBox::from_id_salt("histogram_z")
                .selected_text(
                    current_z
                        .and_then(|i| channel_names.get(&i).map(|n| n.as_str()))
                        .unwrap_or("Select..."),
                )
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (idx, name, _) in &sorted_channels {
                        if ui.selectable_label(current_z == Some(*idx), name).clicked() {
                            new_z = Some(*idx);
                        }
                    }
                });
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);

        // ============================================================================
        // DISPLAY OPTIONS SECTION
        // ============================================================================
        ui.label(egui::RichText::new("⚙ Display").size(font_15).strong());
        ui.add_space(8.0);

        // Mode selector
        ui.label(egui::RichText::new(t!("histogram.mode")).size(font_14));
        ui.horizontal(|ui| {
            if ui
                .selectable_label(
                    current_mode == HistogramMode::HitCount,
                    t!("histogram.hit_count"),
                )
                .clicked()
            {
                new_mode = Some(HistogramMode::HitCount);
            }
            if ui
                .selectable_label(
                    current_mode == HistogramMode::AverageZ,
                    t!("histogram.average_z"),
                )
                .clicked()
            {
                new_mode = Some(HistogramMode::AverageZ);
            }
        });

        ui.add_space(8.0);

        // Grid columns
        ui.label(egui::RichText::new("Columns (X bins)").size(font_14));
        let effective_cols = if current_custom_grid_cols > 0 {
            current_custom_grid_cols
        } else {
            current_grid_size.size()
        };
        let mut cols_value = effective_cols as i32;
        ui.add(
            egui::DragValue::new(&mut cols_value)
                .range(4..=256)
                .speed(1.0),
        );
        if cols_value != effective_cols as i32 {
            new_custom_grid_cols = Some(cols_value.clamp(4, 256) as usize);
        }

        ui.add_space(8.0);

        // Grid rows
        ui.label(egui::RichText::new("Rows (Y bins)").size(font_14));
        let effective_rows = if current_custom_grid_rows > 0 {
            current_custom_grid_rows
        } else {
            current_grid_size.size()
        };
        let mut rows_value = effective_rows as i32;
        ui.add(
            egui::DragValue::new(&mut rows_value)
                .range(4..=256)
                .speed(1.0),
        );
        if rows_value != effective_rows as i32 {
            new_custom_grid_rows = Some(rows_value.clamp(4, 256) as usize);
        }

        ui.add_space(8.0);

        // Min hits filter
        ui.label(egui::RichText::new(t!("histogram.min_hits")).size(font_14));
        let mut min_hits_value = current_min_hits as i32;
        ui.add(
            egui::DragValue::new(&mut min_hits_value)
                .range(0..=1000)
                .speed(1.0),
        );
        if min_hits_value != current_min_hits as i32 {
            new_min_hits = Some(min_hits_value.max(0) as u32);
        }

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);

        // ============================================================================
        // DATA RANGE SECTION
        // ============================================================================
        ui.label(egui::RichText::new("📏 Data Range").size(font_15).strong());
        ui.add_space(8.0);

        // X Range
        ui.label(egui::RichText::new(t!("histogram.x_range")).size(font_14));
        let mut x_auto = current_custom_x.is_none();
        if ui.checkbox(&mut x_auto, t!("histogram.auto")).changed() {
            if x_auto {
                new_custom_x = Some(None);
            } else {
                new_custom_x = Some(Some((dynamic_x_min, dynamic_x_max)));
            }
        }
        if !x_auto {
            let (mut x_min, mut x_max) = current_custom_x.unwrap_or((dynamic_x_min, dynamic_x_max));
            ui.horizontal(|ui| {
                ui.label("Min:");
                ui.add(egui::DragValue::new(&mut x_min).speed(1.0));
                ui.label("Max:");
                ui.add(egui::DragValue::new(&mut x_max).speed(1.0));
            });
            if x_max > x_min {
                new_custom_x = Some(Some((x_min, x_max)));
            }
        }

        ui.add_space(8.0);

        // Y Range
        ui.label(egui::RichText::new(t!("histogram.y_range")).size(font_14));
        let mut y_auto = current_custom_y.is_none();
        if ui.checkbox(&mut y_auto, t!("histogram.auto")).changed() {
            if y_auto {
                new_custom_y = Some(None);
            } else {
                new_custom_y = Some(Some((dynamic_y_min, dynamic_y_max)));
            }
        }
        if !y_auto {
            let (mut y_min, mut y_max) = current_custom_y.unwrap_or((dynamic_y_min, dynamic_y_max));
            ui.horizontal(|ui| {
                ui.label("Min:");
                ui.add(egui::DragValue::new(&mut y_min).speed(1.0));
                ui.label("Max:");
                ui.add(egui::DragValue::new(&mut y_max).speed(1.0));
            });
            if y_max > y_min {
                new_custom_y = Some(Some((y_min, y_max)));
            }
        }

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);

        // ============================================================================
        // SAMPLE FILTERS SECTION
        // ============================================================================
        let current_sample_filters = config.sample_filters.clone();
        egui::CollapsingHeader::new(
            egui::RichText::new(format!("🔍 {}", t!("histogram.sample_filters"))).size(font_15),
        )
        .default_open(false)
        .show(ui, |ui| {
            ui.add_space(4.0);

            if current_sample_filters.is_empty() {
                ui.label(
                    egui::RichText::new(t!("histogram.no_filters"))
                        .size(font_14)
                        .color(egui::Color32::GRAY),
                );
            } else {
                for (filter_idx, filter) in current_sample_filters.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let mut enabled = filter.enabled;
                        if ui.checkbox(&mut enabled, "").changed() {
                            let mut updated_filter = filter.clone();
                            updated_filter.enabled = enabled;
                            sample_filter_updates.push((filter_idx, updated_filter));
                        }

                        ui.label(
                            egui::RichText::new(&filter.channel_name)
                                .size(font_14)
                                .color(if enabled {
                                    egui::Color32::WHITE
                                } else {
                                    egui::Color32::GRAY
                                }),
                        );

                        if ui
                            .button(egui::RichText::new("❌").size(font_14))
                            .on_hover_text(t!("histogram.remove_filter"))
                            .clicked()
                        {
                            sample_filters_to_remove.push(filter_idx);
                        }
                    });

                    // Min/Max on separate row for cleaner layout
                    if filter.enabled {
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            let mut min_val = filter.min_value.unwrap_or(f64::NEG_INFINITY);
                            let mut use_min = filter.min_value.is_some();
                            if ui
                                .checkbox(&mut use_min, t!("histogram.filter_min"))
                                .changed()
                                || (use_min
                                    && ui
                                        .add(
                                            egui::DragValue::new(&mut min_val)
                                                .speed(1.0)
                                                .range(f64::NEG_INFINITY..=f64::INFINITY),
                                        )
                                        .changed())
                            {
                                let mut updated_filter = filter.clone();
                                updated_filter.min_value =
                                    if use_min { Some(min_val) } else { None };
                                sample_filter_updates.push((filter_idx, updated_filter));
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            let mut max_val = filter.max_value.unwrap_or(f64::INFINITY);
                            let mut use_max = filter.max_value.is_some();
                            if ui
                                .checkbox(&mut use_max, t!("histogram.filter_max"))
                                .changed()
                                || (use_max
                                    && ui
                                        .add(
                                            egui::DragValue::new(&mut max_val)
                                                .speed(1.0)
                                                .range(f64::NEG_INFINITY..=f64::INFINITY),
                                        )
                                        .changed())
                            {
                                let mut updated_filter = filter.clone();
                                updated_filter.max_value =
                                    if use_max { Some(max_val) } else { None };
                                sample_filter_updates.push((filter_idx, updated_filter));
                            }
                        });
                    }

                    ui.add_space(4.0);
                }
            }

            ui.add_space(8.0);

            // Add filter
            ui.label(egui::RichText::new(t!("histogram.add_filter")).size(font_14));
            egui::ComboBox::from_id_salt("add_sample_filter")
                .selected_text(t!("histogram.filter_channel"))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    for (idx, name, _) in &sorted_channels {
                        if ui.selectable_label(false, name).clicked() {
                            new_sample_filter = Some(SampleFilter::new(*idx, name.clone()));
                        }
                    }
                });
        });

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(16.0);

        // ============================================================================
        // TABLE OPERATIONS SECTION
        // ============================================================================
        ui.label(
            egui::RichText::new("📋 Table Operations")
                .size(font_15)
                .strong(),
        );
        ui.add_space(8.0);

        // Copy/Paste buttons
        ui.horizontal(|ui| {
            if ui.button(format!("📋 {}", t!("histogram.copy"))).clicked() {
                self.copy_histogram_to_clipboard(tab_idx);
            }
            if ui.button(format!("📥 {}", t!("histogram.paste"))).clicked() {
                do_paste = true;
            }
        });

        // Comparison controls (shown when table is pasted)
        if has_pasted_table {
            ui.add_space(12.0);

            ui.label(egui::RichText::new(t!("histogram.operation")).size(font_14));
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(current_table_op == TableOperation::Add, "+")
                    .clicked()
                {
                    new_table_op = Some(TableOperation::Add);
                }
                if ui
                    .selectable_label(current_table_op == TableOperation::Subtract, "−")
                    .clicked()
                {
                    new_table_op = Some(TableOperation::Subtract);
                }
                if ui
                    .selectable_label(current_table_op == TableOperation::Multiply, "×")
                    .clicked()
                {
                    new_table_op = Some(TableOperation::Multiply);
                }
                if ui
                    .selectable_label(current_table_op == TableOperation::Divide, "÷")
                    .clicked()
                {
                    new_table_op = Some(TableOperation::Divide);
                }
            });

            ui.add_space(8.0);

            let mut show_compare = current_show_compare;
            if ui
                .checkbox(&mut show_compare, t!("histogram.compare"))
                .changed()
            {
                new_show_compare = Some(show_compare);
            }

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.button(format!("❌ {}", t!("histogram.clear"))).clicked() {
                    clear_pasted_table = true;
                }
                if ui
                    .button(format!("📋 {}", t!("histogram.copy_result")))
                    .clicked()
                {
                    self.copy_result_to_clipboard(tab_idx);
                }
            });
        }
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
        if let Some(cols) = new_custom_grid_cols {
            config.custom_grid_columns = cols;
            config.selected_cell = None;
        }
        if let Some(rows) = new_custom_grid_rows {
            config.custom_grid_rows = rows;
            config.selected_cell = None;
        }
        if let Some(min_hits) = new_min_hits {
            config.min_hits_filter = min_hits;
        }
        if let Some(range) = new_custom_x {
            config.custom_x_range = range;
            config.selected_cell = None;
        }
        if let Some(range) = new_custom_y {
            config.custom_y_range = range;
            config.selected_cell = None;
        }
        if let Some(op) = new_table_op {
            config.table_operation = op;
        }
        if let Some(show) = new_show_compare {
            config.show_comparison_view = show;
        }
        if clear_pasted_table {
            config.pasted_table = None;
            config.show_comparison_view = false;
        }

        // Apply sample filter updates
        for (idx, updated_filter) in sample_filter_updates {
            if idx < config.sample_filters.len() {
                config.sample_filters[idx] = updated_filter;
            }
        }

        // Remove filters (in reverse order to preserve indices)
        for idx in sample_filters_to_remove.iter().rev() {
            if *idx < config.sample_filters.len() {
                config.sample_filters.remove(*idx);
            }
        }

        // Add new filter
        if let Some(filter) = new_sample_filter {
            config.sample_filters.push(filter);
        }

        // Handle paste after all config updates are done
        if do_paste {
            self.paste_table_from_clipboard(tab_idx);
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
        let (grid_cols, grid_rows) = config.effective_grid_size();
        let min_hits_filter = config.min_hits_filter;
        let custom_x_range = config.custom_x_range;
        let custom_y_range = config.custom_y_range;
        let sample_filters = config.sample_filters.clone();
        let show_comparison = config.show_comparison_view && config.pasted_table.is_some();
        let pasted_table = config.pasted_table.clone();
        let table_operation = config.table_operation;

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
        let x_data = self.get_channel_data(file_idx, x_idx);
        let y_data = self.get_channel_data(file_idx, y_idx);
        let z_data = z_idx.map(|z| self.get_channel_data(file_idx, z));

        if x_data.is_empty() || y_data.is_empty() || x_data.len() != y_data.len() {
            return;
        }

        // Calculate data bounds (use custom ranges if set, otherwise auto from data)
        let (x_min, x_max) = match custom_x_range {
            Some((min, max)) if max > min => (min, max),
            _ => {
                let min = x_data.iter().cloned().fold(f64::MAX, f64::min);
                let max = x_data.iter().cloned().fold(f64::MIN, f64::max);
                (min, max)
            }
        };
        let (y_min, y_max) = match custom_y_range {
            Some((min, max)) if max > min => (min, max),
            _ => {
                let min = y_data.iter().cloned().fold(f64::MAX, f64::min);
                let max = y_data.iter().cloned().fold(f64::MIN, f64::max);
                (min, max)
            }
        };

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

        // Pre-fetch filter channel data for efficiency
        let filter_data: Vec<(&SampleFilter, Vec<f64>)> = sample_filters
            .iter()
            .filter(|f| f.enabled)
            .map(|f| (f, self.get_channel_data(file_idx, f.channel_idx)))
            .collect();

        // Build histogram grid with full statistics
        // Grid is [row][col] = [y][x]
        let mut hit_counts = vec![vec![0u32; grid_cols]; grid_rows];
        let mut z_sums = vec![vec![0.0f64; grid_cols]; grid_rows];
        let mut z_sum_sq = vec![vec![0.0f64; grid_cols]; grid_rows];
        let mut z_mins = vec![vec![f64::MAX; grid_cols]; grid_rows];
        let mut z_maxs = vec![vec![f64::MIN; grid_cols]; grid_rows];

        'sample_loop: for i in 0..x_data.len() {
            // Check all sample filters (AND logic)
            for (filter, data) in &filter_data {
                if i >= data.len() {
                    continue 'sample_loop;
                }
                let val = data[i];
                if let Some(min) = filter.min_value {
                    if val < min {
                        continue 'sample_loop;
                    }
                }
                if let Some(max) = filter.max_value {
                    if val > max {
                        continue 'sample_loop;
                    }
                }
            }

            // Skip samples outside custom range (if set)
            if custom_x_range.is_some() && (x_data[i] < x_min || x_data[i] > x_max) {
                continue;
            }
            if custom_y_range.is_some() && (y_data[i] < y_min || y_data[i] > y_max) {
                continue;
            }

            let x_bin = calculate_data_bin(x_data[i], x_min, x_range, grid_cols);
            let y_bin = calculate_data_bin(y_data[i], y_min, y_range, grid_rows);

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
        let mut cell_values = vec![vec![None::<f64>; grid_cols]; grid_rows];
        let mut min_value: f64 = f64::MAX;
        let mut max_value: f64 = f64::MIN;

        for y_bin in 0..grid_rows {
            for x_bin in 0..grid_cols {
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

        // Comparison view: show Histogram, Pasted Table, and Result side-by-side
        if show_comparison {
            if let Some(ref pasted) = pasted_table {
                // Resample pasted table to match grid size
                let resampled = Self::resample_table(pasted, grid_cols, grid_rows);
                let pasted_values: Vec<Vec<Option<f64>>> = resampled
                    .iter()
                    .map(|row| row.iter().map(|&v| Some(v)).collect())
                    .collect();

                // Calculate result by applying operation
                let result_values =
                    Self::apply_table_operation(&cell_values, &resampled, table_operation);

                // Find min/max for pasted and result
                let mut pasted_min = f64::MAX;
                let mut pasted_max = f64::MIN;
                for row in &resampled {
                    for &v in row {
                        pasted_min = pasted_min.min(v);
                        pasted_max = pasted_max.max(v);
                    }
                }

                let mut result_min = f64::MAX;
                let mut result_max = f64::MIN;
                for row in &result_values {
                    for val in row.iter().flatten() {
                        result_min = result_min.min(*val);
                        result_max = result_max.max(*val);
                    }
                }

                // Allocate space for comparison view
                let available = ui.available_size();
                let chart_height = (available.y - LEGEND_HEIGHT - 40.0).max(200.0);
                let panel_width = (available.x - 20.0) / 3.0;

                let (full_rect, _response) = ui.allocate_exact_size(
                    egui::vec2(available.x, chart_height + 30.0),
                    egui::Sense::hover(),
                );

                let painter = ui.painter_at(full_rect);

                // Draw three panels
                let panel1_rect = egui::Rect::from_min_size(
                    full_rect.min,
                    egui::vec2(panel_width, chart_height + 30.0),
                );
                let panel2_rect = egui::Rect::from_min_size(
                    egui::pos2(full_rect.left() + panel_width + 10.0, full_rect.top()),
                    egui::vec2(panel_width, chart_height + 30.0),
                );
                let panel3_rect = egui::Rect::from_min_size(
                    egui::pos2(
                        full_rect.left() + 2.0 * (panel_width + 10.0),
                        full_rect.top(),
                    ),
                    egui::vec2(panel_width, chart_height + 30.0),
                );

                // Render the three panels
                Self::render_mini_heat_map(
                    &painter,
                    panel1_rect,
                    &cell_values,
                    min_value,
                    max_value,
                    &t!("histogram.comparison_histogram"),
                    font_13,
                    Some(&hit_counts),
                    min_hits_filter,
                );

                Self::render_mini_heat_map(
                    &painter,
                    panel2_rect,
                    &pasted_values,
                    pasted_min,
                    pasted_max,
                    &t!("histogram.comparison_pasted"),
                    font_13,
                    None,
                    0,
                );

                let op_symbol = match table_operation {
                    TableOperation::Add => "+",
                    TableOperation::Subtract => "−",
                    TableOperation::Multiply => "×",
                    TableOperation::Divide => "÷",
                };
                Self::render_mini_heat_map(
                    &painter,
                    panel3_rect,
                    &result_values,
                    result_min,
                    result_max,
                    &format!("{} ({})", t!("histogram.comparison_result"), op_symbol),
                    font_13,
                    None,
                    0,
                );

                // Render legend with comparison info
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {:.1} to {:.1}",
                            t!("histogram.comparison_histogram"),
                            min_value,
                            max_value
                        ))
                        .size(font_12)
                        .color(egui::Color32::WHITE),
                    );
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {:.1} to {:.1}",
                            t!("histogram.comparison_pasted"),
                            pasted_min,
                            pasted_max
                        ))
                        .size(font_12)
                        .color(egui::Color32::WHITE),
                    );
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "{}: {:.1} to {:.1}",
                            t!("histogram.comparison_result"),
                            result_min,
                            result_max
                        ))
                        .size(font_12)
                        .color(egui::Color32::WHITE),
                    );
                });

                return;
            }
        }

        // Allocate space for the grid
        let available = ui.available_size();
        let chart_size = egui::vec2(available.x, (available.y - LEGEND_HEIGHT).max(200.0));
        let (full_rect, response) = ui.allocate_exact_size(chart_size, egui::Sense::click());

        // Create inner plot rect with margins
        let plot_rect = egui::Rect::from_min_max(
            egui::pos2(
                full_rect.left() + AXIS_LABEL_MARGIN_LEFT,
                full_rect.top() + AXIS_LABEL_MARGIN_TOP,
            ),
            egui::pos2(
                full_rect.right() - AXIS_LABEL_MARGIN_RIGHT,
                full_rect.bottom() - AXIS_LABEL_MARGIN_BOTTOM,
            ),
        );

        let painter = ui.painter_at(full_rect);
        painter.rect_filled(plot_rect, 0.0, egui::Color32::BLACK);

        let cell_width = plot_rect.width() / grid_cols as f32;
        let cell_height = plot_rect.height() / grid_rows as f32;

        // Get selected cell for highlighting
        let selected_cell = self.tabs[tab_idx]
            .histogram_state
            .config
            .selected_cell
            .clone();

        // Draw grid cells with values
        for y_bin in 0..grid_rows {
            for x_bin in 0..grid_cols {
                let cell_x = plot_rect.left() + x_bin as f32 * cell_width;
                let cell_y = plot_rect.bottom() - (y_bin + 1) as f32 * cell_height;
                let cell_rect = egui::Rect::from_min_size(
                    egui::pos2(cell_x, cell_y),
                    egui::vec2(cell_width, cell_height),
                );

                let hits = hit_counts[y_bin][x_bin];

                // Skip cells below minimum hits threshold (draw grayed out)
                if min_hits_filter > 0 && hits < min_hits_filter {
                    painter.rect_filled(cell_rect, 0.0, egui::Color32::from_rgb(30, 30, 30));
                    continue;
                }

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

                        let max_dim = grid_cols.max(grid_rows);
                        let base_font_size = if max_dim <= 16 {
                            11.0
                        } else if max_dim <= 32 {
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
        // Vertical lines (X axis divisions)
        for i in 0..=grid_cols {
            let x = plot_rect.left() + i as f32 * cell_width;
            painter.line_segment(
                [
                    egui::pos2(x, plot_rect.top()),
                    egui::pos2(x, plot_rect.bottom()),
                ],
                egui::Stroke::new(0.5, grid_color),
            );
        }
        // Horizontal lines (Y axis divisions)
        for i in 0..=grid_rows {
            let y = plot_rect.top() + i as f32 * cell_height;
            painter.line_segment(
                [
                    egui::pos2(plot_rect.left(), y),
                    egui::pos2(plot_rect.right(), y),
                ],
                egui::Stroke::new(0.5, grid_color),
            );
        }

        // Get channel names for axis labels (handles computed channels)
        let base_channel_count = file.log.channels.len();
        let x_channel_name = if x_idx < base_channel_count {
            file.log.channels[x_idx].name()
        } else {
            let computed_idx = x_idx - base_channel_count;
            self.file_computed_channels
                .get(&file_idx)
                .and_then(|c| c.get(computed_idx))
                .map(|c| c.template.name.clone())
                .unwrap_or_else(|| "Unknown".to_string())
        };
        let y_channel_name = if y_idx < base_channel_count {
            file.log.channels[y_idx].name()
        } else {
            let computed_idx = y_idx - base_channel_count;
            self.file_computed_channels
                .get(&file_idx)
                .and_then(|c| c.get(computed_idx))
                .map(|c| c.template.name.clone())
                .unwrap_or_else(|| "Unknown".to_string())
        };

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

        // Y axis title (wrapped vertically on word boundaries)
        let y_title_x = full_rect.left() + 12.0;
        let y_title_center_y = plot_rect.center().y;

        // Wrap long names on word boundaries for vertical display
        let y_title_display = if y_channel_name.len() > MAX_AXIS_LABEL_LENGTH {
            // Try to wrap on spaces, showing up to 2 lines
            let words: Vec<&str> = y_channel_name.split_whitespace().collect();
            let mut lines = Vec::new();
            let mut current_line = String::new();

            for word in words {
                if current_line.is_empty() {
                    current_line = word.to_string();
                } else if current_line.len() + 1 + word.len() <= MAX_AXIS_LABEL_LENGTH {
                    current_line.push(' ');
                    current_line.push_str(word);
                } else {
                    lines.push(current_line);
                    current_line = word.to_string();
                    if lines.len() >= 2 {
                        // Truncate if more than 2 lines
                        current_line = truncate_label(&current_line, MAX_AXIS_LABEL_LENGTH);
                        break;
                    }
                }
            }
            if !current_line.is_empty() {
                lines.push(current_line);
            }
            lines.join("\n")
        } else {
            y_channel_name.clone()
        };

        // Calculate text height for multi-line labels
        let line_count = y_title_display.lines().count() as f32;
        let line_height = font_13 * 1.2;
        let total_height = line_count * line_height;
        let y_start = y_title_center_y - total_height / 2.0 + line_height / 2.0;

        // Draw each line centered
        let mut combined_rect: Option<egui::Rect> = None;
        for (i, line) in y_title_display.lines().enumerate() {
            let line_y = y_start + i as f32 * line_height;
            let rect = painter.text(
                egui::pos2(y_title_x, line_y),
                egui::Align2::CENTER_CENTER,
                line,
                egui::FontId::proportional(font_13),
                axis_title_color,
            );
            combined_rect = Some(combined_rect.map_or(rect, |r| r.union(rect)));
        }

        // Show full name as tooltip if wrapped or truncated
        if y_title_display != y_channel_name || y_title_display.contains('\n') {
            if let Some(rect) = combined_rect {
                let y_title_response =
                    ui.interact(rect, ui.id().with("y_title_tooltip"), egui::Sense::hover());
                y_title_response.on_hover_text(&y_channel_name);
            }
        }

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

        // X axis title (truncate long names for consistency)
        let x_title_x = plot_rect.center().x;
        let x_title_y = full_rect.bottom() - 8.0;
        let x_title_truncated = truncate_label(&x_channel_name, MAX_AXIS_LABEL_LENGTH);
        let x_title_rect = painter.text(
            egui::pos2(x_title_x, x_title_y),
            egui::Align2::CENTER_CENTER,
            &x_title_truncated,
            egui::FontId::proportional(font_13),
            axis_title_color,
        );
        // Show full name as tooltip if truncated
        if x_title_truncated != x_channel_name {
            let x_title_response = ui.interact(
                x_title_rect,
                ui.id().with("x_title_tooltip"),
                egui::Sense::hover(),
            );
            x_title_response.on_hover_text(&x_channel_name);
        }

        // Draw selected cell highlight
        if let Some(ref sel) = selected_cell {
            if sel.x_bin < grid_cols && sel.y_bin < grid_rows {
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
                    let cell_x_bin = calculate_bin(rel_x, grid_cols);
                    let cell_y_bin = calculate_bin(rel_y, grid_rows);

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

                    let x_bin = calculate_bin(rel_x, grid_cols);
                    let y_bin = calculate_bin(rel_y, grid_rows);

                    let hits = hit_counts[y_bin][x_bin];
                    let cell_value = cell_values[y_bin][x_bin];

                    // Truncate channel names for tooltip display
                    let x_label = truncate_label(&x_channel_name, 10);
                    let y_label = truncate_label(&y_channel_name, 10);

                    let tooltip_text = match mode {
                        HistogramMode::HitCount => {
                            format!(
                                "{}: {:.1}\n{}: {:.1}\nHits: {}",
                                x_label, x_val, y_label, y_val, hits
                            )
                        }
                        HistogramMode::AverageZ => {
                            let avg = cell_value
                                .map(|v| format!("{:.2}", v))
                                .unwrap_or("-".to_string());
                            format!(
                                "{}: {:.1}\n{}: {:.1}\nAvg: {}\nHits: {}",
                                x_label, x_val, y_label, y_val, avg, hits
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
                        let x_bin = calculate_bin(rel_x, grid_cols);
                        let y_bin = calculate_bin(rel_y, grid_rows);

                        let hits = hit_counts[y_bin][x_bin];

                        // Calculate cell value ranges
                        let bin_width_x = x_range / grid_cols as f64;
                        let bin_width_y = y_range / grid_rows as f64;
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

                            // Only show Z-related statistics in AverageZ mode
                            if mode == HistogramMode::AverageZ {
                                ui.add_space(8.0);

                                ui.label(
                                    egui::RichText::new("Mean:").size(font_12).color(stat_color),
                                );
                                ui.label(
                                    egui::RichText::new(format!("{:.2}", cell.mean))
                                        .size(font_12)
                                        .color(value_color),
                                );
                                ui.add_space(8.0);

                                ui.label(
                                    egui::RichText::new("Min:").size(font_12).color(stat_color),
                                );
                                ui.label(
                                    egui::RichText::new(format!("{:.2}", cell.minimum))
                                        .size(font_12)
                                        .color(value_color),
                                );
                                ui.add_space(8.0);

                                ui.label(
                                    egui::RichText::new("Max:").size(font_12).color(stat_color),
                                );
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
                            }
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

        let config = &self.tabs[tab_idx].histogram_state.config;
        let selected = &config.selected_cell;
        let mode = config.mode;

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

                    // Build stats list - always show range and hit count
                    let mut stats: Vec<(&str, String)> = vec![
                        (
                            "X Range",
                            format!("{:.2} - {:.2}", cell.x_range.0, cell.x_range.1),
                        ),
                        (
                            "Y Range",
                            format!("{:.2} - {:.2}", cell.y_range.0, cell.y_range.1),
                        ),
                        ("Hit Count", format!("{}", cell.hit_count)),
                    ];

                    // Only show Z-related stats in AverageZ mode
                    if mode == HistogramMode::AverageZ {
                        stats.extend([
                            ("Cell Weight", format!("{:.4}", cell.cell_weight)),
                            ("Mean", format!("{:.4}", cell.mean)),
                            ("Minimum", format!("{:.4}", cell.minimum)),
                            ("Maximum", format!("{:.4}", cell.maximum)),
                            ("Variance", format!("{:.4}", cell.variance)),
                            ("Std Dev", format!("{:.4}", cell.std_dev)),
                        ]);
                    }

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

    /// Resample a pasted table to target dimensions using bilinear interpolation
    #[allow(clippy::needless_range_loop)]
    fn resample_table(
        table: &PastedTable,
        target_cols: usize,
        target_rows: usize,
    ) -> Vec<Vec<f64>> {
        let src_rows = table.data.len();
        let src_cols = if src_rows > 0 { table.data[0].len() } else { 0 };

        if src_rows == 0 || src_cols == 0 {
            return vec![vec![0.0; target_cols]; target_rows];
        }

        // If dimensions match, just reverse Y (pasted table has 0 = top, we need 0 = bottom)
        if src_rows == target_rows && src_cols == target_cols {
            let mut result = Vec::with_capacity(target_rows);
            for y in 0..target_rows {
                result.push(table.data[src_rows - 1 - y].clone());
            }
            return result;
        }

        let mut result = vec![vec![0.0; target_cols]; target_rows];

        for target_y in 0..target_rows {
            for target_x in 0..target_cols {
                // Map target coordinates to source coordinates
                let src_x =
                    target_x as f64 * (src_cols - 1) as f64 / (target_cols - 1).max(1) as f64;
                let src_y =
                    target_y as f64 * (src_rows - 1) as f64 / (target_rows - 1).max(1) as f64;

                // Bilinear interpolation
                let x0 = src_x.floor() as usize;
                let y0 = src_y.floor() as usize;
                let x1 = (x0 + 1).min(src_cols - 1);
                let y1 = (y0 + 1).min(src_rows - 1);

                let fx = src_x - x0 as f64;
                let fy = src_y - y0 as f64;

                // Note: y index in pasted table is reversed (0 = top, but we want 0 = bottom)
                let src_y0 = src_rows - 1 - y0;
                let src_y1 = src_rows - 1 - y1;

                let v00 = table
                    .data
                    .get(src_y0)
                    .and_then(|r| r.get(x0))
                    .copied()
                    .unwrap_or(0.0);
                let v10 = table
                    .data
                    .get(src_y0)
                    .and_then(|r| r.get(x1))
                    .copied()
                    .unwrap_or(0.0);
                let v01 = table
                    .data
                    .get(src_y1)
                    .and_then(|r| r.get(x0))
                    .copied()
                    .unwrap_or(0.0);
                let v11 = table
                    .data
                    .get(src_y1)
                    .and_then(|r| r.get(x1))
                    .copied()
                    .unwrap_or(0.0);

                let value = v00 * (1.0 - fx) * (1.0 - fy)
                    + v10 * fx * (1.0 - fy)
                    + v01 * (1.0 - fx) * fy
                    + v11 * fx * fy;

                result[target_y][target_x] = value;
            }
        }

        result
    }

    /// Apply an operation between histogram values and pasted table values
    /// For cells where histogram has no data but pasted does, use 0 for histogram value
    fn apply_table_operation(
        hist_values: &[Vec<Option<f64>>],
        pasted_values: &[Vec<f64>],
        operation: TableOperation,
    ) -> Vec<Vec<Option<f64>>> {
        let rows = hist_values.len();
        let cols = if rows > 0 { hist_values[0].len() } else { 0 };

        let mut result = vec![vec![None; cols]; rows];

        for y in 0..rows {
            for x in 0..cols {
                let pasted_val = pasted_values
                    .get(y)
                    .and_then(|r| r.get(x))
                    .copied()
                    .unwrap_or(0.0);

                // Only skip if pasted value is effectively zero (no data there either)
                if pasted_val.abs() < f64::EPSILON && hist_values[y][x].is_none() {
                    continue;
                }

                let hist_val = hist_values[y][x].unwrap_or(0.0);
                let res_val = match operation {
                    TableOperation::Add => hist_val + pasted_val,
                    TableOperation::Subtract => hist_val - pasted_val,
                    TableOperation::Multiply => hist_val * pasted_val,
                    TableOperation::Divide => {
                        if pasted_val.abs() > f64::EPSILON {
                            hist_val / pasted_val
                        } else {
                            0.0
                        }
                    }
                };
                result[y][x] = Some(res_val);
            }
        }

        result
    }

    /// Render a mini heat map panel (used in comparison view)
    /// hit_counts is optional - when provided, cells below min_hits_filter are grayed out
    #[allow(clippy::too_many_arguments)]
    fn render_mini_heat_map(
        painter: &egui::Painter,
        rect: egui::Rect,
        values: &[Vec<Option<f64>>],
        min_value: f64,
        max_value: f64,
        title: &str,
        font_size: f32,
        hit_counts: Option<&[Vec<u32>]>,
        min_hits_filter: u32,
    ) {
        let grid_rows = values.len();
        if grid_rows == 0 {
            return;
        }
        let grid_cols = values.get(0).map(|row| row.len()).unwrap_or(0);
        if grid_cols == 0 {
            return;
        }

        // Draw title
        painter.text(
            egui::pos2(rect.center().x, rect.top() + 15.0),
            egui::Align2::CENTER_CENTER,
            title,
            egui::FontId::proportional(font_size),
            egui::Color32::WHITE,
        );

        // Calculate plot area
        let plot_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 10.0, rect.top() + 30.0),
            egui::pos2(rect.right() - 10.0, rect.bottom() - 10.0),
        );

        painter.rect_filled(plot_rect, 0.0, egui::Color32::BLACK);

        let cell_width = plot_rect.width() / grid_cols as f32;
        let cell_height = plot_rect.height() / grid_rows as f32;

        let value_range = if (max_value - min_value).abs() < f64::EPSILON {
            1.0
        } else {
            max_value - min_value
        };

        for (y_bin, row) in values.iter().enumerate().take(grid_rows) {
            for (x_bin, cell_value) in row.iter().enumerate().take(grid_cols) {
                let cell_x = plot_rect.left() + x_bin as f32 * cell_width;
                let cell_y = plot_rect.bottom() - (y_bin + 1) as f32 * cell_height;
                let cell_rect = egui::Rect::from_min_size(
                    egui::pos2(cell_x, cell_y),
                    egui::vec2(cell_width, cell_height),
                );

                // Check min hits filter if hit_counts provided
                if let Some(hits) = hit_counts {
                    if min_hits_filter > 0 {
                        let cell_hits = hits
                            .get(y_bin)
                            .and_then(|r| r.get(x_bin))
                            .copied()
                            .unwrap_or(0);
                        if cell_hits < min_hits_filter {
                            painter.rect_filled(
                                cell_rect,
                                0.0,
                                egui::Color32::from_rgb(30, 30, 30),
                            );
                            continue;
                        }
                    }
                }

                if let Some(value) = cell_value {
                    let normalized = ((value - min_value) / value_range).clamp(0.0, 1.0);
                    let color = Self::get_histogram_color(normalized);
                    painter.rect_filled(cell_rect, 0.0, color);

                    // Draw value text if cell is large enough
                    if cell_width > 20.0 && cell_height > 14.0 {
                        let text = format!("{:.1}", value);
                        let text_color = get_aaa_text_color(color);
                        let max_dim = grid_cols.max(grid_rows);
                        let text_size = if max_dim <= 16 { 9.0 } else { 7.0 };
                        painter.text(
                            cell_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            text,
                            egui::FontId::proportional(text_size),
                            text_color,
                        );
                    }
                } else {
                    painter.rect_filled(cell_rect, 0.0, egui::Color32::from_rgb(30, 30, 30));
                }
            }
        }

        // Draw grid lines
        let grid_color = egui::Color32::from_rgb(60, 60, 60);
        // Vertical lines (X axis divisions)
        for i in 0..=grid_cols {
            let x = plot_rect.left() + i as f32 * cell_width;
            painter.line_segment(
                [
                    egui::pos2(x, plot_rect.top()),
                    egui::pos2(x, plot_rect.bottom()),
                ],
                egui::Stroke::new(0.5, grid_color),
            );
        }
        // Horizontal lines (Y axis divisions)
        for i in 0..=grid_rows {
            let y = plot_rect.top() + i as f32 * cell_height;
            painter.line_segment(
                [
                    egui::pos2(plot_rect.left(), y),
                    egui::pos2(plot_rect.right(), y),
                ],
                egui::Stroke::new(0.5, grid_color),
            );
        }
    }

    /// Copy histogram data to clipboard as TSV (tab-separated values)
    fn copy_histogram_to_clipboard(&self, tab_idx: usize) {
        let config = &self.tabs[tab_idx].histogram_state.config;
        let file_idx = self.tabs[tab_idx].file_index;

        if file_idx >= self.files.len() {
            return;
        }

        let (x_idx, y_idx) = match (config.x_channel, config.y_channel) {
            (Some(x), Some(y)) => (x, y),
            _ => return,
        };

        let (grid_cols, grid_rows) = config.effective_grid_size();
        let mode = config.mode;
        let z_idx = config.z_channel;

        let x_data = self.get_channel_data(file_idx, x_idx);
        let y_data = self.get_channel_data(file_idx, y_idx);
        let z_data = z_idx.map(|z| self.get_channel_data(file_idx, z));

        if x_data.is_empty() || y_data.is_empty() {
            return;
        }

        // Calculate data bounds
        let (x_min, x_max) = match config.custom_x_range {
            Some((min, max)) if max > min => (min, max),
            _ => {
                let min = x_data.iter().cloned().fold(f64::MAX, f64::min);
                let max = x_data.iter().cloned().fold(f64::MIN, f64::max);
                (min, max)
            }
        };
        let (y_min, y_max) = match config.custom_y_range {
            Some((min, max)) if max > min => (min, max),
            _ => {
                let min = y_data.iter().cloned().fold(f64::MAX, f64::min);
                let max = y_data.iter().cloned().fold(f64::MIN, f64::max);
                (min, max)
            }
        };

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
        let mut hit_counts = vec![vec![0u32; grid_cols]; grid_rows];
        let mut z_sums = vec![vec![0.0f64; grid_cols]; grid_rows];

        for i in 0..x_data.len() {
            let x_bin = calculate_data_bin(x_data[i], x_min, x_range, grid_cols);
            let y_bin = calculate_data_bin(y_data[i], y_min, y_range, grid_rows);
            hit_counts[y_bin][x_bin] += 1;
            if let Some(ref z) = z_data {
                z_sums[y_bin][x_bin] += z[i];
            }
        }

        // Generate TSV string
        let mut tsv = String::new();

        // Header row: X breakpoints
        tsv.push('\t'); // Empty cell for Y label column
        for x_bin in 0..grid_cols {
            let x_val = x_min + (x_bin as f64 + 0.5) * (x_range / grid_cols as f64);
            tsv.push_str(&format!("{:.1}", x_val));
            if x_bin < grid_cols - 1 {
                tsv.push('\t');
            }
        }
        tsv.push('\n');

        // Data rows (from top to bottom, so reverse Y order)
        for y_bin in (0..grid_rows).rev() {
            // Y label
            let y_val = y_min + (y_bin as f64 + 0.5) * (y_range / grid_rows as f64);
            tsv.push_str(&format!("{:.1}\t", y_val));

            for x_bin in 0..grid_cols {
                let value = match mode {
                    HistogramMode::HitCount => hit_counts[y_bin][x_bin] as f64,
                    HistogramMode::AverageZ => {
                        let hits = hit_counts[y_bin][x_bin];
                        if hits > 0 {
                            z_sums[y_bin][x_bin] / hits as f64
                        } else {
                            0.0
                        }
                    }
                };
                tsv.push_str(&format!("{:.2}", value));
                if x_bin < grid_cols - 1 {
                    tsv.push('\t');
                }
            }
            tsv.push('\n');
        }

        // Copy to clipboard
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(tsv);
        }
    }

    /// Copy the result of histogram + pasted table operation to clipboard
    fn copy_result_to_clipboard(&self, tab_idx: usize) {
        let config = &self.tabs[tab_idx].histogram_state.config;
        let file_idx = self.tabs[tab_idx].file_index;

        if file_idx >= self.files.len() {
            return;
        }

        // Need a pasted table to have a result
        let pasted_table = match &config.pasted_table {
            Some(table) => table,
            None => return,
        };

        let (x_idx, y_idx) = match (config.x_channel, config.y_channel) {
            (Some(x), Some(y)) => (x, y),
            _ => return,
        };

        let (grid_cols, grid_rows) = config.effective_grid_size();
        let mode = config.mode;
        let z_idx = config.z_channel;
        let table_operation = config.table_operation;

        let x_data = self.get_channel_data(file_idx, x_idx);
        let y_data = self.get_channel_data(file_idx, y_idx);
        let z_data = z_idx.map(|z| self.get_channel_data(file_idx, z));

        if x_data.is_empty() || y_data.is_empty() {
            return;
        }

        // Calculate data bounds
        let (x_min, x_max) = match config.custom_x_range {
            Some((min, max)) if max > min => (min, max),
            _ => {
                let min = x_data.iter().cloned().fold(f64::MAX, f64::min);
                let max = x_data.iter().cloned().fold(f64::MIN, f64::max);
                (min, max)
            }
        };
        let (y_min, y_max) = match config.custom_y_range {
            Some((min, max)) if max > min => (min, max),
            _ => {
                let min = y_data.iter().cloned().fold(f64::MAX, f64::min);
                let max = y_data.iter().cloned().fold(f64::MIN, f64::max);
                (min, max)
            }
        };

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
        let mut hit_counts = vec![vec![0u32; grid_cols]; grid_rows];
        let mut z_sums = vec![vec![0.0f64; grid_cols]; grid_rows];

        for i in 0..x_data.len() {
            let x_bin = calculate_data_bin(x_data[i], x_min, x_range, grid_cols);
            let y_bin = calculate_data_bin(y_data[i], y_min, y_range, grid_rows);
            hit_counts[y_bin][x_bin] += 1;
            if let Some(ref z) = z_data {
                z_sums[y_bin][x_bin] += z[i];
            }
        }

        // Build histogram values
        let hist_values: Vec<Vec<Option<f64>>> = (0..grid_rows)
            .map(|y_bin| {
                (0..grid_cols)
                    .map(|x_bin| {
                        let hits = hit_counts[y_bin][x_bin];
                        if hits > 0 {
                            Some(match mode {
                                HistogramMode::HitCount => hits as f64,
                                HistogramMode::AverageZ => z_sums[y_bin][x_bin] / hits as f64,
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .collect();

        // Resample pasted table and apply operation
        let resampled = Self::resample_table(pasted_table, grid_cols, grid_rows);
        let result_values = Self::apply_table_operation(&hist_values, &resampled, table_operation);

        // Generate TSV string with pasted table's breakpoints
        let mut tsv = String::new();

        // Header row: X breakpoints (use pasted table's breakpoints if they match grid size)
        tsv.push('\t'); // Empty cell for Y label column
        for x_bin in 0..grid_cols {
            let x_val = if !pasted_table.x_breakpoints.is_empty()
                && pasted_table.x_breakpoints.len() == grid_cols
            {
                pasted_table.x_breakpoints[x_bin]
            } else {
                x_min + (x_bin as f64 + 0.5) * (x_range / grid_cols as f64)
            };
            tsv.push_str(&format!("{:.1}", x_val));
            if x_bin < grid_cols - 1 {
                tsv.push('\t');
            }
        }
        tsv.push('\n');

        // Data rows (from top to bottom, so reverse Y order)
        for y_bin in (0..grid_rows).rev() {
            // Y label (use pasted table's breakpoints if they match grid size)
            let y_val = if !pasted_table.y_breakpoints.is_empty()
                && pasted_table.y_breakpoints.len() == grid_rows
            {
                pasted_table.y_breakpoints[grid_rows - 1 - y_bin]
            } else {
                y_min + (y_bin as f64 + 0.5) * (y_range / grid_rows as f64)
            };
            tsv.push_str(&format!("{:.1}\t", y_val));

            for (x_bin, val) in result_values[y_bin].iter().enumerate().take(grid_cols) {
                let value = val.unwrap_or(0.0);
                tsv.push_str(&format!("{:.2}", value));
                if x_bin < grid_cols - 1 {
                    tsv.push('\t');
                }
            }
            tsv.push('\n');
        }

        // Copy to clipboard
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(tsv);
        }
    }

    /// Paste table data from clipboard for comparison operations
    fn paste_table_from_clipboard(&mut self, tab_idx: usize) {
        let clipboard_text = match arboard::Clipboard::new() {
            Ok(mut clipboard) => match clipboard.get_text() {
                Ok(text) => text,
                Err(_) => return,
            },
            Err(_) => return,
        };

        // Parse TSV
        let lines: Vec<&str> = clipboard_text.lines().collect();
        if lines.is_empty() {
            return;
        }

        // Helper to parse numbers that may have comma thousands separators (e.g., "1,234.56")
        let parse_number = |s: &str| -> Option<f64> {
            let cleaned = s.trim().replace(',', "");
            cleaned.parse::<f64>().ok()
        };

        // Try to parse first row as X breakpoints
        let first_row: Vec<&str> = lines[0].split('\t').collect();
        let x_start = if first_row
            .first()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true)
        {
            1
        } else {
            0
        };
        let x_breakpoints: Vec<f64> = first_row[x_start..]
            .iter()
            .filter_map(|s| parse_number(s))
            .collect();

        let mut y_breakpoints = Vec::new();
        let mut data = Vec::new();

        for line in &lines[1..] {
            let cells: Vec<&str> = line.split('\t').collect();
            if cells.is_empty() {
                continue;
            }

            // First cell is Y breakpoint
            if let Some(y_val) = parse_number(cells[0]) {
                y_breakpoints.push(y_val);
            }

            // Remaining cells are data
            let row: Vec<f64> = cells[1..].iter().filter_map(|s| parse_number(s)).collect();
            if !row.is_empty() {
                data.push(row);
            }
        }

        if data.is_empty() {
            return;
        }

        let original_rows = data.len();
        let original_cols = data.first().map(|r| r.len()).unwrap_or(0);

        let pasted_table = PastedTable {
            data,
            x_breakpoints,
            y_breakpoints,
            original_rows,
            original_cols,
            is_resampled: false,
        };

        self.tabs[tab_idx].histogram_state.config.pasted_table = Some(pasted_table);
        self.tabs[tab_idx]
            .histogram_state
            .config
            .show_comparison_view = true;
    }
}
