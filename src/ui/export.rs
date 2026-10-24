//! Export: build an ExportSheet (`ui/export_sheet`) from the active tool's state and write it
//! as PNG or PDF, or copy the chart or table as an image for pasting.
//!
//! Every tool exports through [`UltraLogApp::export_sheet`], so the chart, scatter plots,
//! histogram and tuning tables share one layout, the export themes and one file path.

use std::path::Path;

use rust_i18n::t;

use crate::analysis::tables::binning::uniform_bin;
use crate::analytics;
use crate::app::UltraLogApp;
use crate::normalize::normalize_channel_name_with_custom;
use crate::state::{ActiveTool, HistogramMode, ScatterPlotConfig};
use crate::ui::export_sheet::{
    self, Body, ChartModel, ExportFormat, GridModel, SeriesModel, Sheet,
};
use crate::ui::heatmap::MapKind;
use crate::ui::theme::ExportTheme;

/// Points per series on an exported chart (LTTB keeps peaks and dropouts).
const CHART_POINTS: usize = 1500;
/// Bins per axis on an exported scatter density map.
const SCATTER_BINS: usize = 96;

pub(crate) type ExportResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Write `sheet` to `path` in `format`.
pub fn write_sheet(
    sheet: &Sheet,
    format: ExportFormat,
    theme: ExportTheme,
    path: &Path,
) -> ExportResult<()> {
    match format {
        ExportFormat::Png => export_sheet::render_png(sheet, theme).save(path)?,
        ExportFormat::Pdf => std::fs::write(path, export_sheet::render_pdf(sheet, theme))?,
    }
    Ok(())
}

/// Decimals that keep about three significant digits across `range`.
fn decimals_for(range: f64) -> usize {
    if range >= 100.0 {
        0
    } else if range >= 10.0 {
        1
    } else {
        2
    }
}

/// `count + 1` evenly spaced axis labels from `min` to `min + range`.
pub(crate) fn even_ticks(min: f64, range: f64, count: usize) -> Vec<(f32, String)> {
    let d = decimals_for(range);
    (0..=count)
        .map(|i| {
            let k = i as f64 / count as f64;
            (k as f32, format!("{:.d$}", min + k * range))
        })
        .collect()
}

/// `n` with thin-space thousands separators (`223 559`).
pub(crate) fn group_thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{2009}');
        }
        out.push(ch);
    }
    out
}

