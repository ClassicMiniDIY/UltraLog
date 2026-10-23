//! Files panel - file management, loading, and file list.
//!
//! The empty state is the DropZone (`design/design-system/components/DropZone/README.md`): a
//! dashed `line_control` border on `bg_panel` with the perspective floor rising from the bottom
//! edge, a Saira italic title, one primary "Select a file" and the extensions in mono. While a
//! file is dragged over the window the border goes solid `cyan` on `cyan_soft`.

use eframe::egui::{self, Align2, CornerRadius, FontId, Rect, RichText, Stroke, StrokeKind};
use rust_i18n::t;

use crate::app::UltraLogApp;
use crate::state::{LoadingState, SUPPORTED_EXTENSIONS};
use crate::ui::components::{self, Kind, icon};
use crate::ui::fonts::{self, style};
use crate::ui::theme::{
    paint,
    tokens::{radius, space, stroke},
};

impl UltraLogApp {
    /// Render the files panel content (called from side_panel.rs)
    pub fn render_files_panel_content(&mut self, ui: &mut egui::Ui) {
        // Show loading indicator
        if let LoadingState::Loading(filename) = &self.loading_state {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(t!("files.loading", filename = filename));
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
        }

        let is_loading = matches!(self.loading_state, LoadingState::Loading(_));

        // File list (if any files loaded)
        if !self.files.is_empty() {
            self.render_file_list(ui);

            ui.add_space(10.0);

            // Add more files button
            ui.add_enabled_ui(!is_loading, |ui| {
                self.render_add_file_button(ui);
            });
        } else if !is_loading {
            self.render_drop_zone_card(ui);
        }
    }

    /// Render the list of loaded files
    fn render_file_list(&mut self, ui: &mut egui::Ui) {
        let t = self.tokens;
        let mut file_to_remove: Option<usize> = None;
        let mut file_to_switch: Option<usize> = None;

        // Collect file info upfront to avoid borrow issues
        let file_info: Vec<(String, bool, &'static str, String, usize, usize)> = self
            .files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                (
                    file.name.clone(),
                    self.selected_file == Some(i),
                    file.ecu_type.short_code(),
                    file.ecu_type.name().to_string(),
                    file.log.channels.len(),
                    file.log.data.len(),
                )
            })
            .collect();

        ui.label(
            RichText::new(t!("files.loaded_files", count = file_info.len()))
                .text_style(fonts::text_style(style::LABEL))
                .color(t.ink_muted),
        );
        ui.add_space(space::SPACE_1);

