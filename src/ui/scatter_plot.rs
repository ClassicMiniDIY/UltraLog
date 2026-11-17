//! Scatter plot / heatmap view for comparing two variables.
//!
//! Two side-by-side 2D histograms. Each cell is coloured by its hit count (log scale) or, when
//! a Z channel is chosen (issue #114), by the mean, minimum or maximum of that channel's
//! samples in the cell. The cells come from [`bin_scatter`], which the export uses too.

use eframe::egui;
use rust_i18n::t;

use crate::analysis::scatter::{ScatterError, ScatterGrid, ZStat, bin_scatter};
use crate::analysis::tables::binning::finite_bounds;
use crate::app::UltraLogApp;
use crate::colormap::{Colormap, ColormapLut};
use crate::normalize::sort_channels_by_priority;
use crate::state::{
    SCATTER_CACHE_MAX_ENTRIES, ScatterCacheKey, ScatterPlotConfig, ScatterZ, SelectedHeatmapPoint,
};
use crate::ui::components::{self, icon};
use crate::ui::fonts::{self, style};
use crate::ui::heatmap::{self, MapKind};

/// Number of bins in each dimension for the heatmap grid (higher = more detail)
const HEATMAP_BINS: usize = 512;

/// Margin for axis labels
const AXIS_LABEL_MARGIN_LEFT: f32 = 50.0;
const AXIS_LABEL_MARGIN_BOTTOM: f32 = 25.0;

/// Height reserved for the legend at the bottom
const LEGEND_HEIGHT: f32 = 35.0;

/// Colour scale for a Z channel: diverging around zero when the range spans it, otherwise
/// sequential. Same rule as the table tools (`render_table_tool_view`, `table_sheet`), shared
/// by the screen and the export.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ZScale {
    pub kind: MapKind,
    pub lo: f64,
    pub hi: f64,
}

impl ZScale {
    pub fn new(lo: f64, hi: f64) -> Self {
        let kind = if lo < 0.0 && hi > 0.0 {
            MapKind::Diverging
        } else {
            MapKind::Sequential
        };
        Self { kind, lo, hi }
    }

    fn span(&self) -> f64 {
        self.lo.abs().max(self.hi.abs()).max(f64::EPSILON)
    }

    /// `v` on the map's 0..=1 scale (0.5 is zero on a diverging map).
    pub fn norm(&self, v: f64) -> f32 {
        let t = match self.kind {
            MapKind::Diverging => 0.5 + v / (2.0 * self.span()),
            MapKind::Sequential => {
                let range = self.hi - self.lo;
                if range.abs() < f64::EPSILON {
                    1.0
                } else {
                    (v - self.lo) / range
                }
            }
        };
        t.clamp(0.0, 1.0) as f32
    }

    /// Decimals that tell the ends of the range apart.
    pub fn decimals(&self) -> usize {
        let span = (self.hi - self.lo).abs();
        if span < 1.0 {
            3
        } else if span < 10.0 {
            2
        } else if span < 1000.0 {
            1
        } else {
            0
        }
    }

    /// Labels for the two ends of the colour bar.
    pub fn end_labels(&self) -> (String, String) {
        let d = self.decimals();
        match self.kind {
            MapKind::Diverging => {
                let s = self.span();
                (format!("\u{2212}{s:.d$}"), format!("+{s:.d$}"))
            }
            MapKind::Sequential => (format!("{:.d$}", self.lo), format!("{:.d$}", self.hi)),
        }
    }

    pub fn colormap(&self) -> Colormap {
        match self.kind {
            MapKind::Sequential => Colormap::Sequential,
            MapKind::Diverging => Colormap::Diverging,
        }
    }
}

/// The Z colour scale of a plot: its fixed range when one is set, else the range of the
/// chosen statistic over the cells. `None` in Hits mode or when no cell holds a Z sample.
pub(crate) fn z_scale(grid: &ScatterGrid, config: &ScatterPlotConfig) -> Option<ZScale> {
    let ScatterZ::Channel { stat, .. } = config.z else {
        return None;
    };
    let (lo, hi) = match config.z_range {
        Some((lo, hi)) if hi > lo => (lo, hi),
        _ => grid.z_bounds(stat)?,
    };
    Some(ZScale::new(lo, hi))
}

