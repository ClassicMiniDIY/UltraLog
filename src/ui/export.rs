//! Chart export functionality (PNG, PDF).

use printpdf::path::{PaintMode, WindingOrder};
use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

// Use fully qualified path to disambiguate from printpdf's image module
use ::image::{Rgba, RgbaImage};

use crate::analytics;
use crate::app::UltraLogApp;
use crate::normalize::normalize_channel_name_with_custom;
use crate::state::HistogramMode;

impl UltraLogApp {
    /// Export the current chart view as PNG
    pub fn export_chart_png(&mut self) {
        // Show save dialog
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .set_file_name("ultralog_chart.png")
            .save_file()
        else {
            return;
        };

        // Create a simple chart representation as image
        match self.render_chart_to_png(&path) {
            Ok(_) => {
                analytics::track_export("png");
                self.show_toast_success("Chart exported as PNG");
            }
            Err(e) => self.show_toast_error(&format!("Export failed: {}", e)),
        }
    }

    /// Export the current chart view as PDF
    pub fn export_chart_pdf(&mut self) {
        // Show save dialog
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF Document", &["pdf"])
            .set_file_name("ultralog_chart.pdf")
            .save_file()
        else {
            return;
        };

        match self.render_chart_to_pdf(&path) {
            Ok(_) => {
                analytics::track_export("pdf");
                self.show_toast_success("Chart exported as PDF");
            }
            Err(e) => self.show_toast_error(&format!("Export failed: {}", e)),
        }
    }

    /// Render chart data to PNG file
    fn render_chart_to_png(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let width = 1920u32;
        let height = 1080u32;

        // Create image buffer
        let mut imgbuf = RgbaImage::new(width, height);

        // Fill with dark background
        for pixel in imgbuf.pixels_mut() {
            *pixel = Rgba([30, 30, 30, 255]);
        }

        // Draw chart area background
        let chart_left = 80u32;
        let chart_right = width - 40;
        let chart_top = 60u32;
        let chart_bottom = height - 80;

        for y in chart_top..chart_bottom {
            for x in chart_left..chart_right {
                imgbuf.put_pixel(x, y, Rgba([40, 40, 40, 255]));
            }
        }

        // Get time range
        let Some((min_time, max_time)) = self.time_range else {
            return Err("No time range available".into());
        };

        let time_span = max_time - min_time;
        if time_span <= 0.0 {
            return Err("Invalid time range".into());
        }

        let chart_width = (chart_right - chart_left) as f64;
        let chart_height = (chart_bottom - chart_top) as f64;

        // Draw each channel
        for selected in self.get_selected_channels() {
            let color = self.get_channel_color(selected.color_index);
            let pixel_color = Rgba([color[0], color[1], color[2], 255]);

            // Get channel data
            if selected.file_index >= self.files.len() {
                continue;
            }
            let file = &self.files[selected.file_index];
            let times = file.log.get_times_as_f64();
            let data = file.log.get_channel_data(selected.channel_index);

            if data.is_empty() {
                continue;
            }

            // Find min/max for normalization
            let mut data_min = f64::MAX;
            let mut data_max = f64::MIN;
            for &val in &data {
                data_min = data_min.min(val);
                data_max = data_max.max(val);
            }

            let data_range = if (data_max - data_min).abs() < 0.0001 {
                1.0
            } else {
                data_max - data_min
            };

            // Draw data points as lines
            let mut prev_x: Option<u32> = None;
            let mut prev_y: Option<u32> = None;

            for (&time, &value) in times.iter().zip(data.iter()) {
                // Skip points outside time range
                if time < min_time || time > max_time {
                    continue;
                }

                let x_ratio = (time - min_time) / time_span;
                let y_ratio = (value - data_min) / data_range;

                let x = chart_left + (x_ratio * chart_width) as u32;
                let y = chart_bottom - (y_ratio * chart_height) as u32;

                // Draw line from previous point
                if let (Some(px), Some(py)) = (prev_x, prev_y) {
                    draw_line(&mut imgbuf, px, py, x, y, pixel_color);
                }

                prev_x = Some(x);
                prev_y = Some(y);
            }
        }

        // Save the image
        imgbuf.save(path)?;

        Ok(())
    }

