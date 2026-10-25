//! ActivityRail: the vertical bar that switches the side panel
//! (`design/design-system/components/ActivityRail/README.md`).
//!
//! 20px outline icons at `ink_subtle`; hover `ink` on `bg_hover`; active `cyan` on `cyan_soft`
//! with a 2px cyan indicator at the rail's left edge. Settings is pinned to the bottom.

use eframe::egui::{self, Align2, CornerRadius, Rect, Sense, pos2, vec2};

use crate::app::UltraLogApp;
use crate::state::ActivePanel;
use crate::ui::theme::tokens::{radius, space, stroke};

/// Width of the activity bar in pixels
pub const ACTIVITY_BAR_WIDTH: f32 = 48.0;

/// Size of the square hit area behind each icon (px)
const BUTTON_SIZE: f32 = 36.0;

/// Icon size (px)
const ICON_SIZE: f32 = 20.0;

impl UltraLogApp {
    /// Render the activity bar (vertical icon strip on the far left)
    pub fn render_activity_bar(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = space::SPACE_1;
        ui.add_space(space::SPACE_2);
        for panel in [
            ActivePanel::Files,
            ActivePanel::ToolProperties,
            ActivePanel::Tools,
        ] {
            self.render_activity_icon(ui, panel);
        }
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            ui.add_space(space::SPACE_2);
            self.render_activity_icon(ui, ActivePanel::Settings);
        });
    }

    /// Render a single activity bar icon button
    fn render_activity_icon(&mut self, ui: &mut egui::Ui, panel: ActivePanel) {
        let t = self.tokens;
        let selected = self.active_panel == panel;
        let (row, response) =
            ui.allocate_exact_size(vec2(ACTIVITY_BAR_WIDTH, BUTTON_SIZE), Sense::click());
        let button = Rect::from_center_size(row.center(), vec2(BUTTON_SIZE, BUTTON_SIZE));
        let hovered = response.hovered();

        let (fill, color) = if selected {
            (Some(t.cyan_soft), t.cyan)
        } else if hovered {
            (Some(t.bg_hover), t.ink)
        } else {
            (None, t.ink_subtle)
        };
        let painter = ui.painter();
        if let Some(fill) = fill {
            painter.rect_filled(button, CornerRadius::same(radius::RADIUS_MD), fill);
        }
        if selected {
            let indicator = Rect::from_min_size(
                pos2(row.left(), button.top() + space::SPACE_1),
                vec2(stroke::STROKE_INDICATOR, BUTTON_SIZE - 2.0 * space::SPACE_1),
            );
            painter.rect_filled(indicator, CornerRadius::ZERO, t.cyan);
        }
        painter.text(
            button.center(),
            Align2::CENTER_CENTER,
            panel.icon(),
            egui::FontId::proportional(ICON_SIZE),
            color,
        );

        if response.clicked() {
            self.active_panel = panel;
        }
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        response.on_hover_text(panel.name());
    }
}
