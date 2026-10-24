//! The drawing surface the export sheet is laid out on. Two back ends implement it: PNG
//! (`png.rs`, vello_cpu) and PDF (`pdf.rs`, printpdf), so every export is drawn once.
//!
//! Units are points with the origin at the top-left and y growing down. Text positions are the
//! baseline.

use eframe::egui::{Color32, Pos2, Rect};

/// The faces an export uses (the same files as the UI, `assets/fonts/`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    /// Saira 800 italic: the sheet title.
    Display,
    /// Outfit 400: legend names.
    Ui,
    /// Outfit 600: emphasis.
    UiSemibold,
    /// JetBrains Mono 400: numbers, meta, footer, axis ticks.
    Mono,
}

impl Face {
    /// The bundled face name (`fonts::face_bytes`).
    pub fn font_name(self) -> &'static str {
        match self {
            Face::Display => "Saira-ExtraBoldItalic",
            Face::Ui => "Outfit-Regular",
            Face::UiSemibold => "Outfit-SemiBold",
            Face::Mono => "JetBrainsMono-Regular",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HAlign {
    Left,
    Center,
    Right,
}

pub trait Canvas {
    fn fill_rect(&mut self, rect: Rect, color: Color32);

    /// An open polyline; `dash` is `[on, off]` in points.
    fn polyline(&mut self, points: &[Pos2], width: f32, color: Color32, dash: Option<[f32; 2]>);

    /// Text with its baseline at `pos.y`, aligned to `pos.x` per `align`.
    fn text(&mut self, pos: Pos2, text: &str, face: Face, size: f32, color: Color32, align: HAlign);

    /// An RGBA image stretched over `rect`.
    fn image(&mut self, rect: Rect, image: &::image::RgbaImage);

    fn stroke_rect(&mut self, rect: Rect, width: f32, color: Color32) {
        let pts = [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
            rect.left_top(),
        ];
        self.polyline(&pts, width, color, None);
    }

    /// A horizontal gradient through `stops` (evenly spaced), drawn as thin slices so both
    /// back ends render it the same way.
    fn gradient_rect(&mut self, rect: Rect, color_at: &dyn Fn(f32) -> Color32) {
        let slices = 96;
        let w = rect.width() / slices as f32;
        for i in 0..slices {
            let k = (i as f32 + 0.5) / slices as f32;
            let r = Rect::from_min_size(
                rect.left_top() + eframe::egui::vec2(w * i as f32, 0.0),
                eframe::egui::vec2(w + 0.6, rect.height()),
            );
            self.fill_rect(r, color_at(k));
        }
    }
}

/// `color` composited over `background` (PDF has no cheap alpha; export colours are flattened).
pub fn flatten(color: Color32, background: Color32) -> Color32 {
    let a = color.a() as f32 / 255.0;
    if a >= 1.0 {
        return color;
    }
    let [r, g, b, _] = color.to_srgba_unmultiplied();
    let mix = |c: u8, bg: u8| (c as f32 * a + bg as f32 * (1.0 - a)).round() as u8;
    Color32::from_rgb(
        mix(r, background.r()),
        mix(g, background.g()),
        mix(b, background.b()),
    )
}
