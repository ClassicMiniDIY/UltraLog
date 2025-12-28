//! Update dialog window UI.
//!
//! Displays update information and provides download/install options.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::updater::{InstallResult, UpdateInfo, UpdateState};

impl UltraLogApp {
    /// Render the update available dialog window
    pub fn render_update_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_update_dialog {
            return;
        }

        let mut open = true;
        let mut should_close = false;

        egui::Window::new("Update Available")
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(420.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| match &self.update_state {
                UpdateState::UpdateAvailable(info) => {
                    let info_clone = info.clone();
                    self.render_update_available(ui, info_clone, &mut should_close);
                }
                UpdateState::Downloading => {
                    self.render_downloading(ui);
                }
                UpdateState::ReadyToInstall(path) => {
                    let path_clone = path.clone();
                    self.render_ready_to_install(ui, &path_clone, &mut should_close);
                }
                UpdateState::Error(e) => {
                    let error = e.clone();
                    self.render_update_error(ui, &error, &mut should_close);
                }
                _ => {}
            });

        if !open || should_close {
            self.show_update_dialog = false;
            // Reset state if user dismissed without updating
            if matches!(self.update_state, UpdateState::UpdateAvailable(_)) {
                self.update_state = UpdateState::Idle;
            }
        }
    }

    fn render_update_available(
        &mut self,
        ui: &mut egui::Ui,
        info: UpdateInfo,
        should_close: &mut bool,
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);

            ui.label(
                egui::RichText::new("A new version is available!")
                    .size(18.0)
                    .strong(),
            );

            ui.add_space(15.0);

            // Version comparison
            ui.horizontal(|ui| {
                ui.label("Current version:");
                ui.label(egui::RichText::new(&info.current_version).color(egui::Color32::GRAY));
            });

            ui.horizontal(|ui| {
                ui.label("New version:");
                ui.label(
                    egui::RichText::new(&info.new_version)
                        .color(egui::Color32::LIGHT_GREEN)
                        .strong(),
                );
            });

            ui.add_space(10.0);

            // Download size
            let size_mb = info.download_size as f64 / (1024.0 * 1024.0);
            ui.label(
                egui::RichText::new(format!("Download size: {:.1} MB", size_mb))
                    .color(egui::Color32::GRAY),
            );

            ui.add_space(10.0);

            // Release notes (scrollable, collapsible)
            if let Some(notes) = &info.release_notes {
                egui::CollapsingHeader::new("Release Notes")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(150.0)
                            .show(ui, |ui| {
                                ui.label(notes);
                            });
                    });
            }

            ui.add_space(15.0);

            // Action buttons
            ui.horizontal(|ui| {
                if ui.button("Download & Install").clicked() {
                    self.start_update_download(info.download_url.clone());
                }

                if ui.button("View on GitHub").clicked() {
                    let _ = open::that(&info.release_page_url);
                }

                if ui.button("Later").clicked() {
                    *should_close = true;
                }
            });

            ui.add_space(10.0);
        });
    }

    fn render_downloading(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);

            ui.label(egui::RichText::new("Downloading update...").size(16.0));

            ui.add_space(15.0);

            ui.spinner();

            ui.add_space(15.0);

            ui.label(egui::RichText::new("Please wait...").color(egui::Color32::GRAY));

            ui.add_space(20.0);
        });
    }

    fn render_ready_to_install(
        &mut self,
        ui: &mut egui::Ui,
        path: &std::path::Path,
        should_close: &mut bool,
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);

            ui.label(
                egui::RichText::new("Download complete!")
                    .size(16.0)
                    .color(egui::Color32::LIGHT_GREEN),
            );

            ui.add_space(15.0);

            ui.label("Click Install to apply the update.");

            #[cfg(target_os = "windows")]
            ui.label(
                egui::RichText::new("The application will close and restart automatically.")
                    .color(egui::Color32::GRAY),
            );

            #[cfg(target_os = "macos")]
            ui.label(
                egui::RichText::new("The DMG will open - drag UltraLog to Applications.")
                    .color(egui::Color32::GRAY),
            );

            #[cfg(target_os = "linux")]
            ui.label(
                egui::RichText::new("The application will close and restart automatically.")
                    .color(egui::Color32::GRAY),
            );

            ui.add_space(15.0);

            ui.horizontal(|ui| {
                if ui.button("Install Now").clicked() {
                    match crate::updater::install_update(path) {
                        InstallResult::ReadyToRestart { message } => {
                            self.show_toast_success(&message);
                            *should_close = true;
                            self.update_state = UpdateState::Idle;
                            // Request app exit so the updater script can run
                            self.should_exit_for_update = true;
                        }
                        InstallResult::ManualInstallRequired { message } => {
                            self.show_toast_success(&message);
                            *should_close = true;
                            self.update_state = UpdateState::Idle;
                        }
                        InstallResult::Error(e) => {
                            self.show_toast_error(&e);
                        }
                    }
                }

                if ui.button("Install Later").clicked() {
                    self.show_toast("Update saved to your temp folder.");
                    *should_close = true;
                    self.update_state = UpdateState::Idle;
                }
            });

            ui.add_space(20.0);
        });
    }

    fn render_update_error(&mut self, ui: &mut egui::Ui, error: &str, should_close: &mut bool) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);

            ui.label(
                egui::RichText::new("Update Error")
                    .size(16.0)
                    .color(egui::Color32::from_rgb(191, 78, 48)),
            );

            ui.add_space(15.0);

            ui.label(error);

            ui.add_space(15.0);

            if ui.button("Close").clicked() {
                *should_close = true;
                self.update_state = UpdateState::Idle;
            }

            ui.add_space(20.0);
        });
    }
}
