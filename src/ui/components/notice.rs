//! Status: Notice (inline message) and status Badge
//! (`design/design-system/components/Notice/README.md`, `.../Badge/README.md`).
//!
//! Every status carries an icon shape and a word, never colour alone: check = success,
//! triangle = warning, octagon = danger, circle-i = info.

use eframe::egui::{self, Color32, CornerRadius, Margin, RichText, Stroke, Ui, text::LayoutJob};

use crate::ui::components::icon;
use crate::ui::theme::{
    self, Tokens,
    tokens::{radius, space},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Info,
    Success,
    Warning,
    Danger,
}

impl Status {
    /// The fixed icon shape for this status.
    pub fn icon(self) -> &'static str {
        match self {
            Status::Info => icon::INFO,
            Status::Success => icon::CHECK,
            Status::Warning => icon::WARNING,
            Status::Danger => icon::WARNING_OCTAGON,
        }
    }

    /// The status colour (icon, lead sentence, badge text).
    pub fn color(self, t: &Tokens) -> Color32 {
        match self {
            Status::Info => t.info,
            Status::Success => t.success,
            Status::Warning => t.warning,
            Status::Danger => t.danger,
        }
    }

    /// The soft fill behind a notice or badge. Info has no soft token; it uses `cyan_soft`.
    pub fn soft(self, t: &Tokens) -> Color32 {
        match self {
            Status::Info => t.cyan_soft,
            Status::Success => t.success_soft,
            Status::Warning => t.warning_soft,
            Status::Danger => t.danger_soft,
        }
    }
}

/// An inline notice: status icon, a bold lead sentence in the status colour, the rest in `ink`.
/// Lead with what happened, then what to do.
pub fn notice(ui: &mut Ui, status: Status, lead: &str, rest: &str) -> egui::Response {
    let t = theme::current(ui.ctx());
    let color = status.color(&t);
    egui::Frame::NONE
        .fill(status.soft(&t))
        .corner_radius(CornerRadius::same(radius::RADIUS_MD))
        .inner_margin(Margin::symmetric(
            space::SPACE_3 as i8,
            space::SPACE_2 as i8,
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(status.icon()).color(color));
                let body = egui::TextStyle::Body.resolve(ui.style());
                let mut job = LayoutJob::default();
                if !lead.is_empty() {
                    job.append(
                        lead,
                        0.0,
                        egui::TextFormat {
                            font_id: egui::FontId::new(
                                body.size,
                                egui::FontFamily::Name(crate::ui::fonts::UI_SEMIBOLD.into()),
                            ),
                            color,
                            ..Default::default()
                        },
                    );
                }
                if !rest.is_empty() {
                    job.append(
                        rest,
                        if lead.is_empty() {
                            0.0
                        } else {
                            body.size * 0.3
                        },
                        egui::TextFormat {
                            font_id: body,
                            color: t.ink,
                            ..Default::default()
                        },
                    );
                }
                job.wrap.max_width = ui.available_width();
                ui.label(job);
            });
        })
        .response
}

/// A small status badge: soft fill, status colour, icon + word.
pub fn status_badge(ui: &mut Ui, status: Status, word: &str) -> egui::Response {
    let t = theme::current(ui.ctx());
    badge_frame(ui, status.soft(&t), Stroke::NONE, |ui| {
        ui.label(
            RichText::new(format!("{} {word}", status.icon()))
                .small()
                .color(status.color(&t)),
        );
    })
}

/// The ECU source badge: mono, outlined.
pub fn ecu_badge(ui: &mut Ui, code: &str) -> egui::Response {
    let t = theme::current(ui.ctx());
    badge_frame(
        ui,
        Color32::TRANSPARENT,
        Stroke::new(1.0, t.line_control),
        |ui| {
            ui.label(
                RichText::new(code)
                    .text_style(crate::ui::fonts::text_style(
                        crate::ui::fonts::style::DATA_SM,
                    ))
                    .color(t.ink_subtle),
            );
        },
    )
}

fn badge_frame(
    ui: &mut Ui,
    fill: Color32,
    stroke: Stroke,
    add: impl FnOnce(&mut Ui),
) -> egui::Response {
    egui::Frame::NONE
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(radius::RADIUS_XS))
        .inner_margin(Margin::symmetric(space::SPACE_1 as i8 + 2, 1))
        .show(ui, add)
        .response
}
