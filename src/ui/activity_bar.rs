//! Activity bar component - VS Code-style vertical icon strip for panel navigation.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::state::ActivePanel;

/// Width of the activity bar in pixels
pub const ACTIVITY_BAR_WIDTH: f32 = 48.0;

/// Size of icons in the activity bar
const ICON_SIZE: f32 = 24.0;

/// Padding around icons (reserved for future use)
#[allow(dead_code)]
const ICON_PADDING: f32 = 12.0;

impl UltraLogApp {
    /// Render the activity bar (vertical icon strip on the far left)
    pub fn render_activity_bar(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.add_space(8.0);

            // Render each panel icon
            for panel in [
                ActivePanel::Files,
                ActivePanel::Channels,
                ActivePanel::Tools,
                ActivePanel::Settings,
            ] {
                let is_selected = self.active_panel == panel;
                self.render_activity_icon(ui, panel, is_selected);
                ui.add_space(4.0);
            }
        });
    }

    /// Render a single activity bar icon button
    fn render_activity_icon(&mut self, ui: &mut egui::Ui, panel: ActivePanel, is_selected: bool) {
        let button_size = egui::vec2(ACTIVITY_BAR_WIDTH - 8.0, ACTIVITY_BAR_WIDTH - 8.0);

        // Colors
        let bg_color = if is_selected {
            egui::Color32::from_rgb(60, 60, 60)
        } else {
            egui::Color32::TRANSPARENT
        };

        let icon_color = if is_selected {
            egui::Color32::WHITE
        } else {
            egui::Color32::from_rgb(150, 150, 150)
        };

        let hover_bg = egui::Color32::from_rgb(50, 50, 50);

        // Selected indicator bar on the left
        let indicator_color = egui::Color32::from_rgb(113, 120, 78); // Olive green accent

        let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());

        if response.clicked() {
            self.active_panel = panel;
        }

        let is_hovered = response.hovered();

        // Draw background
        let final_bg = if is_hovered && !is_selected {
            hover_bg
        } else {
            bg_color
        };

        if final_bg != egui::Color32::TRANSPARENT {
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(4), final_bg);
        }

        // Draw selection indicator bar on left edge
        if is_selected {
            let indicator_rect =
                egui::Rect::from_min_size(rect.left_top(), egui::vec2(3.0, rect.height()));
            ui.painter()
                .rect_filled(indicator_rect, egui::CornerRadius::ZERO, indicator_color);
        }

        // Draw the icon
        let center = rect.center();
        self.draw_panel_icon(ui, center, ICON_SIZE, icon_color, panel);

        // Tooltip on hover
        if is_hovered {
            response.on_hover_text(panel.name());
        }
    }

    /// Draw the icon for a specific panel type
    fn draw_panel_icon(
        &self,
        ui: &egui::Ui,
        center: egui::Pos2,
        size: f32,
        color: egui::Color32,
        panel: ActivePanel,
    ) {
        let painter = ui.painter();
        let half = size / 2.0;

        match panel {
            ActivePanel::Files => {
                // Folder icon
                let folder_width = size * 0.9;
                let folder_height = size * 0.7;
                let tab_width = folder_width * 0.4;
                let tab_height = folder_height * 0.15;

                // Main folder body
                let body_rect = egui::Rect::from_center_size(
                    egui::pos2(center.x, center.y + tab_height / 2.0),
                    egui::vec2(folder_width, folder_height - tab_height),
                );
                painter.rect_stroke(
                    body_rect,
                    egui::CornerRadius::same(2),
                    egui::Stroke::new(1.5, color),
                    egui::StrokeKind::Outside,
                );

                // Folder tab
                let tab_rect = egui::Rect::from_min_size(
                    egui::pos2(body_rect.left() + 2.0, body_rect.top() - tab_height),
                    egui::vec2(tab_width, tab_height + 2.0),
                );
                painter.rect_filled(tab_rect, egui::CornerRadius::same(1), color);
            }
            ActivePanel::Channels => {
                // Bar chart icon
                let bar_width = size * 0.15;
                let spacing = size * 0.22;
                let base_y = center.y + half * 0.6;

                // Three bars of different heights
                let heights = [size * 0.5, size * 0.8, size * 0.35];
                let start_x = center.x - spacing;

                for (i, &height) in heights.iter().enumerate() {
                    let x = start_x + (i as f32) * spacing;
                    let bar_rect = egui::Rect::from_min_max(
                        egui::pos2(x - bar_width / 2.0, base_y - height),
                        egui::pos2(x + bar_width / 2.0, base_y),
                    );
                    painter.rect_filled(bar_rect, egui::CornerRadius::same(1), color);
                }
            }
            ActivePanel::Tools => {
                // Wrench icon
                let stroke = egui::Stroke::new(2.0, color);

                // Handle (diagonal line)
                let handle_start = egui::pos2(center.x - half * 0.5, center.y + half * 0.5);
                let handle_end = egui::pos2(center.x + half * 0.1, center.y - half * 0.1);
                painter.line_segment([handle_start, handle_end], stroke);

                // Wrench head (arc-like shape using lines)
                let head_center = egui::pos2(center.x + half * 0.25, center.y - half * 0.25);
                let head_radius = half * 0.45;

                // Draw wrench head as a partial circle with opening
                let segments = 8;
                let start_angle = std::f32::consts::PI * 0.25;
                let end_angle = std::f32::consts::PI * 1.75;
                let angle_step = (end_angle - start_angle) / segments as f32;

                for i in 0..segments {
                    let a1 = start_angle + (i as f32) * angle_step;
                    let a2 = start_angle + ((i + 1) as f32) * angle_step;
                    let p1 = egui::pos2(
                        head_center.x + head_radius * a1.cos(),
                        head_center.y + head_radius * a1.sin(),
                    );
                    let p2 = egui::pos2(
                        head_center.x + head_radius * a2.cos(),
                        head_center.y + head_radius * a2.sin(),
                    );
                    painter.line_segment([p1, p2], stroke);
                }
            }
            ActivePanel::Settings => {
                // Gear icon
                let outer_radius = half * 0.85;
                let inner_radius = half * 0.45;
                let teeth = 8;
                let tooth_depth = half * 0.2;

                // Draw gear teeth
                for i in 0..teeth {
                    let angle = (i as f32) * std::f32::consts::TAU / teeth as f32;
                    let next_angle = ((i as f32) + 0.5) * std::f32::consts::TAU / teeth as f32;

                    let outer_point = egui::pos2(
                        center.x + outer_radius * angle.cos(),
                        center.y + outer_radius * angle.sin(),
                    );
                    let inner_point = egui::pos2(
                        center.x + (outer_radius - tooth_depth) * next_angle.cos(),
                        center.y + (outer_radius - tooth_depth) * next_angle.sin(),
                    );

                    painter.line_segment([outer_point, inner_point], egui::Stroke::new(2.0, color));
                }

                // Draw outer circle
                painter.circle_stroke(
                    center,
                    outer_radius - tooth_depth / 2.0,
                    egui::Stroke::new(2.0, color),
                );

                // Draw inner circle (hole)
                painter.circle_stroke(center, inner_radius, egui::Stroke::new(1.5, color));
            }
        }
    }
}
