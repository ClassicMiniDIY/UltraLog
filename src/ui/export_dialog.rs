//! ExportDialog (`design/design-system/components/ExportDialog/README.md`).
//!
//! Format (PDF / PNG), then the export theme as two radio cards whose thumbnails are painted
//! with the real export tokens, an info notice about the colour-blind safe palette, and the
//! actions: Copy image (the chart or table alone, for pasting), Cancel, and Export PDF / PNG.
//! The format and the theme per format are remembered (`UserSettings::export`).

use eframe::egui::{
    self, Align2, CornerRadius, FontFamily, FontId, Rect, RichText, Sense, Stroke, StrokeKind,
    pos2, vec2,
};
use rust_i18n::t;

use crate::app::UltraLogApp;
use crate::state::ActiveTool;
use crate::ui::components::{self, Kind, Status, icon};
use crate::ui::export_sheet::ExportFormat;
use crate::ui::fonts;
use crate::ui::theme::tokens::{radius, space};
use crate::ui::theme::{self, ExportTheme, Tokens, export_tokens, paint};

enum Action {
    Export,
    Copy,
    Cancel,
}

/// A miniature sheet in `e` (an export theme's tokens): ground, redline, plot and two series.
fn paint_thumbnail(painter: &egui::Painter, rect: Rect, e: &Tokens, line: egui::Color32) {
    let r = CornerRadius::same(radius::RADIUS_SM);
    painter.rect_filled(rect, r, e.bg_base);
    paint::redline(painter, rect, e, f32::from(radius::RADIUS_SM) * 0.6);
    // The preview's SVG viewBox is 120 × 50 below the redline.
    let area = Rect::from_min_max(rect.min + vec2(0.0, 7.0), rect.max - vec2(0.0, 7.0));
    let at = |x: f32, y: f32| {
        pos2(
            area.left() + x / 120.0 * area.width(),
            area.top() + y / 50.0 * area.height(),
        )
    };
    let plot = Rect::from_min_max(at(8.0, 8.0), at(112.0, 44.0));
    painter.rect_filled(plot, 0.0, e.plot_bg);
    painter.rect_stroke(plot, 0.0, Stroke::new(1.0, e.line), StrokeKind::Inside);
    let one = [
        (8., 36.),
        (30., 30.),
        (50., 34.),
        (70., 16.),
        (90., 26.),
        (112., 20.),
    ];
    let two = [(8., 40.), (40., 38.), (70., 26.), (112., 14.)];
    for (i, pts) in [&one[..], &two[..]].into_iter().enumerate() {
        let (color, _) = theme::series(e, i, true);
        let pts: Vec<_> = pts.iter().map(|&(x, y)| at(x, y)).collect();
        painter.add(egui::Shape::line(pts, Stroke::new(1.5, color)));
    }
    painter.rect_stroke(rect, r, Stroke::new(1.0, line), StrokeKind::Inside);
}

/// One export-theme radio card.
fn theme_card(ui: &mut egui::Ui, theme: ExportTheme, selected: bool, width: f32) -> egui::Response {
    let t = theme::current(ui.ctx());
    let pad = space::SPACE_2;
    let thumb_h = 64.0;
    let height = pad + thumb_h + pad + 36.0 + pad;
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let (name, desc) = match theme {
        ExportTheme::Paper => (t!("export.paper"), t!("export.paper_desc")),
        ExportTheme::Graphite => (t!("export.graphite"), t!("export.graphite_desc")),
    };
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, name.as_ref())
    });

    let painter = ui.painter();
    let r = CornerRadius::same(radius::RADIUS_MD);
    painter.rect_filled(rect, r, t.bg_field);
    let border = if selected {
        Stroke::new(2.0, t.cyan)
    } else if response.hovered() {
        Stroke::new(1.0, t.line)
    } else {
        Stroke::new(1.0, t.line_control)
    };
    painter.rect_stroke(rect, r, border, StrokeKind::Inside);

    let thumb = Rect::from_min_size(rect.min + vec2(pad, pad), vec2(width - 2.0 * pad, thumb_h));
    paint_thumbnail(painter, thumb, &export_tokens(theme), t.line);
    painter.text(
        pos2(thumb.left(), thumb.bottom() + pad),
        Align2::LEFT_TOP,
        name,
        FontId::new(13.0, FontFamily::Name(fonts::UI_SEMIBOLD.into())),
        t.ink,
    );
    painter.text(
        pos2(thumb.left(), thumb.bottom() + pad + 18.0),
        Align2::LEFT_TOP,
        desc,
        FontId::proportional(12.0),
        t.ink_subtle,
    );
    response
}

impl UltraLogApp {
    fn export_dialog_title(&self) -> String {
        match self.active_tool {
            ActiveTool::LogViewer => t!("export.title_chart"),
            ActiveTool::ScatterPlot => t!("export.title_scatter"),
            ActiveTool::Histogram => t!("export.title_histogram"),
            ActiveTool::LambdaDelay | ActiveTool::AccelEnrich => t!("export.title_table"),
        }
        .to_string()
    }

    pub fn render_export_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_export_dialog {
            return;
        }
        let mut open = true;
        let mut action = None;
        let mut choice = self.export_choice;
        let response = components::window(self.export_dialog_title())
            .id(egui::Id::new("export_dialog"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(460.0)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let t = theme::current(ui.ctx());
                ui.spacing_mut().item_spacing.y = space::SPACE_3;
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [64.0, 22.0],
                        egui::Label::new(RichText::new(t!("export.format")).color(t.ink_muted)),
                    );
                    components::segmented(
                        ui,
                        &mut choice.format,
                        &[(ExportFormat::Pdf, "PDF"), (ExportFormat::Png, "PNG")],
                    );
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = space::SPACE_2;
                    let width = (ui.available_width() - space::SPACE_2) / 2.0;
                    for theme in [ExportTheme::Paper, ExportTheme::Graphite] {
                        let selected = choice.theme() == theme;
                        if theme_card(ui, theme, selected, width).clicked() {
                            *choice.theme_mut() = theme;
                        }
                    }
                });
                components::notice(ui, Status::Info, "", &t!("export.cvd_note"));
                ui.separator();
                ui.horizontal(|ui| {
                    let copy = components::button(
                        ui,
                        Kind::Ghost,
                        Some(icon::COPY),
                        t!("export.copy_image"),
                    )
                    .on_hover_text(t!("export.copy_tooltip"));
                    if copy.clicked() {
                        action = Some(Action::Copy);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let label = match choice.format {
                            ExportFormat::Pdf => t!("export.export_pdf"),
                            ExportFormat::Png => t!("export.export_png"),
                        };
                        let export = components::button(
                            ui,
                            Kind::Primary,
                            Some(icon::DOWNLOAD_SIMPLE),
                            label,
                        );
                        if export.clicked() {
                            action = Some(Action::Export);
                        }
                        if components::button(ui, Kind::Ghost, None, t!("common.cancel")).clicked()
                        {
                            action = Some(Action::Cancel);
                        }
                    });
                });
            });
        components::finish_window(ctx, response.as_ref().map(|r| &r.response), &mut open);

        self.export_choice = choice;
        self.show_export_dialog = open && action.is_none();
        match action {
            Some(Action::Export) => self.export_to_file(choice.format, choice.theme()),
            Some(Action::Copy) => self.copy_export_image(choice.theme()),
            Some(Action::Cancel) | None => {}
        }
    }
}
