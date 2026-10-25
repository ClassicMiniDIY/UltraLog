//! Text layout for exports: glyph lookup, advances and outlines from the bundled fonts with
//! skrifa. Layout is plain left-to-right with per-glyph advances (no kerning or shaping), which
//! suits the labels, numbers and names an export prints. A character missing from the chosen
//! face falls back to JetBrains Mono (Greek, Cyrillic), the Noto script faces, then a system CJK
//! face (`fonts::export_cjk_face`). A character no face has prints as `?`, never as `.notdef`:
//! printpdf's `.notdef` width differs from skrifa's and shifted the text after it.

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

/// The system CJK face's key in `Run::font_name`.
pub const SYSTEM_CJK: &str = "system-cjk";

/// Glyphs that share one font, starting at `x` (points from the text start).
pub struct Run {
    pub font_name: &'static str,
    pub font: &'static [u8],
    /// Face index inside `font` (non-zero only for a `.ttc` collection).
    pub index: u32,
    pub text: String,
    pub glyphs: Vec<(GlyphId, f32)>,
    pub x: f32,
}

fn font_ref(bytes: &'static [u8], index: u32) -> Option<FontRef<'static>> {
    FontRef::from_index(bytes, index).ok()
}

type ChainFace = (&'static str, &'static [u8], u32, FontRef<'static>);

/// Lay out `text` at `size` points: runs by font, and the total advance width.
pub fn layout(text: &str, face: Face, size: f32) -> (Vec<Run>, f32) {
    let mut chain: Vec<ChainFace> = std::iter::once(face.font_name())
        .chain(FALLBACKS)
        .filter_map(|name| {
            let bytes = fonts::face_bytes(name)?;
            Some((name, bytes, 0, font_ref(bytes, 0)?))
        })
        .collect();
    let needs_cjk = text.chars().any(|ch| {
        !chain
            .iter()
            .any(|(_, _, _, font)| font.charmap().map(ch).is_some())
    });
    if needs_cjk
        && let Some((bytes, index)) = fonts::export_cjk_face()
        && let Some(font) = font_ref(bytes, index)
    {
        chain.push((SYSTEM_CJK, bytes, index, font));
    }
    let mut runs: Vec<Run> = Vec::new();
    let mut x = 0.0f32;
    for ch in text.chars() {
        let found = chain.iter().find_map(|(name, bytes, index, font)| {
            font.charmap()
                .map(ch)
                .map(|g| (*name, *bytes, *index, font, g, ch))
        });
        // Nothing has it: a `?` from the primary face keeps the gap visible and the widths true.
        let pick = found.or_else(|| {
            chain
                .first()
                .and_then(|(n, b, i, f)| f.charmap().map('?').map(|g| (*n, *b, *i, f, g, '?')))
        });
        let Some((name, bytes, index, font, gid, ch)) = pick else {
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
                index,
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
        let Some(font) = font_ref(run.font, run.index) else {
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
    fn cjk_uses_the_system_face_when_there_is_one() {
        let (runs, _) = layout("ログ 回転数", Face::Ui, 10.0);
        if fonts::export_cjk_face().is_some() {
            assert!(runs.iter().any(|r| r.font_name == SYSTEM_CJK));
            assert!(runs.iter().all(|r| !r.text.contains('?')));
        }
    }

    #[test]
    fn a_character_no_face_has_prints_as_a_question_mark() {
        // U+E000 is a private-use code point: no bundled or system face maps it.
        let (runs, w) = layout("A\u{E000}B", Face::Ui, 10.0);
        let text: String = runs.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(text, "A?B");
        assert!(w > 0.0);
    }

    #[test]
    fn outlines_produce_geometry() {
        let (runs, _) = layout("Ultra", Face::Display, 20.0);
        let mut path = BezPath::new();
        outlines(&runs, 20.0, 0.0, 20.0, &mut path);
        assert!(path.elements().len() > 20);
    }
}
