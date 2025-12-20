//! Chart export functionality (PNG, PDF).

use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

// Use fully qualified path to disambiguate from printpdf's image module
use ::image::{Rgba, RgbaImage};

use crate::app::UltraLogApp;

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
            Ok(_) => self.show_toast("Chart exported as PNG"),
            Err(e) => self.show_toast(&format!("Export failed: {}", e)),
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
            Ok(_) => self.show_toast("Chart exported as PDF"),
            Err(e) => self.show_toast(&format!("Export failed: {}", e)),
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
        for selected in &self.selected_channels {
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
                self.selected_channels.len(),
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
        for selected in &self.selected_channels {
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

        for selected in &self.selected_channels {
            let color_rgb = self.get_channel_color(selected.color_index);
            let text_color = Color::Rgb(Rgb::new(
                color_rgb[0] as f32 / 255.0,
                color_rgb[1] as f32 / 255.0,
                color_rgb[2] as f32 / 255.0,
                None,
            ));

            current_layer.set_fill_color(text_color);
            current_layer.use_text(
                &selected.channel.name(),
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