        for (i, (file_name, is_selected, code, ecu_name, channel_count, data_count)) in
            file_info.iter().enumerate()
        {
            let border = if *is_selected { t.cyan } else { t.line };
            egui::Frame::NONE
                .fill(if *is_selected {
                    t.bg_active
                } else {
                    t.bg_raised
                })
                .stroke(Stroke::new(stroke::STROKE_HAIRLINE, border))
                .corner_radius(CornerRadius::same(radius::RADIUS_MD))
                .inner_margin(egui::Margin::symmetric(
                    space::SPACE_3 as i8,
                    space::SPACE_2 as i8,
                ))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        components::ecu_badge(ui, code).on_hover_text(ecu_name.as_str());
                        let response = ui.add(
                            egui::Label::new(RichText::new(file_name).color(if *is_selected {
                                t.ink
                            } else {
                                t.ink_muted
                            }))
                            .truncate()
                            .sense(egui::Sense::click()),
                        );
                        if response.clicked() {
                            file_to_switch = Some(i);
                        }
                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if components::icon_button(ui, icon::X, "Remove log").clicked() {
                                file_to_remove = Some(i);
                            }
                        });
                    });

                    ui.label(
                        RichText::new(t!(
                            "files.file_info",
                            ecu = ecu_name,
                            channels = channel_count,
                            points = data_count
                        ))
                        .text_style(fonts::text_style(style::DATA_SM))
                        .color(t.ink_subtle),
                    );
                });

            ui.add_space(space::SPACE_1);
        }

        // Handle deferred file switching
        if let Some(index) = file_to_switch {
            self.switch_to_file_tab(index);
        }

        if let Some(index) = file_to_remove {
            self.remove_file(index);
        }
    }

    /// Render the "Add File" button
    fn render_add_file_button(&mut self, ui: &mut egui::Ui) {
        if components::button(ui, Kind::Secondary, Some(icon::PLUS), t!("files.add_file")).clicked()
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("Log Files", crate::state::SUPPORTED_EXTENSIONS)
                .pick_file()
        {
            self.start_loading_file(path);
        }
    }

    /// Render the drop zone card for when no files are loaded
    fn render_drop_zone_card(&mut self, ui: &mut egui::Ui) {
        let t = self.tokens;
        let dragging = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());

        ui.add_space(space::SPACE_3);
        let frame = egui::Frame::NONE.inner_margin(egui::Margin::symmetric(
            space::SPACE_4 as i8,
            space::SPACE_6 as i8,
        ));
        // Reserve slots so the fill and the floor paint under the content.
        let fill_slot = ui.painter().add(egui::Shape::Noop);
        let floor_slot = ui.painter().add(egui::Shape::Noop);
        let response = frame
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical_centered(|ui| {
                    // Upload mark: cyan ring with the upload icon
                    let size = 44.0;
                    let (mark, _) =
                        ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                    ui.painter().circle_stroke(
                        mark.center(),
                        size / 2.0 - 1.0,
                        Stroke::new(1.5, t.cyan),
                    );
                    ui.painter().text(
                        mark.center(),
                        Align2::CENTER_CENTER,
                        icon::UPLOAD_SIMPLE,
                        FontId::proportional(20.0),
                        t.cyan,
                    );

                    ui.add_space(space::SPACE_2);
                    ui.label(
                        RichText::new(t!("files.drop_file_here"))
                            .text_style(fonts::text_style(style::TITLE))
                            .family(egui::FontFamily::Name(fonts::DISPLAY_BOLD_ITALIC.into()))
                            .color(t.ink),
                    );
                    ui.add_space(space::SPACE_2);

                    if components::button(
                        ui,
                        Kind::Primary,
                        Some(icon::UPLOAD_SIMPLE),
                        t!("files.select_file"),
                    )
                    .clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Log Files", crate::state::SUPPORTED_EXTENSIONS)
                            .pick_file()
                    {
                        self.start_loading_file(path);
                    }

                    ui.add_space(space::SPACE_3);
                    let extensions_text = SUPPORTED_EXTENSIONS
                        .iter()
                        .map(|ext| ext.to_uppercase())
                        .collect::<Vec<_>>()
                        .join(" \u{00B7} ");
                    ui.label(
                        RichText::new(extensions_text)
                            .text_style(fonts::text_style(style::DATA_SM))
                            .color(t.ink_subtle),
                    );
                });
            })
            .response;

        let rect = response.rect;
        let corner = CornerRadius::same(radius::RADIUS_LG);
        ui.painter().set(
            fill_slot,
            egui::Shape::rect_filled(
                rect,
                corner,
                if dragging { t.cyan_soft } else { t.bg_panel },
            ),
        );
        let floor = Rect::from_min_max(
            egui::pos2(rect.left() + 1.0, rect.top() + rect.height() * 0.62),
            rect.right_bottom() - egui::vec2(1.0, 1.0),
        );
        ui.painter().set(
            floor_slot,
            egui::Shape::Vec(paint::floor(floor, t.grid_floor)),
        );

        if dragging {
            ui.painter().rect_stroke(
                rect,
                corner,
                Stroke::new(stroke::STROKE_INDICATOR, t.cyan),
                StrokeKind::Inside,
            );
        } else {
            paint::dashed_rect(
                ui.painter(),
                rect.shrink(0.5),
                f32::from(radius::RADIUS_LG),
                Stroke::new(stroke::STROKE_HAIRLINE, t.line_control),
                4.0,
            );
        }
    }
}