/// Short label of a statistic.
pub(crate) fn stat_label(stat: ZStat) -> String {
    match stat {
        ZStat::Mean => t!("scatter.stat_mean").to_string(),
        ZStat::Min => t!("scatter.stat_min").to_string(),
        ZStat::Max => t!("scatter.stat_max").to_string(),
    }
}

impl UltraLogApp {
    /// Render the scatter plot view with two side-by-side plots
    pub fn render_scatter_plot_view(&mut self, ui: &mut egui::Ui) {
        // Check if we have an active tab with valid file
        if self.active_tab.is_none() || self.files.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new(t!("scatter.no_file_loaded"))
                        .heading()
                        .color(self.tokens.ink_subtle),
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

    fn scatter_config(&self, tab_idx: usize, is_left: bool) -> &ScatterPlotConfig {
        let state = &self.tabs[tab_idx].scatter_plot_state;
        if is_left { &state.left } else { &state.right }
    }

    fn scatter_config_mut(&mut self, tab_idx: usize, is_left: bool) -> &mut ScatterPlotConfig {
        let state = &mut self.tabs[tab_idx].scatter_plot_state;
        if is_left {
            &mut state.left
        } else {
            &mut state.right
        }
    }

    /// Render a single scatter plot panel with controls
    fn render_scatter_plot_panel(&mut self, ui: &mut egui::Ui, is_left: bool) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };

