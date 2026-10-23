//! FileTabs: one tab per loaded log (`design/design-system/components/FileTabs/README.md`).
//!
//! Each tab shows the ECU source as a mono `ecu` badge, the file name truncated in the middle
//! so the extension stays visible, and a close button. Active: `bg_raised`, `ink`,
//! `line_control` border and a 2px `cyan` inset on the top edge. Inactive: `bg_panel`,
//! `ink_muted`. The full file name is in the tooltip.

use eframe::egui::{
    self, Align2, CornerRadius, FontId, Rect, Sense, Stroke, StrokeKind, pos2, vec2,
};

use crate::app::UltraLogApp;
use crate::ui::components::icon;
use crate::ui::fonts::{self, style};
use crate::ui::theme::tokens::{radius, space, stroke};

/// Tab height (px)
const TAB_HEIGHT: f32 = 30.0;
/// Widest a file name may draw before it is truncated in the middle (px)
const MAX_NAME_WIDTH: f32 = 200.0;
/// Close button hit area (px)
const CLOSE_SIZE: f32 = 18.0;

impl UltraLogApp {
    /// Render the tab bar for switching between log files
    pub fn render_tab_bar(&mut self, ui: &mut egui::Ui) {
        if self.tabs.is_empty() {
            return;
        }
        let t = self.tokens;

        let mut tab_to_activate: Option<usize> = None;
        let mut tab_to_close: Option<usize> = None;

        // Collect tab info to avoid borrow issues
        let tab_info: Vec<(String, &'static str, bool)> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let code = self
                    .files
                    .get(tab.file_index)
                    .map(|f| f.ecu_type.short_code())
                    .unwrap_or("LOG");
                (tab.name.clone(), code, self.active_tab == Some(i))
            })
            .collect();

        let name_font = egui::TextStyle::Button.resolve(ui.style());
        let badge_font = fonts::text_style(style::DATA_SM).resolve(ui.style());