    /// Render chart data to PDF file
    fn render_chart_to_pdf(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create PDF document (A4 landscape)
        let (doc, page1, layer1) =
            PdfDocument::new("UltraLog Chart Export", Mm(297.0), Mm(210.0), "Chart");

        let current_layer = doc.get_page(page1).get_layer(layer1);

        // Get time range
        let Some((min_time, max_time)) = self.time_range else {
            return Err("No time range available".into());
        };

        let time_span = max_time - min_time;
        if time_span <= 0.0 {
            return Err("Invalid time range".into());
        }

        // Chart dimensions in mm (A4 landscape with margins)
        let margin: f64 = 20.0;
        let chart_left: f64 = margin;
        let chart_right: f64 = 297.0 - margin;
        let chart_bottom: f64 = margin + 20.0; // Leave room for time labels
        let chart_top: f64 = 210.0 - margin - 30.0; // Leave room for title

        let chart_width: f64 = chart_right - chart_left;
        let chart_height: f64 = chart_top - chart_bottom;

        // Draw title
        let font = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;
        current_layer.use_text(
            "UltraLog Chart Export",
            16.0,
            Mm(margin as f32),
            Mm(200.0),
            &font,
        );

        // Draw subtitle with file info
        let font_regular = doc.add_builtin_font(BuiltinFont::Helvetica)?;
        if let Some(file) = self.files.first() {
            let subtitle = format!(
                "{} | {} channels selected | Time: {:.1}s - {:.1}s",
                file.name,
                self.get_selected_channels().len(),
                min_time,
                max_time
            );
            current_layer.use_text(&subtitle, 10.0, Mm(margin as f32), Mm(192.0), &font_regular);
        }

        // Draw chart border
        let border_color = Color::Rgb(Rgb::new(0.3, 0.3, 0.3, None));
        current_layer.set_outline_color(border_color);
        current_layer.set_outline_thickness(0.5);

        let border = Line {
            points: vec![
                (
                    Point::new(Mm(chart_left as f32), Mm(chart_bottom as f32)),
                    false,
                ),
                (
                    Point::new(Mm(chart_right as f32), Mm(chart_bottom as f32)),
                    false,
                ),
                (
                    Point::new(Mm(chart_right as f32), Mm(chart_top as f32)),
                    false,
                ),
                (
                    Point::new(Mm(chart_left as f32), Mm(chart_top as f32)),
                    false,
                ),
            ],
            is_closed: true,
        };
        current_layer.add_line(border);

        // Draw each channel
        for selected in self.get_selected_channels() {
            let color_rgb = self.get_channel_color(selected.color_index);
            let line_color = Color::Rgb(Rgb::new(
                color_rgb[0] as f32 / 255.0,
                color_rgb[1] as f32 / 255.0,
                color_rgb[2] as f32 / 255.0,
                None,
            ));

            current_layer.set_outline_color(line_color);
            current_layer.set_outline_thickness(0.75);

            // Get channel data
            if selected.file_index >= self.files.len() {
                continue;
            }
            let file = &self.files[selected.file_index];
            let times = file.log.get_times_as_f64();
            let data = file.log.get_channel_data(selected.channel_index);

            if data.is_empty() {
                continue;
            }

            // Find min/max for normalization
            let mut data_min = f64::MAX;
            let mut data_max = f64::MIN;
            for &val in &data {
                data_min = data_min.min(val);
                data_max = data_max.max(val);
            }

            let data_range = if (data_max - data_min).abs() < 0.0001 {
                1.0
            } else {
                data_max - data_min
            };

            // Build line points (downsample for PDF)
            let mut points: Vec<(Point, bool)> = Vec::new();
            let step = (times.len() / 500).max(1); // Max ~500 points per channel

            for (i, (&time, &value)) in times.iter().zip(data.iter()).enumerate() {
                if i % step != 0 {
                    continue;
                }

                if time < min_time || time > max_time {
                    continue;
                }

                let x_ratio = (time - min_time) / time_span;
                let y_ratio = (value - data_min) / data_range;

                let x = chart_left + x_ratio * chart_width;
                let y = chart_bottom + y_ratio * chart_height;

                points.push((Point::new(Mm(x as f32), Mm(y as f32)), false));
            }

            if points.len() >= 2 {
                let line = Line {
                    points,
                    is_closed: false,
                };
                current_layer.add_line(line);
            }
        }

        // Draw legend
        let legend_y = chart_bottom - 12.0;
        let mut legend_x = chart_left;

        for selected in self.get_selected_channels() {
            let color_rgb = self.get_channel_color(selected.color_index);
            let text_color = Color::Rgb(Rgb::new(
                color_rgb[0] as f32 / 255.0,
                color_rgb[1] as f32 / 255.0,
                color_rgb[2] as f32 / 255.0,
                None,
            ));

            // Get display name (normalized or original based on setting)
            let channel_name = selected.channel.name();
            let display_name = if self.field_normalization {
                normalize_channel_name_with_custom(&channel_name, Some(&self.custom_normalizations))
            } else {
                channel_name
            };

            current_layer.set_fill_color(text_color);
            current_layer.use_text(
                &display_name,
                8.0,
                Mm(legend_x as f32),
                Mm(legend_y as f32),
                &font_regular,
            );

            legend_x += 40.0;
            if legend_x > chart_right - 40.0 {
                break; // Don't overflow
            }
        }

        // Save PDF
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        doc.save(&mut writer)?;

        Ok(())
    }

    /// Export the current histogram view as PNG
    pub fn export_histogram_png(&mut self) {
        // Show save dialog
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .set_file_name("ultralog_histogram.png")
            .save_file()
        else {
            return;
        };

        match self.render_histogram_to_png(&path) {
            Ok(_) => {
                analytics::track_export("histogram_png");
                self.show_toast_success("Histogram exported as PNG");
            }
            Err(e) => self.show_toast_error(&format!("Export failed: {}", e)),
        }
    }

    /// Export the current scatter plot view as PNG
    pub fn export_scatter_plot_png(&mut self) {
        // Show save dialog
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG Image", &["png"])
            .set_file_name("ultralog_scatter_plot.png")
            .save_file()
        else {
            return;
        };

        match self.render_scatter_plot_to_png(&path) {
            Ok(_) => {
                analytics::track_export("scatter_plot_png");
                self.show_toast_success("Scatter plot exported as PNG");
            }
            Err(e) => self.show_toast_error(&format!("Export failed: {}", e)),
        }
    }

