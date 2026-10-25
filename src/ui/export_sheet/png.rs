//! PNG back end: vello_cpu rasterizes rectangles, strokes and glyph outlines; images (the
//! lockup) are composited after rendering.

use ::image::{Rgba, RgbaImage};
use eframe::egui::{Color32, Pos2, Rect};
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::{self, Affine, BezPath, Stroke};
use vello_cpu::{Pixmap, RenderContext, Resources};

use super::canvas::{Canvas, Face, HAlign};
use super::text;

pub struct PngCanvas {
    ctx: RenderContext,
    width: u16,
    height: u16,
    scale: f32,
    images: Vec<(Rect, RgbaImage)>,
}

fn paint(c: Color32) -> AlphaColor<Srgb> {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    AlphaColor::from_rgba8(r, g, b, a)
}

impl PngCanvas {
    /// A canvas of `width` × `height` points rendered at `scale` pixels per point.
    pub fn new(width: f32, height: f32, scale: f32) -> Self {
        let w = (width * scale).round() as u16;
        let h = (height * scale).round() as u16;
        let mut ctx = RenderContext::new(w, h);
        ctx.set_transform(Affine::scale(scale as f64));
        Self {
            ctx,
            width: w,
            height: h,
            scale,
            images: Vec::new(),
        }
    }

    /// Render and return straight-alpha RGBA pixels.
    pub fn finish(mut self) -> RgbaImage {
        let mut pixmap = Pixmap::new(self.width, self.height);
        self.ctx.flush();
        self.ctx.render(&mut pixmap, &mut Resources::new());
        let data = pixmap.data_as_u8_slice();
        let mut out = RgbaImage::new(self.width as u32, self.height as u32);
        for (px, chunk) in out.pixels_mut().zip(data.chunks_exact(4)) {
            let a = chunk[3];
            let un = |c: u8| {
                if a == 0 {
                    0
                } else {
                    ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8
                }
            };
            *px = Rgba([un(chunk[0]), un(chunk[1]), un(chunk[2]), a]);
        }
        for (rect, image) in &self.images {
            composite(&mut out, image, *rect, self.scale);
        }
        out
    }
}

/// Alpha-blend `src` into `dst` over `rect` (points), resampled with a Catmull-Rom filter so
/// the lockup's thin strokes survive the downscale.
fn composite(dst: &mut RgbaImage, src: &RgbaImage, rect: Rect, scale: f32) {
    let x0 = (rect.left() * scale).round() as i64;
    let y0 = (rect.top() * scale).round() as i64;
    let w = (rect.width() * scale).round().max(1.0) as i64;
    let h = (rect.height() * scale).round().max(1.0) as i64;
    let resized = ::image::imageops::resize(
        src,
        w as u32,
        h as u32,
        ::image::imageops::FilterType::CatmullRom,
    );
    let src = &resized;
    for dy in 0..h {
        for dx in 0..w {
            let (x, y) = (x0 + dx, y0 + dy);
            if x < 0 || y < 0 || x >= dst.width() as i64 || y >= dst.height() as i64 {
                continue;
            }
            let sx = (dx * src.width() as i64 / w) as u32;
            let sy = (dy * src.height() as i64 / h) as u32;
            let s = src.get_pixel(sx.min(src.width() - 1), sy.min(src.height() - 1));
            let a = s[3] as f32 / 255.0;
            let d = dst.get_pixel_mut(x as u32, y as u32);
            for c in 0..3 {
                d[c] = (s[c] as f32 * a + d[c] as f32 * (1.0 - a)).round() as u8;
            }
            d[3] = ((a + d[3] as f32 / 255.0 * (1.0 - a)) * 255.0).round() as u8;
        }
    }
}

impl Canvas for PngCanvas {
    fn fill_rect(&mut self, rect: Rect, color: Color32) {
        self.ctx.set_paint(paint(color));
        self.ctx.fill_rect(&kurbo::Rect::new(
            rect.left() as f64,
            rect.top() as f64,
            rect.right() as f64,
            rect.bottom() as f64,
        ));
    }

    fn polyline(&mut self, points: &[Pos2], width: f32, color: Color32, dash: Option<[f32; 2]>) {
        if points.len() < 2 {
            return;
        }
        let mut path = BezPath::new();
        path.move_to((points[0].x as f64, points[0].y as f64));
        for p in &points[1..] {
            path.line_to((p.x as f64, p.y as f64));
        }
        let mut stroke = Stroke::new(width as f64);
        if let Some([on, off]) = dash {
            stroke = stroke.with_dashes(0.0, [on as f64, off as f64]);
        }
        self.ctx.set_stroke(stroke);
        self.ctx.set_paint(paint(color));
        self.ctx.stroke_path(&path);
    }

    fn text(&mut self, pos: Pos2, s: &str, face: Face, size: f32, color: Color32, align: HAlign) {
        let (runs, width) = text::layout(s, face, size);
        let x = match align {
            HAlign::Left => pos.x,
            HAlign::Center => pos.x - width / 2.0,
            HAlign::Right => pos.x - width,
        };
        let mut path = BezPath::new();
        text::outlines(&runs, size, x, pos.y, &mut path);
        self.ctx.set_paint(paint(color));
        self.ctx.fill_path(&path);
    }

    fn image(&mut self, rect: Rect, image: &RgbaImage) {
        self.images.push((rect, image.clone()));
    }
}
