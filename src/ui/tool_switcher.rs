//! Tool switcher component for switching between different views.
//!
//! Renders a pill-style tab bar at the top of the main content area
//! allowing users to switch between Log Viewer and Scatter Plots views.

use eframe::egui;

use crate::analytics;
use crate::app::UltraLogApp;
use crate::state::ActiveTool;

impl UltraLogApp {
    /// Render the tool switcher pill tabs
    pub fn render_tool_switcher(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(10.0);

            // Define available tools
            let tools = [
                ActiveTool::LogViewer,
                ActiveTool::ScatterPlot,
                ActiveTool::Histogram,
            ];

            for tool in tools {
                let is_selected = self.active_tool == tool;

                // Style the button based on selection state
                let button_fill = if is_selected {
                    egui::Color32::from_rgb(70, 70, 70)
                } else {
                    egui::Color32::from_rgb(45, 45, 45)
                };

                let text_color = if is_selected {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::from_rgb(180, 180, 180)
                };

                let stroke = if is_selected {
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(113, 120, 78))
                } else {
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 80))
                };

                // Create pill-style button
                let response = ui.add(
                    egui::Button::new(
                        egui::RichText::new(tool.name())
                            .size(self.scaled_font(14.0))
                            .color(text_color),
                    )
                    .fill(button_fill)
                    .stroke(stroke)
                    .corner_radius(egui::CornerRadius::same(16))
                    .min_size(egui::vec2(100.0, 32.0)),
                );

                if response.clicked() {
                    self.active_tool = tool;
                    analytics::track_tool_switched(tool.name());
                }
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }

                ui.add_space(4.0);
            }
        });
    }
}
