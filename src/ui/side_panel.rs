//! Side panel container - routes to the appropriate panel based on activity bar selection.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::state::ActivePanel;

/// Default width of the side panel in pixels
pub const SIDE_PANEL_WIDTH: f32 = 280.0;

/// Minimum width of the side panel
pub const SIDE_PANEL_MIN_WIDTH: f32 = 200.0;

impl UltraLogApp {
    /// Render the side panel content based on the active panel selection
    pub fn render_side_panel(&mut self, ui: &mut egui::Ui) {
        // Panel header with title
        ui.horizontal(|ui| {
            ui.heading(self.active_panel.name());
        });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Route to the appropriate panel content
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match self.active_panel {
                ActivePanel::Files => self.render_files_panel_content(ui),
                ActivePanel::Channels => self.render_channels_panel_content(ui),
                ActivePanel::Tools => self.render_tools_panel_content(ui),
                ActivePanel::Settings => self.render_settings_panel_content(ui),
            });
    }
}