    /// Export the current scatter plot view as PDF
    pub fn export_scatter_plot_pdf(&mut self) {
        // Show save dialog
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF Document", &["pdf"])
            .set_file_name("ultralog_scatter_plot.pdf")
            .save_file()
        else {
            return;
        };

        match self.render_scatter_plot_to_pdf(&path) {
            Ok(_) => {
                analytics::track_export("scatter_plot_pdf");
                self.show_toast_success("Scatter plot exported as PDF");
            }
            Err(e) => self.show_toast_error(&format!("Export failed: {}", e)),
        }
    }

    /// Export the current histogram view as PDF
    pub fn export_histogram_pdf(&mut self) {
        // Show save dialog
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF Document", &["pdf"])
            .set_file_name("ultralog_histogram.pdf")
            .save_file()
        else {
            return;
        };

        match self.render_histogram_to_pdf(&path) {
            Ok(_) => {
                analytics::track_export("histogram_pdf");
                self.show_toast_success("Histogram exported as PDF");
            }
            Err(e) => self.show_toast_error(&format!("Export failed: {}", e)),
        }
    }

    /// Render histogram to PDF file
    fn render_histogram_to_pdf(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get tab and file data
        let tab_idx = self.active_tab.ok_or("No active tab")?;
        let config = &self.tabs[tab_idx].histogram_state.config;
        let file_idx = self.tabs[tab_idx].file_index;

        if file_idx >= self.files.len() {
            return Err("Invalid file index".into());
        }

        let file = &self.files[file_idx];
        let mode = config.mode;
        let grid_size = config.grid_size.size();

        let x_idx = config.x_channel.ok_or("X axis not selected")?;
        let y_idx = config.y_channel.ok_or("Y axis not selected")?;
        let z_idx = if mode == HistogramMode::AverageZ {
            config
                .z_channel
                .ok_or("Z axis not selected for Average mode")?
        } else {
            0 // unused
        };

        // Get channel data
        let x_data = file.log.get_channel_data(x_idx);
        let y_data = file.log.get_channel_data(y_idx);
        let z_data = if mode == HistogramMode::AverageZ {
            Some(file.log.get_channel_data(z_idx))
        } else {
            None
        };

        if x_data.is_empty() || y_data.is_empty() {
            return Err("No data available".into());
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
        let mut hit_counts = vec![vec![0u32; grid_size]; grid_size];
        let mut z_sums = vec![vec![0.0f64; grid_size]; grid_size];

        for i in 0..x_data.len() {
            let x_bin = (((x_data[i] - x_min) / x_range) * (grid_size - 1) as f64).round() as usize;
            let y_bin = (((y_data[i] - y_min) / y_range) * (grid_size - 1) as f64).round() as usize;
            let x_bin = x_bin.min(grid_size - 1);
            let y_bin = y_bin.min(grid_size - 1);

            hit_counts[y_bin][x_bin] += 1;
            if let Some(ref z) = z_data {
                z_sums[y_bin][x_bin] += z[i];
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

        let value_range = if (max_value - min_value).abs() < f64::EPSILON {
            1.0
        } else {
            max_value - min_value
        };

        // Get channel names
        let x_name = file.log.channels[x_idx].name();
        let y_name = file.log.channels[y_idx].name();
        let z_name = if mode == HistogramMode::AverageZ {
            file.log.channels[z_idx].name()
        } else {
            "Hit Count".to_string()
        };

        // Create PDF document (A4 landscape)
        let (doc, page1, layer1) = PdfDocument::new(
            "UltraLog Histogram Export",
            Mm(297.0),
            Mm(210.0),
            "Histogram",
        );

        let current_layer = doc.get_page(page1).get_layer(layer1);

        // Chart dimensions in mm (A4 landscape with margins)
        let margin: f64 = 20.0;
        let axis_margin: f64 = 25.0;
        let chart_left: f64 = margin + axis_margin;
        let chart_right: f64 = 250.0; // Leave room for legend
        let chart_bottom: f64 = margin + axis_margin;
        let chart_top: f64 = 210.0 - margin - 30.0;

        let chart_width: f64 = chart_right - chart_left;
        let chart_height: f64 = chart_top - chart_bottom;

        let cell_width = chart_width / grid_size as f64;
        let cell_height = chart_height / grid_size as f64;

        // Draw title
        let font = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;
        current_layer.use_text(
            "UltraLog Histogram Export",
            16.0,
            Mm(margin as f32),
            Mm(200.0),
            &font,
        );

        // Draw subtitle
        let font_regular = doc.add_builtin_font(BuiltinFont::Helvetica)?;
        let subtitle = format!(
            "{} | Grid: {}x{} | Mode: {}",
            file.name,
            grid_size,
            grid_size,
            if mode == HistogramMode::HitCount {
                "Hit Count"
            } else {
                "Average Z"
            }
        );
        current_layer.use_text(&subtitle, 10.0, Mm(margin as f32), Mm(192.0), &font_regular);

        // Draw axis labels
        let axis_subtitle = format!("X: {} | Y: {} | Z: {}", x_name, y_name, z_name);
        current_layer.use_text(
            &axis_subtitle,
            9.0,
            Mm(margin as f32),
            Mm(186.0),
            &font_regular,
        );

        // Draw histogram cells
        for y_bin in 0..grid_size {
            for x_bin in 0..grid_size {
                let cell_x = chart_left + x_bin as f64 * cell_width;
                let cell_y = chart_bottom + y_bin as f64 * cell_height;

                if let Some(value) = cell_values[y_bin][x_bin] {
                    // Calculate color
                    let normalized = if mode == HistogramMode::HitCount && max_value > 1.0 {
                        (value.ln() / max_value.ln()).clamp(0.0, 1.0)
                    } else {
                        ((value - min_value) / value_range).clamp(0.0, 1.0)
                    };
                    let color = Self::get_pdf_heat_color(normalized);

                    current_layer.set_fill_color(color);

                    // Draw filled rectangle
                    let rect = printpdf::Polygon {
                        rings: vec![vec![
                            (Point::new(Mm(cell_x as f32), Mm(cell_y as f32)), false),
                            (
                                Point::new(Mm((cell_x + cell_width) as f32), Mm(cell_y as f32)),
                                false,
                            ),
                            (
                                Point::new(
                                    Mm((cell_x + cell_width) as f32),
                                    Mm((cell_y + cell_height) as f32),
                                ),
                                false,
                            ),
                            (
                                Point::new(Mm(cell_x as f32), Mm((cell_y + cell_height) as f32)),
                                false,
                            ),
                        ]],
                        mode: PaintMode::Fill,
                        winding_order: WindingOrder::NonZero,
                    };
                    current_layer.add_polygon(rect);

                    // Draw cell value text (only for smaller grids)
                    if grid_size <= 32 {
                        let text = if mode == HistogramMode::HitCount {
                            format!("{}", hit_counts[y_bin][x_bin])
                        } else {
                            format!("{:.1}", value)
                        };

                        // Calculate text color based on brightness
                        let brightness = normalized;
                        let text_color = if brightness > 0.5 {
                            Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)) // Black
                        } else {
                            Color::Rgb(Rgb::new(1.0, 1.0, 1.0, None)) // White
                        };

                        current_layer.set_fill_color(text_color);
                        let font_size = if grid_size <= 16 { 6.0 } else { 4.0 };
                        current_layer.use_text(
                            &text,
                            font_size,
                            Mm((cell_x + cell_width / 2.0 - 2.0) as f32),
                            Mm((cell_y + cell_height / 2.0 - 1.0) as f32),
                            &font_regular,
                        );
                    }
                }
            }
        }

        // Draw grid lines
        let grid_color = Color::Rgb(Rgb::new(0.4, 0.4, 0.4, None));
        current_layer.set_outline_color(grid_color);
        current_layer.set_outline_thickness(0.25);

        for i in 0..=grid_size {
            let x = chart_left + i as f64 * cell_width;
            let y = chart_bottom + i as f64 * cell_height;

            // Vertical line
            let vline = Line {
                points: vec![
                    (Point::new(Mm(x as f32), Mm(chart_bottom as f32)), false),
                    (Point::new(Mm(x as f32), Mm(chart_top as f32)), false),
                ],
                is_closed: false,
            };
            current_layer.add_line(vline);

            // Horizontal line
            let hline = Line {
                points: vec![
                    (Point::new(Mm(chart_left as f32), Mm(y as f32)), false),
                    (Point::new(Mm(chart_right as f32), Mm(y as f32)), false),
                ],
                is_closed: false,
            };
            current_layer.add_line(hline);
        }

        // Draw axis value labels
        let label_color = Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None));
        current_layer.set_fill_color(label_color);