        let config = self.scatter_config(tab_idx, is_left);
        let file_idx = config.file_index.unwrap_or(self.tabs[tab_idx].file_index);
        let title = self.get_scatter_plot_title(config, file_idx);

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(&title)
                    .heading()
                    .strong()
                    .color(self.tokens.ink),
            );
        });

        ui.add_space(8.0);

        // Axis selectors are in the sidebar (tool properties panel)
        self.render_scatter_plot_chart(ui, is_left);
    }

    /// Display name of a channel (raw or computed), normalized when field normalization is on.
    fn scatter_channel_name(&self, file_idx: usize, channel_idx: Option<usize>) -> String {
        match channel_idx {
            Some(idx) if file_idx < self.files.len() => self.export_channel_name(file_idx, idx),
            _ => "---".to_string(),
        }
    }

    /// Name of what colours the cells: "Hits", or "AFR · Max".
    pub(crate) fn scatter_z_label(&self, config: &ScatterPlotConfig, file_idx: usize) -> String {
        match config.z {
            ScatterZ::Hits => t!("scatter.hits").to_string(),
            ScatterZ::Channel { index, stat } => format!(
                "{} \u{00B7} {}",
                self.scatter_channel_name(file_idx, Some(index)),
                stat_label(stat)
            ),
        }
    }

    /// Get the title for a heatmap based on selected axes
    fn get_scatter_plot_title(&self, config: &ScatterPlotConfig, file_idx: usize) -> String {
        if file_idx >= self.files.len() {
            return "No Data".to_string();
        }
        let x_name = self.scatter_channel_name(file_idx, config.x_channel);
        let y_name = self.scatter_channel_name(file_idx, config.y_channel);
        format!(
            "{} vs {} vs {}",
            y_name,
            x_name,
            self.scatter_z_label(config, file_idx)
        )
    }

    /// Bin a plot's channels. Shared by the screen (512 bins) and the export.
    pub(crate) fn build_scatter_grid(
        &self,
        file_idx: usize,
        x_idx: usize,
        y_idx: usize,
        z_idx: Option<usize>,
        bins: usize,
    ) -> Result<ScatterGrid, ScatterError> {
        let x = self.get_channel_data_ref(file_idx, x_idx);
        let y = self.get_channel_data_ref(file_idx, y_idx);
        let z = z_idx.map(|z| self.get_channel_data_ref(file_idx, z));
        bin_scatter(x, y, z, bins)
    }

    /// Render the actual heatmap chart
    fn render_scatter_plot_chart(&mut self, ui: &mut egui::Ui, is_left: bool) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };

        let config = self.scatter_config(tab_idx, is_left);
        let file_idx = config.file_index.unwrap_or(self.tabs[tab_idx].file_index);

        // Pre-compute scaled font sizes
        let font_10 = self.scaled_font(10.0);
        let font_11 = self.scaled_font(11.0);
        let font_16 = self.scaled_font(16.0);

        let placeholder = |ui: &mut egui::Ui, text: String, color: egui::Color32| {
            let available = ui.available_size();
            let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(font_16),
                color,
            );
        };

        // Check if we have valid axis selections
        let (x_idx, y_idx) = match (config.x_channel, config.y_channel) {
            (Some(x), Some(y)) => (x, y),
            _ => {
                placeholder(
                    ui,
                    t!("scatter.select_axes").to_string(),
                    self.tokens.ink_subtle,
                );
                return;
            }
        };

        if file_idx >= self.files.len() {
            return;
        }

        // Channel data is immutable once loaded, so a grid only needs building when the
        // (file, x, y, z) selection is first shown, not every frame. The cache is cleared when
        // files or computed channels are removed.
        let key = ScatterCacheKey {
            file: file_idx,
            x: x_idx,
            y: y_idx,
            z: config.z.channel(),
        };
        if !self.scatter_histogram_cache.contains_key(&key) {
            let grid = self.build_scatter_grid(file_idx, x_idx, y_idx, key.z, HEATMAP_BINS);
            if self.scatter_histogram_cache.len() >= SCATTER_CACHE_MAX_ENTRIES {
                self.scatter_histogram_cache.clear();
            }
            self.scatter_histogram_cache.insert(key, grid);
        }

        let grid = match self.scatter_histogram_cache.get(&key) {
            Some(Ok(grid)) => grid,
            Some(Err(ScatterError::MismatchedLengths)) => {
                placeholder(
                    ui,
                    t!("scatter.mixed_lengths").to_string(),
                    self.tokens.ink_subtle,
                );
                return;
            }
            _ => {
                placeholder(
                    ui,
                    t!("scatter.no_data").to_string(),
                    self.tokens.ink_subtle,
                );
                return;
            }
        };

        let config = self.scatter_config(tab_idx, is_left);
        let z_stat = match config.z {
            ScatterZ::Channel { stat, .. } => Some(stat),
            ScatterZ::Hits => None,
        };
        let scale = z_scale(grid, config);
        let z_name = match config.z {
            ScatterZ::Channel { index, .. } => {
                Some(self.scatter_channel_name(file_idx, Some(index)))
            }
            ScatterZ::Hits => None,
        };
        let (x_min, y_min, x_range, y_range) =
            (grid.x_min, grid.y_min, grid.x_range(), grid.y_range());
        let max_hits = grid.max_hits;
        let bins = grid.bins;

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

        let t = self.tokens;
        let painter = ui.painter_at(full_rect);
        painter.rect_filled(plot_rect, 0.0, t.plot_bg);

        // Calculate cell size based on plot rect (not full rect)
        let cell_width = plot_rect.width() / bins as f32;
        let cell_height = plot_rect.height() / bins as f32;

        // Draw heatmap cells (colours from a LUT: up to 512² bins every frame)
        let lut = ColormapLut::new(&t, scale.map_or(Colormap::Sequential, |s| s.colormap()));
        for y_bin in 0..bins {
            for x_bin in 0..bins {
                let idx = grid.index(x_bin, y_bin);
                let hits = grid.hits[idx];
                if hits == 0 {
                    continue;
                }
                let normalized = match (z_stat, scale) {
                    (Some(stat), Some(scale)) => match grid.z_value(idx, stat) {
                        Some((v, _)) => scale.norm(v as f64),
                        // Every Z sample in this cell was missing: leave it empty.
                        None => continue,
                    },
                    // Hits mode: log scale so sparse cells stay visible.
                    _ if max_hits > 1 => ((hits as f64).ln() / (max_hits as f64).ln()) as f32,
                    _ => 1.0,
                };

                // Calculate cell position (Y is inverted - higher values at top)
                let cell_x = plot_rect.left() + x_bin as f32 * cell_width;
                let cell_y = plot_rect.bottom() - (y_bin + 1) as f32 * cell_height;

                painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(cell_x, cell_y),
                        egui::vec2(cell_width + 0.5, cell_height + 0.5),
                    ),
                    0.0,
                    lut.get(normalized),
                );
            }
        }

        // Draw axes labels
        let text_color = t.plot_axis;

        // Y axis labels (left margin, outside plot area)
        let y_labels = 5;
        for i in 0..=y_labels {
            let f = i as f64 / y_labels as f64;
            let value = y_min + f * y_range;
            let y_pos = plot_rect.bottom() - f as f32 * plot_rect.height();

            painter.text(
                egui::pos2(plot_rect.left() - 5.0, y_pos),
                egui::Align2::RIGHT_CENTER,
                format!("{:.1}", value),
                egui::FontId::proportional(font_10),
                text_color,
            );

            painter.line_segment(
                [
                    egui::pos2(plot_rect.left(), y_pos),
                    egui::pos2(plot_rect.right(), y_pos),
                ],
                egui::Stroke::new(0.5, t.plot_grid),
            );
        }

        // X axis labels (bottom margin, outside plot area)
        let x_labels = 5;
        for i in 0..=x_labels {
            let f = i as f64 / x_labels as f64;
            let value = x_min + f * x_range;
            let x_pos = plot_rect.left() + f as f32 * plot_rect.width();

            painter.text(
                egui::pos2(x_pos, plot_rect.bottom() + 5.0),
                egui::Align2::CENTER_TOP,
                format!("{:.0}", value),
                egui::FontId::proportional(font_10),
                text_color,
            );

            painter.line_segment(
                [
                    egui::pos2(x_pos, plot_rect.top()),
                    egui::pos2(x_pos, plot_rect.bottom()),
                ],
                egui::Stroke::new(0.5, t.plot_grid),
            );
        }

        // Hover read-out for this frame (computed while the grid is borrowed).
        let z_decimals = scale.map_or(2, |s| s.decimals());
        let hover = response.hover_pos().and_then(|pos| {
            if !plot_rect.contains(pos) {
                return None;
            }
            let rel_x = (pos.x - plot_rect.left()) / plot_rect.width();
            let rel_y = 1.0 - (pos.y - plot_rect.top()) / plot_rect.height();
            if !(0.0..=1.0).contains(&rel_x) || !(0.0..=1.0).contains(&rel_y) {
                return None;
            }
            let x_val = x_min + rel_x as f64 * x_range;
            let y_val = y_min + rel_y as f64 * y_range;
            let (x_bin, y_bin) = grid.cell_of(x_val, y_val);
            let idx = grid.index(x_bin, y_bin);
            let z = z_stat.and_then(|stat| grid.z_value(idx, stat));
            Some((pos, x_val, y_val, grid.hits[idx], z))
        });

        // Selected point crosshairs (persistent)
        let config = self.scatter_config(tab_idx, is_left);
        if let Some(ref selected) = config.selected_point {
            let sel_rel_x = ((selected.x_value - x_min) / x_range) as f32;
            let sel_rel_y = ((selected.y_value - y_min) / y_range) as f32;

            if (0.0..=1.0).contains(&sel_rel_x) && (0.0..=1.0).contains(&sel_rel_y) {
                let sel_x = plot_rect.left() + sel_rel_x * plot_rect.width();
                let sel_y = plot_rect.bottom() - sel_rel_y * plot_rect.height();

                let selected_color = t.plot_cursor;
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
                painter.circle_filled(egui::pos2(sel_x, sel_y), 4.0, selected_color);
            }
        }

        // Hover crosshairs, read-out and click-to-select (only within the plot area)
        if let Some((pos, x_val, y_val, hits, z)) = hover {
            painter.line_segment(
                [
                    egui::pos2(pos.x, plot_rect.top()),
                    egui::pos2(pos.x, plot_rect.bottom()),
                ],
                egui::Stroke::new(1.0, t.ink_subtle),
            );
            painter.line_segment(
                [
                    egui::pos2(plot_rect.left(), pos.y),
                    egui::pos2(plot_rect.right(), pos.y),
                ],
                egui::Stroke::new(1.0, t.ink_subtle),
            );

            let mut tooltip_text = format!(
                "X: {:.1}\nY: {:.1}\n{}: {}",
                x_val,
                y_val,
                t!("scatter.hits"),
                hits
            );
            if let (Some(name), Some(stat)) = (&z_name, z_stat) {
                let value = match z {
                    Some((v, n)) => format!("{v:.z_decimals$} (n={n})"),
                    None => "\u{2014}".to_string(),
                };
                tooltip_text.push_str(&format!("\n{name} {}: {value}", stat_label(stat)));
            }
            painter.text(
                egui::pos2(plot_rect.right() - 10.0, plot_rect.top() + 15.0),
                egui::Align2::RIGHT_TOP,
                tooltip_text,
                egui::FontId::monospace(font_11),
                t.ink,
            );

            if response.clicked() {
                self.scatter_config_mut(tab_idx, is_left).selected_point =
                    Some(SelectedHeatmapPoint {
                        x_value: x_val,
                        y_value: y_val,
                        hits,
                        z,
                    });
            }
        }

        ui.add_space(8.0);
        self.render_heatmap_legend(ui, max_hits, scale, is_left);
    }

    /// Render the heatmap legend with color scale and selected point info
    fn render_heatmap_legend(
        &mut self,
        ui: &mut egui::Ui,
        max_hits: u32,
        scale: Option<ZScale>,
        is_left: bool,
    ) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };

        let config = self.scatter_config(tab_idx, is_left);
        let selected_point = config.selected_point.clone();
        let file_idx = config.file_index.unwrap_or(self.tabs[tab_idx].file_index);
        let x_name = self.scatter_channel_name(file_idx, config.x_channel);
        let y_name = self.scatter_channel_name(file_idx, config.y_channel);
        let z_label = self.scatter_z_label(config, file_idx);
        let z_decimals = scale.map_or(2, |s| s.decimals());

        let (kind, range_text) = match scale {
            Some(scale) => {
                let (lo, hi) = scale.end_labels();
                (scale.kind, format!("{lo} \u{2013} {hi}"))
            }
            None if matches!(config.z, ScatterZ::Channel { .. }) => {
                (MapKind::Sequential, "\u{2014}".to_string())
            }
            None => (MapKind::Sequential, format!("1 \u{2013} {max_hits}")),
        };

        let mut should_clear = false;

        ui.horizontal(|ui| {
            ui.add_space(4.0);

            // Color scale legend
            egui::Frame::NONE
                .fill(self.tokens.bg_raised)
                .corner_radius(4)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{z_label}:"))
                                .small()
                                .color(self.tokens.ink),
                        );

                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(100.0, 8.0), egui::Sense::hover());
                        heatmap::gradient_bar(ui.painter(), rect, &self.tokens, kind);

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(range_text)
                                .text_style(fonts::text_style(style::DATA_SM))
                                .color(self.tokens.ink),
                        );
                    });
                });

            ui.add_space(8.0);

            // Selected point legend (similar to log viewer channel display)
            if let Some(ref selected) = selected_point {
                egui::Frame::NONE
                    .fill(self.tokens.bg_raised)
                    .corner_radius(4)
                    .inner_margin(6.0)
                    .stroke(egui::Stroke::new(1.0, self.tokens.cyan))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (dot_rect, _) = ui
                                .allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            ui.painter()
                                .circle_filled(dot_rect.center(), 5.0, self.tokens.cyan);

                            ui.add_space(4.0);

                            let mut text = format!(
                                "{}: {:.1}  |  {}: {:.1}  |  {}: {}",
                                x_name,
                                selected.x_value,
                                y_name,
                                selected.y_value,
                                t!("scatter.hits"),
                                selected.hits
                            );
                            if let Some((v, n)) = selected.z {
                                text.push_str(&format!("  |  {z_label}: {v:.z_decimals$} (n={n})"));
                            }
                            ui.label(egui::RichText::new(text).small().color(self.tokens.ink));

                            ui.add_space(8.0);

                            if components::icon_button(
                                ui,
                                icon::X,
                                t!("scatter.clear_point").to_string(),
                            )
                            .clicked()
                            {
                                should_clear = true;
                            }
                        });
                    });
            }
        });

        if should_clear {
            self.scatter_config_mut(tab_idx, is_left).selected_point = None;
        }
    }

    /// Render scatter plot controls in the sidebar - optimized for vertical layout
    /// Shows controls for both left and right plots
    pub(crate) fn render_scatter_plot_controls(&mut self, ui: &mut egui::Ui) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };
        let file_idx = self.tabs[tab_idx].file_index;
        if file_idx >= self.files.len() {
            return;
        }

        let file = &self.files[file_idx];
        let base_channel_count = file.log.channels.len();
        let mut channels = sort_channels_by_priority(
            base_channel_count,
            |idx| file.log.channels[idx].name(),
            self.field_normalization,
            Some(&self.custom_normalizations),
        );
        // Computed channels follow the log's own channels (same indices as `get_channel_data`).
        if let Some(computed) = self.file_computed_channels.get(&file_idx) {
            for (i, c) in computed.iter().enumerate() {
                channels.push((
                    base_channel_count + i,
                    format!("{} {}", icon::FUNCTION, c.template.name),
                    false,
                ));
            }
        }

        for is_left in [true, false] {
            if !is_left {
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(16.0);
            }
            self.render_scatter_axis_section(ui, tab_idx, file_idx, is_left, &channels);
        }
    }

    /// One plot's X / Y / Z controls.
    fn render_scatter_axis_section(
        &mut self,
        ui: &mut egui::Ui,
        tab_idx: usize,
        file_idx: usize,
        is_left: bool,
        channels: &[(usize, String, bool)],
    ) {
        let side = if is_left { "left" } else { "right" };
        let name_of = |idx: Option<usize>| -> String {
            idx.and_then(|i| channels.iter().find(|(c, _, _)| *c == i))
                .map(|(_, n, _)| n.clone())
                .unwrap_or_else(|| t!("scatter.select").to_string())
        };
        let config = self.scatter_config(tab_idx, is_left).clone();
        let mut new = config.clone();

        egui::Frame::NONE
            .fill(self.tokens.bg_raised)
            .corner_radius(6)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                let heading = if is_left {
                    t!("scatter.left_plot")
                } else {
                    t!("scatter.right_plot")
                };
                ui.label(
                    egui::RichText::new(format!("{} {}", icon::CHART_SCATTER, heading))
                        .heading()
                        .strong(),
                );
                ui.add_space(12.0);

                // X and Y axes
                for (label, salt, current, target) in [
                    (
                        t!("scatter.x_axis"),
                        "x",
                        config.x_channel,
                        &mut new.x_channel,
                    ),
                    (
                        t!("scatter.y_axis"),
                        "y",
                        config.y_channel,
                        &mut new.y_channel,
                    ),
                ] {
                    ui.label(egui::RichText::new(label).heading().strong());
                    egui::ComboBox::from_id_salt(format!("{side}_{salt}_sidebar"))
                        .selected_text(name_of(current))
                        .width(ui.available_width())
                        .show_ui(ui, |ui| {
                            for (idx, name, _) in channels {
                                if ui.selectable_label(current == Some(*idx), name).clicked() {
                                    *target = Some(*idx);
                                }
                            }
                        });
                    ui.add_space(8.0);
                }

                // Z: hit count or a channel
                ui.add_space(4.0);
                ui.label(egui::RichText::new(t!("scatter.z_axis")).heading().strong());
                let z_text = match config.z {
                    ScatterZ::Hits => t!("scatter.hits").to_string(),
                    ScatterZ::Channel { index, .. } => name_of(Some(index)),
                };
                egui::ComboBox::from_id_salt(format!("{side}_z_sidebar"))
                    .selected_text(z_text)
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                config.z == ScatterZ::Hits,
                                t!("scatter.hits").to_string(),
                            )
                            .clicked()
                        {
                            new.z = ScatterZ::Hits;
                        }
                        for (idx, name, _) in channels {
                            if ui
                                .selectable_label(config.z.channel() == Some(*idx), name)
                                .clicked()
                            {
                                let stat = match config.z {
                                    ScatterZ::Channel { stat, .. } => stat,
                                    ScatterZ::Hits => ZStat::default(),
                                };
                                new.z = ScatterZ::Channel { index: *idx, stat };
                            }
                        }
                    });

                if let ScatterZ::Channel { index, mut stat } = new.z {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(t!("scatter.cell_value"))
                            .small()
                            .color(self.tokens.ink_subtle),
                    );
                    let labels: Vec<(ZStat, String)> =
                        ZStat::ALL.iter().map(|&s| (s, stat_label(s))).collect();
                    let options: Vec<(ZStat, &str)> =
                        labels.iter().map(|(s, l)| (*s, l.as_str())).collect();
                    if components::segmented(ui, &mut stat, &options) {
                        new.z = ScatterZ::Channel { index, stat };
                    }

                    ui.add_space(8.0);
                    let mut auto = new.z_range.is_none();
                    if components::checkbox(ui, &mut auto, t!("scatter.z_auto_range").to_string())
                        .changed()
                    {
                        new.z_range = if auto {
                            None
                        } else {
                            // Start from the channel's own range.
                            Some(
                                finite_bounds(self.get_channel_data_ref(file_idx, index))
                                    .unwrap_or((0.0, 1.0)),
                            )
                        };
                    }
                    if let Some((lo, hi)) = new.z_range.as_mut() {
                        ui.horizontal(|ui| {
                            let speed = ((*hi - *lo).abs() / 200.0).max(0.001);
                            ui.add(egui::DragValue::new(lo).speed(speed));
                            ui.label("\u{2013}");
                            ui.add(egui::DragValue::new(hi).speed(speed));
                        });
                    }
                }
            });

        // A colour range set for one channel means nothing for another.
        if new.z.channel() != config.z.channel() {
            new.z_range = None;
        }
        // A selection made under other axes no longer points at the same cell.
        if (new.x_channel, new.y_channel, new.z) != (config.x_channel, config.y_channel, config.z) {
            new.selected_point = None;
        }
        *self.scatter_config_mut(tab_idx, is_left) = new;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn z_scale_picks_diverging_only_across_zero() {
        assert_eq!(ZScale::new(-0.1, 0.1).kind, MapKind::Diverging);
        assert_eq!(ZScale::new(10.0, 16.0).kind, MapKind::Sequential);
        assert_eq!(ZScale::new(0.0, 5.0).kind, MapKind::Sequential);
    }

    #[test]
    fn z_scale_normalizes() {
        let s = ZScale::new(10.0, 20.0);
        assert_eq!(s.norm(10.0), 0.0);
        assert_eq!(s.norm(15.0), 0.5);
        assert_eq!(s.norm(30.0), 1.0);
        let d = ZScale::new(-0.2, 0.1);
        assert_eq!(d.norm(0.0), 0.5);
        assert_eq!(d.norm(-0.2), 0.0);
        assert_eq!(d.end_labels(), ("\u{2212}0.200".into(), "+0.200".into()));
    }

    #[test]
    fn z_scale_uses_fixed_range_when_set() {
        let grid = bin_scatter(&[0.0, 1.0], &[0.0, 1.0], Some(&[12.0, 15.0]), 4).unwrap();
        let mut config = ScatterPlotConfig {
            z: ScatterZ::Channel {
                index: 2,
                stat: ZStat::Mean,
            },
            ..Default::default()
        };
        assert_eq!(z_scale(&grid, &config), Some(ZScale::new(12.0, 15.0)));
        config.z_range = Some((10.0, 16.0));
        assert_eq!(z_scale(&grid, &config), Some(ZScale::new(10.0, 16.0)));
        // An inverted or empty range falls back to auto.
        config.z_range = Some((16.0, 16.0));
        assert_eq!(z_scale(&grid, &config), Some(ZScale::new(12.0, 15.0)));
        config.z = ScatterZ::Hits;
        assert_eq!(z_scale(&grid, &config), None);
    }
}
