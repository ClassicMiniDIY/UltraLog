//! PDF back end: printpdf with the bundled TTFs embedded, so text stays selectable and every
//! character the UI shows (−, λ, °, thin spaces, Cyrillic channel names) prints.

use std::collections::HashMap;

use ::image::RgbaImage;
use eframe::egui::{Color32, Pos2, Rect};
use printpdf::{
    Color, FontId, Line, LineCapStyle, LineDashPattern, LineJoinStyle, LinePoint, Op, PaintMode,
    ParsedFont, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Point, Polygon, PolygonRing,
    Pt, RawImage, RawImageData, RawImageFormat, Rgb, TextItem, WindingOrder, XObjectTransform,
};

use super::canvas::{Canvas, Face, HAlign};
use super::text;

pub struct PdfCanvas {
    doc: PdfDocument,
    ops: Vec<Op>,
    width: f32,
    height: f32,
    fonts: HashMap<&'static str, FontId>,
}

fn rgb(c: Color32) -> Color {
    Color::Rgb(Rgb::new(
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
        None,
    ))
}

impl PdfCanvas {
    /// A one-page document of `width` × `height` points.
    pub fn new(title: &str, width: f32, height: f32) -> Self {
        Self {
            doc: PdfDocument::new(title),
            // Round joins and caps, like the PNG back end (kurbo's default): PDF's default
            // miter join makes narrow spikes look taller than the data.
            ops: vec![
                Op::SetLineJoinStyle {
                    join: LineJoinStyle::Round,
                },
                Op::SetLineCapStyle {
                    cap: LineCapStyle::Round,
                },
            ],
            width,
            height,
            fonts: HashMap::new(),
        }
    }

    fn point(&self, p: Pos2) -> Point {
        Point {
            x: Pt(p.x),
            y: Pt(self.height - p.y),
        }
    }

    fn font(&mut self, name: &'static str, bytes: &'static [u8], index: u32) -> Option<FontId> {
        if let Some(id) = self.fonts.get(name) {
            return Some(id.clone());
        }
        let mut warnings = Vec::new();
        let parsed = ParsedFont::from_bytes(bytes, index as usize, &mut warnings)?;
        let id = self.doc.add_font(&parsed);
        self.fonts.insert(name, id.clone());
        Some(id)
    }

    /// Serialize the document.
    pub fn finish(mut self) -> Vec<u8> {
        let page = PdfPage::new(
            printpdf::Mm::from(Pt(self.width)),
            printpdf::Mm::from(Pt(self.height)),
            std::mem::take(&mut self.ops),
        );
        self.doc.with_pages(vec![page]);
        let mut warnings = Vec::new();
        self.doc.save(&PdfSaveOptions::default(), &mut warnings)
    }
}

impl Canvas for PdfCanvas {
    fn fill_rect(&mut self, rect: Rect, color: Color32) {
        let corners = [
            rect.left_bottom(),
            rect.right_bottom(),
            rect.right_top(),
            rect.left_top(),
        ];
        let points = corners
            .iter()
            .map(|p| LinePoint {
                p: self.point(*p),
                bezier: false,
            })
            .collect();
        self.ops.push(Op::SetFillColor { col: rgb(color) });
        self.ops.push(Op::DrawPolygon {
            polygon: Polygon {
                rings: vec![PolygonRing { points }],
                mode: PaintMode::Fill,
                winding_order: WindingOrder::NonZero,
            },
        });
    }

    fn polyline(&mut self, points: &[Pos2], width: f32, color: Color32, dash: Option<[f32; 2]>) {
        if points.len() < 2 {
            return;
        }
        self.ops.push(Op::SetOutlineColor { col: rgb(color) });
        self.ops.push(Op::SetOutlineThickness { pt: Pt(width) });
        if let Some([on, off]) = dash {
            self.ops.push(Op::SetLineDashPattern {
                dash: LineDashPattern::new(0.0, &[on, off]),
            });
        }
        let line = Line {
            points: points
                .iter()
                .map(|p| LinePoint {
                    p: self.point(*p),
                    bezier: false,
                })
                .collect(),
            is_closed: false,
        };
        self.ops.push(Op::DrawLine { line });
        if dash.is_some() {
            self.ops.push(Op::SetLineDashPattern {
                dash: LineDashPattern::solid(),
            });
        }
    }

    fn text(&mut self, pos: Pos2, s: &str, face: Face, size: f32, color: Color32, align: HAlign) {
        let (runs, width) = text::layout(s, face, size);
        let x = match align {
            HAlign::Left => pos.x,
            HAlign::Center => pos.x - width / 2.0,
            HAlign::Right => pos.x - width,
        };
        self.ops.push(Op::SetFillColor { col: rgb(color) });
        for run in runs {
            let Some(font) = self.font(run.font_name, run.font, run.index) else {
                continue;
            };
            let origin = self.point(Pos2::new(x + run.x, pos.y));
            self.ops.push(Op::StartTextSection);
            self.ops.push(Op::SetFont {
                font: PdfFontHandle::External(font),
                size: Pt(size),
            });
            self.ops.push(Op::SetTextCursor { pos: origin });
            self.ops.push(Op::ShowText {
                items: vec![TextItem::Text(run.text)],
            });
            self.ops.push(Op::EndTextSection);
        }
    }

    fn image(&mut self, rect: Rect, image: &RgbaImage) {
        let raw = RawImage {
            pixels: RawImageData::U8(image.as_raw().clone()),
            width: image.width() as usize,
            height: image.height() as usize,
            data_format: RawImageFormat::RGBA8,
            tag: Vec::new(),
        };
        let id = self.doc.add_image(&raw);
        // dpi maps the image's pixel width onto the rect's width in points.
        let dpi = image.width() as f32 * 72.0 / rect.width().max(1.0);
        self.ops.push(Op::UseXobject {
            id,
            transform: XObjectTransform {
                translate_x: Some(Pt(rect.left())),
                translate_y: Some(Pt(self.height - rect.bottom())),
                dpi: Some(dpi),
                ..Default::default()
            },
        });
    }
}