        // A horizontal scroll area caps the row at the panel width. A bare
        // `horizontal` grew with the tabs and pushed the chart and data panel
        // past the window's right edge once five or six logs were open.
        egui::ScrollArea::horizontal()
            .id_salt("tab_bar_scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = space::SPACE_1;
                    for (i, (name, code, is_active)) in tab_info.iter().enumerate() {
                        let shown = truncate_middle(ui, name, &name_font, MAX_NAME_WIDTH);
                        let name_galley = ui.painter().layout_no_wrap(
                            shown,
                            name_font.clone(),
                            if *is_active { t.ink } else { t.ink_muted },
                        );
                        let badge_galley = ui.painter().layout_no_wrap(
                            (*code).to_owned(),
                            badge_font.clone(),
                            t.ink_subtle,
                        );
                        let badge_size = badge_galley.size() + vec2(8.0, 4.0);
                        let width = space::SPACE_3
                            + badge_size.x
                            + space::SPACE_2
                            + name_galley.size().x
                            + space::SPACE_2
                            + CLOSE_SIZE
                            + space::SPACE_1;

                        let (rect, response) =
                            ui.allocate_exact_size(vec2(width, TAB_HEIGHT), Sense::click());
                        let painter = ui.painter();
                        let corners = CornerRadius {
                            nw: radius::RADIUS_SM,
                            ne: radius::RADIUS_SM,
                            sw: 0,
                            se: 0,
                        };
                        let (fill, border) = if *is_active {
                            (t.bg_raised, t.line_control)
                        } else if response.hovered() {
                            (t.bg_hover, t.line)
                        } else {
                            (t.bg_panel, t.line)
                        };
                        painter.rect(
                            rect,
                            corners,
                            fill,
                            Stroke::new(stroke::STROKE_HAIRLINE, border),
                            StrokeKind::Inside,
                        );
                        if *is_active {
                            let inset = Rect::from_min_size(
                                rect.left_top() + vec2(1.0, 1.0),
                                vec2(rect.width() - 2.0, stroke::STROKE_INDICATOR),
                            );
                            painter.rect_filled(inset, corners, t.cyan);
                        }

                        // ECU badge: mono, outlined
                        let badge = Rect::from_min_size(
                            pos2(
                                rect.left() + space::SPACE_3,
                                rect.center().y - badge_size.y / 2.0,
                            ),
                            badge_size,
                        );
                        painter.rect_stroke(
                            badge,
                            CornerRadius::same(radius::RADIUS_XS),
                            Stroke::new(stroke::STROKE_HAIRLINE, t.line_control),
                            StrokeKind::Inside,
                        );
                        painter.galley(
                            badge.center() - badge_galley.size() / 2.0,
                            badge_galley,
                            t.ink_subtle,
                        );

                        let name_pos = pos2(
                            badge.right() + space::SPACE_2,
                            rect.center().y - name_galley.size().y / 2.0,
                        );
                        painter.galley(name_pos, name_galley, t.ink);

                        // Close button
                        let close = Rect::from_center_size(
                            pos2(
                                rect.right() - space::SPACE_1 - CLOSE_SIZE / 2.0,
                                rect.center().y,
                            ),
                            vec2(CLOSE_SIZE, CLOSE_SIZE),
                        );
                        let close_response = ui
                            .interact(close, ui.id().with(("tab_close", i)), Sense::click())
                            .on_hover_text("Close");
                        let painter = ui.painter();
                        if close_response.hovered() {
                            painter.rect_filled(
                                close,
                                CornerRadius::same(radius::RADIUS_XS),
                                t.bg_hover,
                            );
                        }
                        painter.text(
                            close.center(),
                            Align2::CENTER_CENTER,
                            icon::X,
                            FontId::proportional(12.0),
                            if close_response.hovered() {
                                t.ink
                            } else {
                                t.ink_subtle
                            },
                        );

                        if close_response.clicked() {
                            tab_to_close = Some(i);
                        } else if response.clicked() {
                            tab_to_activate = Some(i);
                        }
                        if response.hovered() || close_response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        response.on_hover_text(name.as_str());
                    }
                });
            });

        // Handle deferred tab activation
        if let Some(index) = tab_to_activate {
            self.active_tab = Some(index);
            self.selected_file = Some(self.tabs[index].file_index);
        }

        // Handle deferred tab close
        if let Some(index) = tab_to_close {
            self.close_tab(index);
        }

        // Baseline under the tabs
        let rect = ui.available_rect_before_wrap();
        ui.painter().line_segment(
            [
                egui::pos2(rect.left(), rect.top()),
                egui::pos2(rect.right(), rect.top()),
            ],
            Stroke::new(stroke::STROKE_HAIRLINE, t.line),
        );
    }
}

/// `name` shortened in the middle with "…" until it fits `max_width`, so the start and the
/// extension both stay visible ("2025-03-06_09…Logs.csv").
fn truncate_middle(ui: &egui::Ui, name: &str, font: &FontId, max_width: f32) -> String {
    let width = |s: &str| {
        ui.painter()
            .layout_no_wrap(s.to_owned(), font.clone(), egui::Color32::WHITE)
            .size()
            .x
    };
    if width(name) <= max_width {
        return name.to_owned();
    }
    let chars: Vec<char> = name.chars().collect();
    // Keep the extension (and a few characters before it) whole.
    let tail_len = name
        .rfind('.')
        .map(|dot| name[dot..].chars().count() + 4)
        .unwrap_or(6)
        .min(chars.len() / 2);
    let tail: String = chars[chars.len() - tail_len..].iter().collect();
    let mut head_len = chars.len() - tail_len;
    while head_len > 1 {
        head_len -= 1;
        let candidate: String = chars[..head_len].iter().collect::<String>() + "…" + &tail;
        if width(&candidate) <= max_width {
            return candidate;
        }
    }
    format!("…{tail}")
}