/// Finite min and max of `values`, or `None` when there are none.
fn finite_bounds(values: &[f64]) -> Option<(f64, f64)> {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &v in values.iter().filter(|v| v.is_finite()) {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    (lo <= hi).then_some((lo, hi))
}

fn nonzero(range: f64) -> f64 {
    if range.abs() < f64::EPSILON {
        1.0
    } else {
        range
    }
}

impl UltraLogApp {
    /// The display name of a channel (normalized when field normalization is on).
    fn export_channel_name(&self, file_idx: usize, channel_idx: usize) -> String {
        let name = self.get_channel_name(file_idx, channel_idx);
        if self.field_normalization {
            normalize_channel_name_with_custom(&name, Some(&self.custom_normalizations))
        } else {
            name
        }
    }

    fn active_file_index(&self) -> Option<usize> {
        self.active_tab
            .and_then(|i| self.tabs.get(i))
            .map(|tab| tab.file_index)
            .filter(|&i| i < self.files.len())
    }

    /// The file-name stem and analytics id for the active tool's export.
    fn export_slug(&self) -> String {
        match self.active_tool {
            ActiveTool::LogViewer => "chart".into(),
            ActiveTool::ScatterPlot => "scatter_plot".into(),
            ActiveTool::Histogram => "histogram".into(),
            _ => self
                .active_tool
                .generator_kind()
                .map_or("table".into(), |k| k.id().to_string()),
        }
    }

    /// Build the ExportSheet for the active tool.
    pub fn export_sheet(&self) -> ExportResult<Sheet> {
        match self.active_tool {
            ActiveTool::LogViewer => self.chart_sheet(),
            ActiveTool::ScatterPlot => self.scatter_sheet(),
            ActiveTool::Histogram => self.histogram_sheet(),
            _ => {
                let kind = self
                    .active_tool
                    .generator_kind()
                    .ok_or("Nothing to export")?;
                let acc = self
                    .table_generator
                    .table(kind)
                    .ok_or_else(|| t!("table_gen.no_events").to_string())?;
                crate::ui::table_export::table_sheet(
                    acc,
                    self.table_generator.measure_index(kind),
                    &self.table_generator.export,
                    self.active_tool.name(),
                )
            }
        }
    }

    /// Export the active tool to a file the user picks.
    pub fn export_to_file(&mut self, format: ExportFormat, theme: ExportTheme) {
        let sheet = match self.export_sheet() {
            Ok(sheet) => sheet,
            Err(e) => {
                self.show_toast_error(&t!("toast.export_failed", error = e.to_string()));
                return;
            }
        };
        let slug = self.export_slug();
        let ext = format.extension();
        let filter = match format {
            ExportFormat::Pdf => "PDF Document",
            ExportFormat::Png => "PNG Image",
        };
        let success = match (self.active_tool, format) {
            (ActiveTool::LogViewer, ExportFormat::Pdf) => t!("toast.export_pdf_success"),
            (ActiveTool::LogViewer, ExportFormat::Png) => t!("toast.export_png_success"),
            (ActiveTool::ScatterPlot, ExportFormat::Pdf) => t!("toast.scatter_exported_pdf"),
            (ActiveTool::ScatterPlot, ExportFormat::Png) => t!("toast.scatter_exported_png"),
            (ActiveTool::Histogram, ExportFormat::Pdf) => t!("toast.histogram_exported_pdf"),
            (ActiveTool::Histogram, ExportFormat::Png) => t!("toast.histogram_exported_png"),
            (_, ExportFormat::Pdf) => t!("toast.table_exported_pdf"),
            (_, ExportFormat::Png) => t!("toast.table_exported_png"),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(filter, &[ext])
            .set_file_name(format!("ultralog_{slug}.{ext}"))
            .save_file()
        else {
            return;
        };
        match write_sheet(&sheet, format, theme, &path) {
            Ok(()) => {
                // The chart keeps its pre-3.0 event names ("png" / "pdf").
                let event = if slug == "chart" {
                    ext.to_string()
                } else {
                    format!("{slug}_{ext}")
                };
                analytics::track_export(&event);
                self.show_toast_success(&success);
            }
            Err(e) => self.show_toast_error(&t!("toast.export_failed", error = e.to_string())),
        }
    }

    /// Copy the chart or table alone (no header or footer) as an image for pasting.
    pub fn copy_export_image(&mut self, theme: ExportTheme) {
        let result = self.export_sheet().and_then(|sheet| {
            let img = export_sheet::render_png_body(&sheet, theme);
            let data = arboard::ImageData {
                width: img.width() as usize,
                height: img.height() as usize,
                bytes: std::borrow::Cow::Owned(img.into_raw()),
            };
            arboard::Clipboard::new()?.set_image(data)?;
            Ok(())
        });
        match result {
            Ok(()) => {
                analytics::track_export(&format!("{}_clipboard", self.export_slug()));
                self.show_toast_success(&t!("toast.export_copied"));
            }
            Err(e) => self.show_toast_error(&t!("toast.export_failed", error = e.to_string())),
        }
    }

    /// The Log Viewer chart over the visible time range.
    fn chart_sheet(&self) -> ExportResult<Sheet> {
        let channels = self.get_selected_channels();
        if channels.is_empty() {
            return Err("No channels selected".into());
        }
        let (full0, full1) = self.time_range.ok_or("No time range available")?;
        let (t0, t1) = self
            .chart_last_x_bounds
            .get(&0)
            .map(|&(a, b)| (a.max(full0), b.min(full1)))
            .filter(|(a, b)| b > a)
            .unwrap_or((full0, full1));
        if t1 <= t0 {
            return Err("Invalid time range".into());
        }

        let mut series = Vec::new();
        for selected in channels {
            let Some(file) = self.files.get(selected.file_index) else {
                continue;
            };
            let times = file.log.get_times_as_f64();
            let data = self.get_channel_data_ref(selected.file_index, selected.channel_index);
            if times.is_empty() || times.len() != data.len() {
                continue;
            }
            // Normalized to the whole channel, like the chart.
            let Some((lo, hi)) = finite_bounds(data) else {
                continue;
            };
            let range = nonzero(hi - lo);
            let a = times.partition_point(|&t| t < t0).saturating_sub(1);
            let b = (times.partition_point(|&t| t <= t1) + 1).min(times.len());
            let points =
                crate::app::UltraLogApp::downsample_lttb(&times[a..b], &data[a..b], CHART_POINTS)
                    .into_iter()
                    .filter(|p| p[1].is_finite())
                    .map(|p| [p[0].clamp(t0, t1), ((p[1] - lo) / range).clamp(0.0, 1.0)])
                    .collect();
            let unit = self
                .unit_preferences
                .convert_value(0.0, selected.channel.unit())
                .1
                .to_string();
            series.push(SeriesModel {
                name: self.export_channel_name(selected.file_index, selected.channel_index),
                unit,
                color_index: selected.color_index,
                points,
            });
        }
        if series.is_empty() {
            return Err("No data available".into());
        }

        let file = self
            .active_file_index()
            .or(channels.first().map(|c| c.file_index))
            .and_then(|i| self.files.get(i))
            .ok_or("No file loaded")?;
        Ok(Sheet {
            title: file.name.clone(),
            meta: vec![
                format!(
                    "{} \u{2013} {}",
                    Self::format_time(t0),
                    Self::format_time(t1)
                ),
                format!(
                    "{} records \u{00B7} {} ch",
                    group_thousands(file.log.times.len()),
                    file.log.channels.len()
                ),
            ],
            body: Body::Chart(ChartModel { series, t0, t1 }),
        })
    }

    /// The histogram as the Histogram tool shows it (custom ranges, sample filters, min hits).
    fn histogram_sheet(&self) -> ExportResult<Sheet> {
        let tab_idx = self.active_tab.ok_or("No active tab")?;
        let config = &self.tabs[tab_idx].histogram_state.config;
        let file_idx = self.active_file_index().ok_or("Invalid file index")?;
        let mode = config.mode;
        let (cols, rows) = config.effective_grid_size();

        let x_idx = config.x_channel.ok_or("X axis not selected")?;
        let y_idx = config.y_channel.ok_or("Y axis not selected")?;
        let z_idx = match mode {
            HistogramMode::AverageZ => Some(
                config
                    .z_channel
                    .ok_or("Z axis not selected for Average mode")?,
            ),
            HistogramMode::HitCount => None,
        };
        let x_data = self.get_channel_data(file_idx, x_idx);
        let y_data = self.get_channel_data(file_idx, y_idx);
        let z_data = z_idx.map(|z| self.get_channel_data(file_idx, z));
        if x_data.is_empty() || x_data.len() != y_data.len() {
            return Err("No data available".into());
        }

        let bounds = |custom: Option<(f64, f64)>, data: &[f64]| match custom {
            Some((lo, hi)) if hi > lo => (lo, hi),
            _ => finite_bounds(data).unwrap_or((0.0, 1.0)),
        };
        let (x_min, x_max) = bounds(config.custom_x_range, &x_data);
        let (y_min, y_max) = bounds(config.custom_y_range, &y_data);
        let (x_range, y_range) = (nonzero(x_max - x_min), nonzero(y_max - y_min));

        let filters: Vec<_> = config
            .sample_filters
            .iter()
            .filter(|f| f.enabled)
            .map(|f| (f, self.get_channel_data(file_idx, f.channel_idx)))
            .collect();

        // Same binning as `render_histogram_grid` (`binning::uniform_bin`).
        let mut hits = vec![vec![0u32; cols]; rows];
        let mut z_sums = vec![vec![0.0f64; cols]; rows];
        let mut samples = 0usize;
        'samples: for i in 0..x_data.len() {
            for (filter, data) in &filters {
                let Some(&v) = data.get(i) else {
                    continue 'samples;
                };
                if filter.min_value.is_some_and(|m| v < m)
                    || filter.max_value.is_some_and(|m| v > m)
                {
                    continue 'samples;
                }
            }
            if config.custom_x_range.is_some() && (x_data[i] < x_min || x_data[i] > x_max) {
                continue;
            }
            if config.custom_y_range.is_some() && (y_data[i] < y_min || y_data[i] > y_max) {
                continue;
            }
            let xb = uniform_bin(x_data[i], x_min, x_range, cols);
            let yb = uniform_bin(y_data[i], y_min, y_range, rows);
            hits[yb][xb] += 1;
            if let Some(z) = &z_data {
                z_sums[yb][xb] += z[i];
            }
            samples += 1;
        }

        let min_hits = config.min_hits_filter;
        let mut values = vec![vec![None::<f64>; cols]; rows];
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for r in 0..rows {
            for c in 0..cols {
                let h = hits[r][c];
                if h == 0 || (min_hits > 0 && h < min_hits) {
                    continue;
                }
                let v = match mode {
                    HistogramMode::HitCount => h as f64,
                    HistogramMode::AverageZ => z_sums[r][c] / h as f64,
                };
                values[r][c] = Some(v);
                lo = lo.min(v);
                hi = hi.max(v);
            }
        }
        if !lo.is_finite() {
            return Err("No data available".into());
        }
        let value_range = nonzero(hi - lo);
        let norm = values
            .iter()
            .map(|row| {
                row.iter()
                    .map(|v| {
                        v.map(|v| {
                            let k = if mode == HistogramMode::HitCount && hi > 1.0 {
                                v.ln() / hi.ln()
                            } else {
                                (v - lo) / value_range
                            };
                            k.clamp(0.0, 1.0) as f32
                        })
                    })
                    .collect()
            })
            .collect();
        let labels = values
            .iter()
            .enumerate()
            .map(|(r, row)| {
                row.iter()
                    .enumerate()
                    .map(|(c, v)| {
                        v.map(|v| match mode {
                            HistogramMode::HitCount => hits[r][c].to_string(),
                            HistogramMode::AverageZ => format!("{v:.1}"),
                        })
                    })
                    .collect()
            })
            .collect();
        let (scale_title, scale_min, scale_max) = match mode {
            HistogramMode::HitCount => (
                t!("histogram.scale_hits").to_string(),
                "0".to_string(),
                format!("{hi:.0}"),
            ),
            HistogramMode::AverageZ => (
                format!(
                    "{} \u{00B7} {}",
                    t!("histogram.scale_average"),
                    self.export_channel_name(file_idx, z_idx.unwrap_or(0))
                ),
                format!("{lo:.1}"),
                format!("{hi:.1}"),
            ),
        };

        let file = &self.files[file_idx];
        Ok(Sheet {
            title: file.name.clone(),
            meta: vec![
                format!("{cols} \u{00D7} {rows}"),
                format!("{} samples", group_thousands(samples)),
            ],
            body: Body::Grids(vec![GridModel {
                caption: None,
                x_title: self.export_channel_name(file_idx, x_idx),
                y_title: self.export_channel_name(file_idx, y_idx),
                x_ticks: even_ticks(x_min, x_range, 4),
                y_ticks: even_ticks(y_min, y_range, 4),
                norm,
                labels,
                counts: None,
                kind: MapKind::Sequential,
                scale_title,
                scale_min,
                scale_max,
                dense: false,
            }]),
        })
    }

    /// Both scatter plots as density maps, side by side.
    fn scatter_sheet(&self) -> ExportResult<Sheet> {
        let tab_idx = self.active_tab.ok_or("No active tab")?;
        let state = &self.tabs[tab_idx].scatter_plot_state;
        let fallback = self.tabs[tab_idx].file_index;
        let grids: Vec<GridModel> = [&state.left, &state.right]
            .into_iter()
            .filter_map(|config| self.scatter_grid(config, fallback))
            .collect();
        if grids.is_empty() {
            return Err("Select X and Y channels to export a scatter plot".into());
        }
        let file = self
            .files
            .get(state.left.file_index.unwrap_or(fallback))
            .ok_or("Invalid file index")?;
        Ok(Sheet {
            title: file.name.clone(),
            meta: vec![format!("{} records", group_thousands(file.log.times.len()))],
            body: Body::Grids(grids),
        })
    }

    fn scatter_grid(&self, config: &ScatterPlotConfig, fallback: usize) -> Option<GridModel> {
        let file_idx = config.file_index.unwrap_or(fallback);
        let (x_idx, y_idx) = (config.x_channel?, config.y_channel?);
        if file_idx >= self.files.len() {
            return None;
        }
        let x = self.get_channel_data(file_idx, x_idx);
        let y = self.get_channel_data(file_idx, y_idx);
        if x.is_empty() || x.len() != y.len() {
            return None;
        }
        let (x_min, x_max) = finite_bounds(&x)?;
        let (y_min, y_max) = finite_bounds(&y)?;
        let (x_range, y_range) = (nonzero(x_max - x_min), nonzero(y_max - y_min));
        let n = SCATTER_BINS;
        let mut hits = vec![vec![0u32; n]; n];
        for (&xv, &yv) in x.iter().zip(&y) {
            if xv.is_finite() && yv.is_finite() {
                hits[uniform_bin(yv, y_min, y_range, n)][uniform_bin(xv, x_min, x_range, n)] += 1;
            }
        }
        let max = hits.iter().flatten().copied().max().unwrap_or(0);
        let norm = hits
            .iter()
            .map(|row| {
                row.iter()
                    .map(|&h| {
                        (h > 0).then(|| {
                            if max > 1 {
                                ((h as f64).ln() / (max as f64).ln()) as f32
                            } else {
                                1.0
                            }
                        })
                    })
                    .collect()
            })
            .collect();
        let x_title = self.export_channel_name(file_idx, x_idx);
        let y_title = self.export_channel_name(file_idx, y_idx);
        Some(GridModel {
            caption: Some(format!("{y_title} vs {x_title}")),
            x_title,
            y_title,
            x_ticks: even_ticks(x_min, x_range, 4),
            y_ticks: even_ticks(y_min, y_range, 4),
            norm,
            labels: Vec::new(),
            counts: None,
            kind: MapKind::Sequential,
            scale_title: t!("histogram.scale_hits").to_string(),
            scale_min: "1".into(),
            scale_max: max.to_string(),
            dense: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_use_thin_spaces() {
        assert_eq!(group_thousands(223_559), "223\u{2009}559");
        assert_eq!(group_thousands(1_000_000), "1\u{2009}000\u{2009}000");
        assert_eq!(group_thousands(999), "999");
    }

    #[test]
    fn even_ticks_span_the_range() {
        let ticks = even_ticks(1000.0, 6000.0, 4);
        assert_eq!(ticks.first().unwrap(), &(0.0, "1000".to_string()));
        assert_eq!(ticks.last().unwrap(), &(1.0, "7000".to_string()));
        assert_eq!(even_ticks(0.5, 1.0, 2)[1].1, "1.00");
    }
}
