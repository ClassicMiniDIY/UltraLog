//! Text layout for exports: glyph lookup, advances and outlines from the bundled fonts with
//! skrifa. Layout is plain left-to-right with per-glyph advances (no kerning or shaping), which
//! suits the labels, numbers and names an export prints. A character missing from the chosen
//! face falls back to JetBrains Mono (Greek, Cyrillic) and the Noto script faces.

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use vello_cpu::kurbo::BezPath;

use super::canvas::Face;
use crate::ui::fonts;

const FALLBACKS: [&str; 4] = [
    "JetBrainsMono-Regular",
    "NotoSansDevanagari",
    "NotoSansBengali",
    "NotoSansArabic",
];

/// Glyphs that share one font, starting at `x` (points from the text start).
pub struct Run {
    pub font_name: &'static str,
    pub font: &'static [u8],
    pub text: String,
    pub glyphs: Vec<(GlyphId, f32)>,
    pub x: f32,
}

fn font_ref(bytes: &'static [u8]) -> Option<FontRef<'static>> {
    FontRef::new(bytes).ok()
}

/// Lay out `text` at `size` points: runs by font, and the total advance width.
pub fn layout(text: &str, face: Face, size: f32) -> (Vec<Run>, f32) {
    let chain: Vec<(&'static str, &'static [u8], FontRef<'static>)> =
        std::iter::once(face.font_name())
            .chain(FALLBACKS)
            .filter_map(|name| {
                let bytes = fonts::face_bytes(name)?;
                Some((name, bytes, font_ref(bytes)?))
            })
            .collect();
    let mut runs: Vec<Run> = Vec::new();
    let mut x = 0.0f32;
    for ch in text.chars() {
        let pick = chain
            .iter()
            .find_map(|(name, bytes, font)| {
                font.charmap().map(ch).map(|g| (*name, *bytes, font, g))
            })
            .or_else(|| {
                // Nothing has it: draw the primary face's .notdef so the gap stays visible.
                chain.first().map(|(n, b, f)| (*n, *b, f, GlyphId::new(0)))
            });
        let Some((name, bytes, font, gid)) = pick else {
            continue;
        };
        let advance = font
            .glyph_metrics(Size::new(size), LocationRef::default())
            .advance_width(gid)
            .unwrap_or(size * 0.5);
        match runs.last_mut() {
            Some(run) if run.font_name == name => {
                run.glyphs.push((gid, x - run.x));
                run.text.push(ch);
            }
            _ => runs.push(Run {
                font_name: name,
                font: bytes,
                text: ch.to_string(),
                glyphs: vec![(gid, 0.0)],
                x,
            }),
        }
        x += advance;
    }
    (runs, x)
}

/// The advance width of `text` at `size` points.
pub fn width(text: &str, face: Face, size: f32) -> f32 {
    layout(text, face, size).1
}

struct PathPen<'a> {
    path: &'a mut BezPath,
    ox: f64,
    oy: f64,
}

impl OutlinePen for PathPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to((self.ox + x as f64, self.oy - y as f64));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to((self.ox + x as f64, self.oy - y as f64));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.path.quad_to(
            (self.ox + cx0 as f64, self.oy - cy0 as f64),
            (self.ox + x as f64, self.oy - y as f64),
        );
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.path.curve_to(
            (self.ox + cx0 as f64, self.oy - cy0 as f64),
            (self.ox + cx1 as f64, self.oy - cy1 as f64),
            (self.ox + x as f64, self.oy - y as f64),
        );
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}

/// Append the outlines of `runs` at `size` points to `path`, with the text origin (baseline,
/// left) at `(x, y)`.
pub fn outlines(runs: &[Run], size: f32, x: f32, y: f32, path: &mut BezPath) {
    for run in runs {
        let Some(font) = font_ref(run.font) else {
            continue;
        };
        let outlines = font.outline_glyphs();
        for (gid, gx) in &run.glyphs {
            let Some(glyph) = outlines.get(*gid) else {
                continue;
            };
            let mut pen = PathPen {
                path,
                ox: (x + run.x + gx) as f64,
                oy: y as f64,
            };
            let _ = glyph.draw(
                DrawSettings::unhinted(Size::new(size), LocationRef::default()),
                &mut pen,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_measures_and_falls_back() {
        let (runs, w) = layout("RPM 6297", Face::Mono, 10.0);
        assert_eq!(runs.len(), 1);
        // JetBrains Mono is monospaced: 8 glyphs × 600/1000 em.
        assert!((w - 48.0).abs() < 0.5, "{w}");
        // Outfit has no Cyrillic: those letters come from the mono fallback.
        let (runs, _) = layout("Ab Жд", Face::Ui, 10.0);
        assert!(runs.iter().any(|r| r.font_name == "JetBrainsMono-Regular"));
    }

    #[test]
    fn outlines_produce_geometry() {
        let (runs, _) = layout("Ultra", Face::Display, 20.0);
        let mut path = BezPath::new();
        outlines(&runs, 20.0, 0.0, 20.0, &mut path);
        assert!(path.elements().len() > 20);
    }
}