        // Y axis labels
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let value = y_min + t * y_range;
            let y_pos = chart_bottom + t * chart_height;
            current_layer.use_text(
                format!("{:.0}", value),
                7.0,
                Mm((chart_left - 12.0) as f32),
                Mm((y_pos - 1.0) as f32),
                &font_regular,
            );
        }

        // X axis labels
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let value = x_min + t * x_range;
            let x_pos = chart_left + t * chart_width;
            current_layer.use_text(
                format!("{:.0}", value),
                7.0,
                Mm((x_pos - 4.0) as f32),
                Mm((chart_bottom - 8.0) as f32),
                &font_regular,
            );
        }

        // Draw legend (color scale)
        let legend_left: f64 = 260.0;
        let legend_width: f64 = 15.0;
        let legend_bottom: f64 = chart_bottom;
        let legend_height: f64 = chart_height;

        // Draw legend title
        current_layer.set_fill_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
        let legend_title = if mode == HistogramMode::HitCount {
            "Hits"
        } else {
            "Value"
        };
        current_layer.use_text(
            legend_title,
            9.0,
            Mm(legend_left as f32),
            Mm((legend_bottom + legend_height + 5.0) as f32),
            &font,
        );

        // Draw color gradient bar
        let gradient_steps = 30;
        let step_height = legend_height / gradient_steps as f64;

        for i in 0..gradient_steps {
            let t = i as f64 / gradient_steps as f64;
            let color = Self::get_pdf_heat_color(t);
            current_layer.set_fill_color(color);

            let y = legend_bottom + i as f64 * step_height;
            let rect = printpdf::Polygon {
                rings: vec![vec![
                    (Point::new(Mm(legend_left as f32), Mm(y as f32)), false),
                    (
                        Point::new(Mm((legend_left + legend_width) as f32), Mm(y as f32)),
                        false,
                    ),
                    (
                        Point::new(
                            Mm((legend_left + legend_width) as f32),
                            Mm((y + step_height + 0.5) as f32),
                        ),
                        false,
                    ),
                    (
                        Point::new(Mm(legend_left as f32), Mm((y + step_height + 0.5) as f32)),
                        false,
                    ),
                ]],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            };
            current_layer.add_polygon(rect);
        }

        // Draw legend min/max labels
        current_layer.set_fill_color(Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None)));
        let min_label = if mode == HistogramMode::HitCount {
            "0".to_string()
        } else {
            format!("{:.1}", min_value)
        };
        let max_label = if mode == HistogramMode::HitCount {
            format!("{:.0}", max_value)
        } else {
            format!("{:.1}", max_value)
        };

        current_layer.use_text(
            &min_label,
            7.0,
            Mm((legend_left + legend_width + 3.0) as f32),
            Mm(legend_bottom as f32),
            &font_regular,
        );
        current_layer.use_text(
            &max_label,
            7.0,
            Mm((legend_left + legend_width + 3.0) as f32),
            Mm((legend_bottom + legend_height - 3.0) as f32),
            &font_regular,
        );

        // Draw statistics
        let stats_y = legend_bottom - 15.0;
        current_layer.use_text(
            format!("Total Points: {}", x_data.len()),
            8.0,
            Mm(legend_left as f32),
            Mm(stats_y as f32),
            &font_regular,
        );

        // Save PDF
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        doc.save(&mut writer)?;

        Ok(())
    }

    /// Get a PDF color from the heat map gradient based on normalized value (0-1)
    fn get_pdf_heat_color(normalized: f64) -> Color {
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

        let t = normalized.clamp(0.0, 1.0);
        let scaled = t * (HEAT_COLORS.len() - 1) as f64;
        let idx = scaled.floor() as usize;
        let frac = scaled - idx as f64;

        if idx >= HEAT_COLORS.len() - 1 {
            let c = HEAT_COLORS[HEAT_COLORS.len() - 1];
            return Color::Rgb(Rgb::new(
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
                None,
            ));
        }

        let c1 = HEAT_COLORS[idx];
        let c2 = HEAT_COLORS[idx + 1];

        let r = (c1[0] as f64 + (c2[0] as f64 - c1[0] as f64) * frac) / 255.0;
        let g = (c1[1] as f64 + (c2[1] as f64 - c1[1] as f64) * frac) / 255.0;
        let b = (c1[2] as f64 + (c2[2] as f64 - c1[2] as f64) * frac) / 255.0;

        Color::Rgb(Rgb::new(r as f32, g as f32, b as f32, None))
    }

    /// Get a PNG color from the heat map gradient based on normalized value (0-1)
    fn get_png_heat_color(normalized: f64) -> Rgba<u8> {
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

        let t = normalized.clamp(0.0, 1.0);
        let scaled = t * (HEAT_COLORS.len() - 1) as f64;
        let idx = scaled.floor() as usize;
        let frac = scaled - idx as f64;

        if idx >= HEAT_COLORS.len() - 1 {
            let c = HEAT_COLORS[HEAT_COLORS.len() - 1];
            return Rgba([c[0], c[1], c[2], 255]);
        }

        let c1 = HEAT_COLORS[idx];
        let c2 = HEAT_COLORS[idx + 1];

        let r = (c1[0] as f64 + (c2[0] as f64 - c1[0] as f64) * frac) as u8;
        let g = (c1[1] as f64 + (c2[1] as f64 - c1[1] as f64) * frac) as u8;
        let b = (c1[2] as f64 + (c2[2] as f64 - c1[2] as f64) * frac) as u8;

        Rgba([r, g, b, 255])
    }

    /// Render histogram to PNG file
    fn render_histogram_to_png(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Get tab and file data
        let tab_idx = self.active_tab.ok_or("No active tab")?;
        let config = &self.tabs[tab_idx].histogram_state.config;
        let file_idx = self.tabs[tab_idx].file_index;

        if file_idx >= self.files.len() {
            return Err("Invalid file index".into());
        }

        let file = &self.files[file_idx];
        let mode = config.mode;
        let grid_size = config.grid_size.size();

        let x_idx = config.x_channel.ok_or("X axis not selected")?;
        let y_idx = config.y_channel.ok_or("Y axis not selected")?;
        let z_idx = if mode == HistogramMode::AverageZ {
            config
                .z_channel
                .ok_or("Z axis not selected for Average mode")?
        } else {
            0 // unused
        };

        // Get channel data
        let x_data = file.log.get_channel_data(x_idx);
        let y_data = file.log.get_channel_data(y_idx);
        let z_data = if mode == HistogramMode::AverageZ {
            Some(file.log.get_channel_data(z_idx))
        } else {
            None
        };

        if x_data.is_empty() || y_data.is_empty() {
            return Err("No data available".into());
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
        let mut hit_counts = vec![vec![0u32; grid_size]; grid_size];
        let mut z_sums = vec![vec![0.0f64; grid_size]; grid_size];

        for i in 0..x_data.len() {
            let x_bin = (((x_data[i] - x_min) / x_range) * (grid_size - 1) as f64).round() as usize;
            let y_bin = (((y_data[i] - y_min) / y_range) * (grid_size - 1) as f64).round() as usize;
            let x_bin = x_bin.min(grid_size - 1);
            let y_bin = y_bin.min(grid_size - 1);

            hit_counts[y_bin][x_bin] += 1;
            if let Some(ref z) = z_data {
                z_sums[y_bin][x_bin] += z[i];
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

        let value_range = if (max_value - min_value).abs() < f64::EPSILON {
            1.0
        } else {
            max_value - min_value
        };

        // Image dimensions
        let width = 1920u32;
        let height = 1080u32;
        let margin = 80u32;
        let legend_width = 100u32;

        let chart_left = margin;
        let chart_right = width - margin - legend_width;
        let chart_top = margin;
        let chart_bottom = height - margin;

        let chart_width = chart_right - chart_left;
        let chart_height = chart_bottom - chart_top;
        let cell_width = chart_width as f64 / grid_size as f64;
        let cell_height = chart_height as f64 / grid_size as f64;

        // Create image buffer
        let mut imgbuf = RgbaImage::new(width, height);

        // Fill with dark background
        for pixel in imgbuf.pixels_mut() {
            *pixel = Rgba([30, 30, 30, 255]);
        }

        // Draw histogram cells
        #[allow(clippy::needless_range_loop)]
        for y_bin in 0..grid_size {
            for x_bin in 0..grid_size {
                if let Some(value) = cell_values[y_bin][x_bin] {
                    // Calculate color
                    let normalized = if mode == HistogramMode::HitCount && max_value > 1.0 {
                        (value.ln() / max_value.ln()).clamp(0.0, 1.0)
                    } else {
                        ((value - min_value) / value_range).clamp(0.0, 1.0)
                    };
                    let color = Self::get_png_heat_color(normalized);

                    // Calculate cell position (Y inverted - higher values at top)
                    let cell_x = chart_left as f64 + x_bin as f64 * cell_width;
                    let cell_y = chart_bottom as f64 - (y_bin + 1) as f64 * cell_height;

                    // Fill cell
                    for py in 0..(cell_height.ceil() as u32) {
                        for px in 0..(cell_width.ceil() as u32) {
                            let x = (cell_x as u32 + px).min(width - 1);
                            let y = (cell_y as u32 + py).min(height - 1);
                            imgbuf.put_pixel(x, y, color);
                        }
                    }
                }
            }
        }

        // Draw grid lines
        let grid_color = Rgba([80, 80, 80, 255]);
        for i in 0..=grid_size {
            let x = chart_left + (i as f64 * cell_width) as u32;
            let y = chart_top + (i as f64 * cell_height) as u32;

            // Vertical line
            for py in chart_top..chart_bottom {
                if x < width {
                    imgbuf.put_pixel(x, py, grid_color);
                }
            }

            // Horizontal line
            for px in chart_left..chart_right {
                if y < height {
                    imgbuf.put_pixel(px, y, grid_color);
                }
            }
        }

        // Draw color scale legend
        let legend_left = width - margin - legend_width + 20;
        let legend_bar_width = 20u32;
        let legend_height = chart_height;

        for i in 0..legend_height {
            let t = i as f64 / legend_height as f64;
            let color = Self::get_png_heat_color(t);

            for px in 0..legend_bar_width {
                let x = legend_left + px;
                let y = chart_bottom - i;
                if x < width && y < height {
                    imgbuf.put_pixel(x, y, color);
                }
            }
        }

        // Save the image
        imgbuf.save(path)?;

        Ok(())
    }

    /// Render scatter plot to PNG file (exports both left and right plots)
    fn render_scatter_plot_to_png(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tab_idx = self.active_tab.ok_or("No active tab")?;

        // Image dimensions (wider to fit two plots)
        let width = 2560u32;
        let height = 1080u32;
        let margin = 60u32;
        let gap = 40u32;

        let plot_width = (width - 2 * margin - gap) / 2;
        let plot_height = height - 2 * margin;

        // Create image buffer
        let mut imgbuf = RgbaImage::new(width, height);

        // Fill with dark background
        for pixel in imgbuf.pixels_mut() {
            *pixel = Rgba([30, 30, 30, 255]);
        }

        // Render left plot
        let left_config = &self.tabs[tab_idx].scatter_plot_state.left;
        let left_rect = (margin, margin, plot_width, plot_height);
        self.render_scatter_plot_to_image(&mut imgbuf, left_config, left_rect, tab_idx)?;

        // Render right plot
        let right_config = &self.tabs[tab_idx].scatter_plot_state.right;
        let right_rect = (margin + plot_width + gap, margin, plot_width, plot_height);
        self.render_scatter_plot_to_image(&mut imgbuf, right_config, right_rect, tab_idx)?;

        // Save the image
        imgbuf.save(path)?;

        Ok(())
    }

    /// Helper to render a single scatter plot to an image region
    fn render_scatter_plot_to_image(
        &self,
        imgbuf: &mut RgbaImage,
        config: &crate::state::ScatterPlotConfig,
        rect: (u32, u32, u32, u32),
        tab_idx: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (left, top, width, height) = rect;
        let right = left + width;
        let bottom = top + height;

        let file_idx = config.file_index.unwrap_or(self.tabs[tab_idx].file_index);

        // Check if we have valid axis selections
        let (x_idx, y_idx) = match (config.x_channel, config.y_channel) {
            (Some(x), Some(y)) => (x, y),
            _ => {
                // Draw placeholder
                let placeholder_color = Rgba([80, 80, 80, 255]);
                for y in top..bottom {
                    for x in left..right {
                        imgbuf.put_pixel(x, y, placeholder_color);
                    }
                }
                return Ok(());
            }
        };

        if file_idx >= self.files.len() {
            return Err("Invalid file index".into());
        }

        let file = &self.files[file_idx];
        let x_data = file.log.get_channel_data(x_idx);
        let y_data = file.log.get_channel_data(y_idx);

        if x_data.is_empty() || y_data.is_empty() || x_data.len() != y_data.len() {
            return Err("No data available".into());
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

        // Build 2D histogram (512 bins like the UI)
        const HEATMAP_BINS: usize = 512;
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

        // Fill background with black
        let bg_color = Rgba([0, 0, 0, 255]);
        for y in top..bottom {
            for x in left..right {
                imgbuf.put_pixel(x, y, bg_color);
            }
        }

        let cell_width = width as f64 / HEATMAP_BINS as f64;
        let cell_height = height as f64 / HEATMAP_BINS as f64;

        // Draw heatmap cells
        #[allow(clippy::needless_range_loop)]
        for y_bin in 0..HEATMAP_BINS {
            for x_bin in 0..HEATMAP_BINS {
                let hits = histogram[y_bin][x_bin];
                if hits > 0 {
                    // Normalize using log scale
                    let normalized = if max_hits > 1 {
                        (hits as f64).ln() / (max_hits as f64).ln()
                    } else {
                        1.0
                    };
                    let color = Self::get_png_heat_color(normalized);

                    // Calculate cell position (Y inverted)
                    let cell_x = left as f64 + x_bin as f64 * cell_width;
                    let cell_y = bottom as f64 - (y_bin + 1) as f64 * cell_height;

                    // Fill cell
                    for py in 0..(cell_height.ceil() as u32 + 1) {
                        for px in 0..(cell_width.ceil() as u32 + 1) {
                            let x = (cell_x as u32 + px).min(right - 1);
                            let y = (cell_y as u32 + py).min(bottom - 1);
                            if x >= left && y >= top {
                                imgbuf.put_pixel(x, y, color);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Render scatter plot to PDF file
    fn render_scatter_plot_to_pdf(
        &self,
        path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tab_idx = self.active_tab.ok_or("No active tab")?;

        // Create PDF document (A4 landscape)
        let (doc, page1, layer1) = PdfDocument::new(
            "UltraLog Scatter Plot Export",
            Mm(297.0),
            Mm(210.0),
            "Scatter Plot",
        );

        let current_layer = doc.get_page(page1).get_layer(layer1);

        // Draw title
        let font = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;
        let font_regular = doc.add_builtin_font(BuiltinFont::Helvetica)?;

        current_layer.use_text(
            "UltraLog Scatter Plot Export",
            16.0,
            Mm(20.0),
            Mm(200.0),
            &font,
        );

        // Subtitle with file info
        if let Some(file_idx) = self.selected_file {
            if file_idx < self.files.len() {
                let file = &self.files[file_idx];
                current_layer.use_text(&file.name, 10.0, Mm(20.0), Mm(192.0), &font_regular);
            }
        }

        // Layout: two plots side by side
        let margin: f64 = 20.0;
        let gap: f64 = 15.0;
        let plot_width: f64 = (297.0 - 2.0 * margin - gap) / 2.0;
        let plot_height: f64 = 150.0;
        let plot_top: f64 = 180.0;
        let plot_bottom: f64 = plot_top - plot_height;

        // Render left plot
        let left_config = &self.tabs[tab_idx].scatter_plot_state.left;
        let left_rect = (margin, plot_bottom, plot_width, plot_height);
        self.render_scatter_plot_to_pdf_region(
            &current_layer,
            &font_regular,
            left_config,
            left_rect,
            tab_idx,
        )?;

        // Render right plot
        let right_config = &self.tabs[tab_idx].scatter_plot_state.right;
        let right_rect = (
            margin + plot_width + gap,
            plot_bottom,
            plot_width,
            plot_height,
        );
        self.render_scatter_plot_to_pdf_region(
            &current_layer,
            &font_regular,
            right_config,
            right_rect,
            tab_idx,
        )?;

        // Save PDF
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        doc.save(&mut writer)?;

        Ok(())
    }

    /// Helper to render a single scatter plot to a PDF region
    fn render_scatter_plot_to_pdf_region(
        &self,
        layer: &printpdf::PdfLayerReference,
        font: &printpdf::IndirectFontRef,
        config: &crate::state::ScatterPlotConfig,
        rect: (f64, f64, f64, f64),
        tab_idx: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (left, bottom, width, height) = rect;
        let right = left + width;
        let top = bottom + height;

        let file_idx = config.file_index.unwrap_or(self.tabs[tab_idx].file_index);

        // Check if we have valid axis selections
        let (x_idx, y_idx) = match (config.x_channel, config.y_channel) {
            (Some(x), Some(y)) => (x, y),
            _ => {
                // Draw placeholder text
                layer.use_text(
                    "No axes selected",
                    10.0,
                    Mm((left + width / 2.0 - 15.0) as f32),
                    Mm((bottom + height / 2.0) as f32),
                    font,
                );
                return Ok(());
            }
        };

        if file_idx >= self.files.len() {
            return Err("Invalid file index".into());
        }

        let file = &self.files[file_idx];
        let x_data = file.log.get_channel_data(x_idx);
        let y_data = file.log.get_channel_data(y_idx);

        if x_data.is_empty() || y_data.is_empty() || x_data.len() != y_data.len() {
            return Err("No data available".into());
        }

        // Get channel names for labels
        let x_name = file.log.channels[x_idx].name();
        let y_name = file.log.channels[y_idx].name();

        // Draw axis labels
        layer.use_text(
            format!("{} vs {}", y_name, x_name),
            9.0,
            Mm(left as f32),
            Mm((top + 3.0) as f32),
            font,
        );

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

        // Build 2D histogram (use smaller bins for PDF)
        const PDF_BINS: usize = 64;
        let mut histogram = vec![vec![0u32; PDF_BINS]; PDF_BINS];
        let mut max_hits: u32 = 0;

        for (&x, &y) in x_data.iter().zip(y_data.iter()) {
            let x_bin = (((x - x_min) / x_range) * (PDF_BINS - 1) as f64).round() as usize;
            let y_bin = (((y - y_min) / y_range) * (PDF_BINS - 1) as f64).round() as usize;

            let x_bin = x_bin.min(PDF_BINS - 1);
            let y_bin = y_bin.min(PDF_BINS - 1);

            histogram[y_bin][x_bin] += 1;
            max_hits = max_hits.max(histogram[y_bin][x_bin]);
        }

        let cell_width = width / PDF_BINS as f64;
        let cell_height = height / PDF_BINS as f64;

        // Draw heatmap cells
        #[allow(clippy::needless_range_loop)]
        for y_bin in 0..PDF_BINS {
            for x_bin in 0..PDF_BINS {
                let hits = histogram[y_bin][x_bin];
                if hits > 0 {
                    // Normalize using log scale
                    let normalized = if max_hits > 1 {
                        (hits as f64).ln() / (max_hits as f64).ln()
                    } else {
                        1.0
                    };
                    let color = Self::get_pdf_heat_color(normalized);

                    layer.set_fill_color(color);

                    let cell_x = left + x_bin as f64 * cell_width;
                    let cell_y = bottom + y_bin as f64 * cell_height;

                    let rect = printpdf::Polygon {
                        rings: vec![vec![
                            (Point::new(Mm(cell_x as f32), Mm(cell_y as f32)), false),
                            (
                                Point::new(Mm((cell_x + cell_width) as f32), Mm(cell_y as f32)),
                                false,
                            ),
                            (
                                Point::new(
                                    Mm((cell_x + cell_width) as f32),
                                    Mm((cell_y + cell_height) as f32),
                                ),
                                false,
                            ),
                            (
                                Point::new(Mm(cell_x as f32), Mm((cell_y + cell_height) as f32)),
                                false,
                            ),
                        ]],
                        mode: PaintMode::Fill,
                        winding_order: WindingOrder::NonZero,
                    };
                    layer.add_polygon(rect);
                }
            }
        }

        // Draw border
        let border_color = Color::Rgb(Rgb::new(0.5, 0.5, 0.5, None));
        layer.set_outline_color(border_color);
        layer.set_outline_thickness(0.5);

        let border = Line {
            points: vec![
                (Point::new(Mm(left as f32), Mm(bottom as f32)), false),
                (Point::new(Mm(right as f32), Mm(bottom as f32)), false),
                (Point::new(Mm(right as f32), Mm(top as f32)), false),
                (Point::new(Mm(left as f32), Mm(top as f32)), false),
            ],
            is_closed: true,
        };
        layer.add_line(border);

        // Draw axis labels
        let label_color = Color::Rgb(Rgb::new(0.0, 0.0, 0.0, None));
        layer.set_fill_color(label_color);

        // X axis labels
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let value = x_min + t * x_range;
            let x_pos = left + t * width;
            layer.use_text(
                format!("{:.0}", value),
                6.0,
                Mm((x_pos - 3.0) as f32),
                Mm((bottom - 5.0) as f32),
                font,
            );
        }

        // Y axis labels
        for i in 0..=4 {
            let t = i as f64 / 4.0;
            let value = y_min + t * y_range;
            let y_pos = bottom + t * height;
            layer.use_text(
                format!("{:.0}", value),
                6.0,
                Mm((left - 10.0) as f32),
                Mm((y_pos - 1.0) as f32),
                font,
            );
        }

        Ok(())
    }
}

/// Draw a line between two points using Bresenham's algorithm
fn draw_line(img: &mut RgbaImage, x0: u32, y0: u32, x1: u32, y1: u32, color: Rgba<u8>) {
    let dx = (x1 as i32 - x0 as i32).abs();
    let dy = -(y1 as i32 - y0 as i32).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    let mut x = x0 as i32;
    let mut y = y0 as i32;

    let (width, height) = img.dimensions();

    loop {
        if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
            img.put_pixel(x as u32, y as u32, color);
        }

        if x == x1 as i32 && y == y1 as i32 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}
