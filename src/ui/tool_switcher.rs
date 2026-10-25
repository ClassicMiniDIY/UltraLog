//! ModeNav: the top-level workspace switch (`design/design-system/components/ModeNav/README.md`).
//!
//! Overline type (Saira 600, uppercase, 0.08em) with a 16px icon. The active mode is `cyan` with
//! a 2px cyan bar on the bottom edge carrying `glow-cyan`, the only glow in the chrome besides
//! the primary button hover.

use eframe::egui::{
    self, Align, CornerRadius, Rect, Sense, TextFormat, pos2, text::LayoutJob, vec2,
};
use rust_i18n::t;

use crate::app::UltraLogApp;
use crate::state::ActiveTool;
use crate::ui::components::icon;
use crate::ui::fonts::{self, style};
use crate::ui::theme::{
    paint,
    tokens::{space, stroke},
};

/// Height of the mode nav row (px); the active bar sits on its bottom edge.
pub const MODE_NAV_HEIGHT: f32 = 40.0;

impl ActiveTool {
    /// The Phosphor icon for this mode.
    fn icon(self) -> &'static str {
        match self {
            ActiveTool::LogViewer => icon::WAVEFORM,
            ActiveTool::ScatterPlot => icon::CHART_SCATTER,
            ActiveTool::Histogram => icon::GRID_FOUR,
            ActiveTool::LambdaDelay => icon::TIMER,
            ActiveTool::AccelEnrich => icon::CHART_LINE_UP,
        }
    }
}

impl UltraLogApp {
    /// Render the mode nav
    pub fn render_tool_switcher(&mut self, ui: &mut egui::Ui) {
        let t = self.tokens;
        let overline = fonts::text_style(style::OVERLINE).resolve(ui.style());
        let icon_font = egui::FontId::proportional((overline.size * 16.0 / 11.0).round());

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = space::SPACE_4;
            for tool in ActiveTool::ALL {
                let selected = self.active_tool == tool;
                let label = match tool {
                    ActiveTool::LogViewer => t!("tools.log_viewer"),
                    ActiveTool::ScatterPlot => t!("tools.scatter_plots"),
                    ActiveTool::Histogram => t!("tools.histogram"),
                    ActiveTool::LambdaDelay => t!("tools.lambda_delay"),
                    ActiveTool::AccelEnrich => t!("tools.accel_enrich"),
                }
                .to_uppercase();

                let job = |color| {
                    let mut job = LayoutJob::default();
                    let base = TextFormat {
                        color,
                        valign: Align::Center,
                        ..Default::default()
                    };
                    job.append(
                        tool.icon(),
                        0.0,
                        TextFormat {
                            font_id: icon_font.clone(),
                            ..base.clone()
                        },
                    );
                    job.append(
                        &label,
                        space::SPACE_2,
                        TextFormat {
                            font_id: overline.clone(),
                            extra_letter_spacing: overline.size * 0.08,
                            ..base
                        },
                    );
                    job
                };

                let width = ui.painter().layout_job(job(t.ink_muted)).size().x;
                let (rect, response) = ui.allocate_exact_size(
                    vec2(width + 2.0 * space::SPACE_2, MODE_NAV_HEIGHT),
                    Sense::click(),
                );
                let color = if selected {
                    t.cyan
                } else if response.hovered() {
                    t.ink
                } else {
                    t.ink_muted
                };
                let galley = ui.painter().layout_job(job(color));
                let text_pos = pos2(
                    rect.center().x - galley.size().x / 2.0,
                    rect.center().y - galley.size().y / 2.0,
                );
                ui.painter().galley(text_pos, galley, color);

                if selected {
                    let bar = Rect::from_min_max(
                        pos2(rect.left(), rect.bottom() - stroke::STROKE_INDICATOR),
                        rect.right_bottom(),
                    );
                    if let Some(glow) = paint::glow(&t, bar, 0, t.cyan) {
                        ui.painter().add(glow);
                    }
                    ui.painter().rect_filled(bar, CornerRadius::ZERO, t.cyan);
                }

                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if response.clicked() {
                    self.set_active_tool(tool);
                }
            }
        });
    }
}
