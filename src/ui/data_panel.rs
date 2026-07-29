//! Right-side data panel that hosts data widgets (track map and friends).
//!
//! The panel is a thin host: it iterates the static widget registry from
//! [`crate::ui::widgets`], skips widgets that report no data for the active
//! tab, and renders the rest stacked vertically with collapsible headers.
//!
//! Panel visibility uses two conditions:
//! 1. `widget.is_available(app)` reports content for the active tab.
//! 2. `Tab.data_panel_state.visible` records whether the panel is expanded.
//!
//! `UserSettings::hidden_widgets` filters widget bodies, not the panel
//! container. An expanded empty panel remains reachable so a hidden widget
//! can be restored from the header.
//!
//! The rail uses only automatic availability. It remains visible as long as
//! something could be shown, including when every available widget is hidden.
//!
//! Visual design notes:
//! - The header is a single 30 px row: title on the left, action buttons
//!   on the right (`+` only when there are hidden widgets to add, then
//!   cog, then collapse). Matches the `DataPane` design from the bundled
//!   HTML prototype.
//! - All icon buttons are drawn with `Painter` primitives (no glyphs, no
//!   `Button` widgets) so they never shift footprint on hover.

use eframe::egui;

use crate::app::UltraLogApp;
use crate::ui::widgets::{DataWidget, registered};

/// Pixel width of the collapsed-rail column.
const RAIL_WIDTH: f32 = 32.0;
/// Pixel size of every icon button (rail expand, header collapse, header
/// options menu, header add).
const ICON_BTN: f32 = 24.0;
/// Header row height - matches the design's `--hdr-h: 30px`.
const HDR_HEIGHT: f32 = 30.0;
/// Width of the cog/plus popup. Wide enough to fit the longest widget
/// title without truncation in any locale we ship.
const POPUP_WIDTH: f32 = 240.0;

impl UltraLogApp {
    /// True when the panel is expanded and at least one registered widget
    /// has data for the active tab. Hidden widgets do not suppress the
    /// container because the header is required to restore them.
    pub fn data_panel_should_show(&self) -> bool {
        let Some(idx) = self.active_tab else {
            return false;
        };
        self.tabs[idx].data_panel_state.visible && self.data_panel_has_content()
    }

    /// True when the panel *could* show something - i.e. any widget is
    /// available for the active tab. Used to decide whether to draw the
    /// collapsed rail. Deliberately ignores `hidden_widgets` so the user
    /// can re-add a hidden widget via the "+" button after expanding the
    /// panel.
    pub fn data_panel_has_content(&self) -> bool {
        if self.active_tab.is_none() {
            return false;
        }
        registered().iter().any(|w| w.is_available(self))
    }

    /// Render a small expand button anchored at the top of a thin column
    /// on the right edge.
    pub fn render_data_panel_rail(&mut self, ui: &mut egui::Ui) {
        let Some(ti) = self.active_tab else {
            return;
        };

        egui::Panel::right("ultralog_data_panel_rail")
            .exact_size(RAIL_WIDTH)
            .resizable(false)
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    let resp = icon_button(ui, ICON_BTN, draw_panel_show_icon);
                    if resp.clicked() {
                        self.tabs[ti].data_panel_state.visible = true;
                    }
                    let _ = resp.on_hover_text(rust_i18n::t!("data_panel.expand"));
                });
            });
    }

    /// Render the panel body. Caller is responsible for placing this inside
    /// an `egui::SidePanel` (or any other container).
    pub fn render_data_panel(&mut self, ui: &mut egui::Ui) {
        let Some(ti) = self.active_tab else { return };

        // Snapshot widget metadata so the popup closures can read titles
        // without needing `&mut self` re-borrows.
        let widget_meta: Vec<(&'static dyn DataWidget, String, bool, bool)> = registered()
            .iter()
            .map(|w| {
                let title = w.title(self);
                let available = w.is_available(self);
                let hidden = self.user_settings.hidden_widgets.contains(w.id());
                (*w, title, available, hidden)
            })
            .collect();
        let has_addable = widget_meta.iter().any(|(_, _, av, hi)| *av && *hi);

        // Header row.
        let header_actions = self.render_header_row(ui, &widget_meta, has_addable);
        let panel_collapsed = header_actions.collapse_panel;
        if panel_collapsed {
            self.tabs[ti].data_panel_state.visible = false;
            for widget in registered() {
                widget.cancel_background_work();
            }
        }
        let mut settings_dirty = false;
        for id in header_actions.toggle_hidden {
            if self.user_settings.hidden_widgets.contains(id.as_str()) {
                self.user_settings.hidden_widgets.remove(id.as_str());
            } else {
                self.user_settings.hidden_widgets.insert(id.clone());
                if let Some(widget) = registered().iter().find(|widget| widget.id() == id) {
                    widget.cancel_background_work();
                }
            }
            settings_dirty = true;
        }
        for id in header_actions.add {
            self.user_settings.hidden_widgets.remove(id.as_str());
            settings_dirty = true;
        }
        if settings_dirty {
            let _ = self.user_settings.save();
        }
        if panel_collapsed {
            return;
        }

        ui.separator();

        // Visible widget panes.
        let visible_widgets: Vec<&'static dyn DataWidget> = widget_meta
            .iter()
            .filter(|(widget, _, available, _)| {
                *available && !self.user_settings.hidden_widgets.contains(widget.id())
            })
            .map(|(w, _, _, _)| *w)
            .collect();

        if visible_widgets.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.weak(rust_i18n::t!("data_panel.no_widgets"));
            });
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let last = visible_widgets.len().saturating_sub(1);
                for (i, widget) in visible_widgets.iter().enumerate() {
                    render_widget_pane(ui, self, *widget);
                    if i != last {
                        ui.add_space(4.0);
                    }
                }
            });
    }

    /// Render the header row (title + action buttons + popups) and return
    /// the deferred actions the caller should apply with `&mut self`. The
    /// popup closures cannot mutate `self` directly because they'd need
    /// to re-borrow through the `Ui` they were given.
    fn render_header_row(
        &mut self,
        ui: &mut egui::Ui,
        widget_meta: &[(&'static dyn DataWidget, String, bool, bool)],
        has_addable: bool,
    ) -> HeaderActions {
        let mut actions = HeaderActions::default();
        let row_size = egui::vec2(ui.available_width(), HDR_HEIGHT);
        ui.allocate_ui_with_layout(
            row_size,
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(rust_i18n::t!("data_panel.title")).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(4.0);

                    // Collapse panel.
                    let collapse = icon_button(ui, ICON_BTN, draw_panel_hide_icon);
                    if collapse.clicked() {
                        actions.collapse_panel = true;
                    }
                    let _ = collapse.on_hover_text(rust_i18n::t!("data_panel.hide"));

                    // Cog (settings) - always visible.
                    let cog = icon_button(ui, ICON_BTN, draw_cog_icon);
                    let cog_id = ui.make_persistent_id("data_panel_cog_popup");
                    let _ = cog
                        .clone()
                        .on_hover_text(rust_i18n::t!("data_panel.options"));
                    egui::Popup::menu(&cog)
                        .id(cog_id)
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                        .show(|ui| {
                            ui.set_min_width(POPUP_WIDTH);
                            ui.label(
                                egui::RichText::new(rust_i18n::t!("data_panel.section.widgets"))
                                    .small()
                                    .weak(),
                            );
                            for (w, title, available, hidden) in widget_meta {
                                let mut visible = !*hidden;
                                let label = if *available {
                                    title.clone()
                                } else {
                                    format!(
                                        "{}  ({})",
                                        title,
                                        rust_i18n::t!("data_panel.no_data_suffix")
                                    )
                                };
                                if ui.checkbox(&mut visible, label).changed() {
                                    actions.toggle_hidden.push(w.id().to_string());
                                }
                            }
                            ui.add_space(4.0);
                            ui.separator();
                            ui.label(
                                egui::RichText::new(rust_i18n::t!("data_panel.section.pane"))
                                    .small()
                                    .weak(),
                            );
                            if ui.button(rust_i18n::t!("data_panel.hide")).clicked() {
                                actions.collapse_panel = true;
                            }
                        });

                    // Plus (add widget) - only when there's something
                    // to add, matching the design's hybrid mode.
                    if has_addable {
                        let plus = icon_button(ui, ICON_BTN, draw_plus_icon);
                        let plus_id = ui.make_persistent_id("data_panel_plus_popup");
                        let _ = plus
                            .clone()
                            .on_hover_text(rust_i18n::t!("data_panel.add_widget"));
                        egui::Popup::menu(&plus)
                            .id(plus_id)
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .show(|ui| {
                                ui.set_min_width(POPUP_WIDTH);
                                let mut any = false;
                                for (w, title, available, hidden) in widget_meta {
                                    if !*available || !*hidden {
                                        continue;
                                    }
                                    any = true;
                                    if ui.button(title).clicked() {
                                        actions.add.push(w.id().to_string());
                                    }
                                }
                                if !any {
                                    ui.weak(rust_i18n::t!("data_panel.no_widgets_to_add"));
                                }
                            });
                    }
                });
            },
        );
        actions
    }
}

/// Deferred header actions, applied after the popup closure ends so we
/// can mutate `&mut self` without overlapping borrows.
#[derive(Default)]
struct HeaderActions {
    collapse_panel: bool,
    /// Widget IDs whose hidden flag should be toggled (cog checklist).
    toggle_hidden: Vec<String>,
    /// Widget IDs to remove from `hidden_widgets` (plus popup).
    add: Vec<String>,
}

/// Allocate a square click target, paint a hover background (no frame
/// at rest, so toggling never shifts siblings), and invoke `draw_icon`
/// to paint a 14x14 icon centered inside.
fn icon_button(
    ui: &mut egui::Ui,
    size: f32,
    draw_icon: fn(&egui::Painter, egui::Rect, egui::Color32),
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let visuals = ui.visuals();
    let painter = ui.painter();
    if resp.hovered() {
        painter.rect_filled(rect, 4.0, visuals.widgets.hovered.bg_fill);
    }
    let fg = if resp.hovered() {
        visuals.strong_text_color()
    } else {
        visuals.weak_text_color()
    };
    let icon_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(14.0, 14.0));
    draw_icon(painter, icon_rect, fg);
    resp
}

/// Outlined rectangle with a vertical divider near the right edge and a
/// chevron pointing right inside the right strip - "collapse panel to
/// the right".
fn draw_panel_hide_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let body = egui::Rect::from_center_size(rect.center(), egui::vec2(14.0, 11.0));
    let stroke = egui::Stroke::new(1.2, color);
    painter.rect_stroke(body, 1.5, stroke, egui::StrokeKind::Inside);
    let div_x = body.left() + body.width() * 0.7;
    painter.line_segment(
        [
            egui::pos2(div_x, body.top()),
            egui::pos2(div_x, body.bottom()),
        ],
        stroke,
    );
    let cy = body.center().y;
    let cx = (div_x + body.right()) * 0.5;
    painter.line_segment(
        [egui::pos2(cx - 1.0, cy - 2.0), egui::pos2(cx + 1.0, cy)],
        stroke,
    );
    painter.line_segment(
        [egui::pos2(cx + 1.0, cy), egui::pos2(cx - 1.0, cy + 2.0)],
        stroke,
    );
}

/// Mirror of [`draw_panel_hide_icon`] for the rail: divider on the left,
/// chevron pointing left.
fn draw_panel_show_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let body = egui::Rect::from_center_size(rect.center(), egui::vec2(14.0, 11.0));
    let stroke = egui::Stroke::new(1.2, color);
    painter.rect_stroke(body, 1.5, stroke, egui::StrokeKind::Inside);
    let div_x = body.left() + body.width() * 0.3;
    painter.line_segment(
        [
            egui::pos2(div_x, body.top()),
            egui::pos2(div_x, body.bottom()),
        ],
        stroke,
    );
    let cy = body.center().y;
    let cx = (body.left() + div_x) * 0.5;
    painter.line_segment(
        [egui::pos2(cx + 1.0, cy - 2.0), egui::pos2(cx - 1.0, cy)],
        stroke,
    );
    painter.line_segment(
        [egui::pos2(cx - 1.0, cy), egui::pos2(cx + 1.0, cy + 2.0)],
        stroke,
    );
}

/// Six-spoke gear icon: outer ring, inner hole, six radial spokes. Read
/// as "settings" without depending on glyph availability.
fn draw_cog_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let center = rect.center();
    let stroke = egui::Stroke::new(1.2, color);
    let r_outer = 4.5;
    let r_inner = 1.6;
    let r_tooth = 6.0;
    painter.circle_stroke(center, r_outer, stroke);
    painter.circle_stroke(center, r_inner, stroke);
    for i in 0..6 {
        let a = std::f32::consts::PI / 3.0 * i as f32;
        let (s, c) = a.sin_cos();
        let p1 = egui::pos2(center.x + c * r_outer, center.y + s * r_outer);
        let p2 = egui::pos2(center.x + c * r_tooth, center.y + s * r_tooth);
        painter.line_segment([p1, p2], stroke);
    }
}

/// Plus sign - two perpendicular line segments.
fn draw_plus_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let center = rect.center();
    let half = 4.0;
    let stroke = egui::Stroke::new(1.4, color);
    painter.line_segment(
        [
            egui::pos2(center.x - half, center.y),
            egui::pos2(center.x + half, center.y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(center.x, center.y - half),
            egui::pos2(center.x, center.y + half),
        ],
        stroke,
    );
}

fn render_widget_pane(ui: &mut egui::Ui, app: &mut UltraLogApp, widget: &'static dyn DataWidget) {
    let title = widget.title(app);
    let tab_id = app.active_tab.map(|index| app.tabs[index].id);
    let id = widget_pane_id(ui, tab_id, widget.id());
    let mut enabled = widget.is_enabled(app);

    let header = egui::CollapsingHeader::new(title)
        .id_salt(id)
        .default_open(enabled);
    let response = header.show(ui, |ui| {
        widget.render(ui, app);
    });
    let now_open = response.openness > 0.5;
    if now_open != enabled {
        enabled = now_open;
        widget.set_enabled(app, enabled);
    }
}

fn widget_pane_id(ui: &egui::Ui, tab_id: Option<u64>, widget_id: &str) -> egui::Id {
    ui.make_persistent_id(("data_panel_widget", tab_id, widget_id))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::parsers::aim::AimChannel;
    use crate::parsers::types::Meta;
    use crate::parsers::{Channel, EcuType, Log, Value};
    use crate::state::{LoadedFile, Tab};

    fn app_with_gps_data() -> UltraLogApp {
        let log = Log {
            meta: Meta::Empty,
            channels: vec![
                Channel::Aim(AimChannel {
                    name: "GPS Latitude".to_string(),
                    unit: "deg".to_string(),
                }),
                Channel::Aim(AimChannel {
                    name: "GPS Longitude".to_string(),
                    unit: "deg".to_string(),
                }),
            ],
            times: vec![0.0],
            data: vec![vec![Value::Float(43.1), Value::Float(131.9)]],
        };

        let mut app = UltraLogApp::default();
        app.files.push(LoadedFile::new(
            PathBuf::from("gps.xrk"),
            "gps.xrk".to_string(),
            EcuType::Aim,
            log,
        ));
        app.tabs.push(Tab::new(0, "gps.xrk".to_string()));
        app.active_tab = Some(0);
        app
    }

    #[test]
    fn hidden_available_widget_does_not_block_panel_expansion() {
        let mut app = app_with_gps_data();
        app.user_settings
            .hidden_widgets
            .insert("track_map".to_string());

        assert!(app.data_panel_has_content());
        assert!(app.data_panel_should_show());

        app.tabs[0].data_panel_state.visible = false;
        assert!(!app.data_panel_should_show());
    }

    #[test]
    fn widget_pane_ids_are_scoped_to_tabs() {
        egui::__run_test_ui(|ui| {
            let first = widget_pane_id(ui, Some(1), "track_map");
            let second = widget_pane_id(ui, Some(2), "track_map");
            assert_ne!(first, second);
        });
    }

    #[test]
    fn gps_channel_lookup_is_cached_and_tracks_the_file() {
        let mut app = app_with_gps_data();
        assert!(
            app.tabs[0]
                .data_panel_state
                .track_map
                .gps_channels
                .get()
                .is_none()
        );

        assert!(app.data_panel_has_content());
        let cached = app.tabs[0]
            .data_panel_state
            .track_map
            .gps_channels
            .get()
            .unwrap();
        assert_eq!(cached.file_index, 0);
        assert_eq!(cached.channels, Some((0, 1)));

        app.files.push(LoadedFile::new(
            PathBuf::from("without-gps.xrk"),
            "without-gps.xrk".to_string(),
            EcuType::Aim,
            Log {
                meta: Meta::Empty,
                channels: Vec::new(),
                times: Vec::new(),
                data: Vec::new(),
            },
        ));
        app.tabs[0].file_index = 1;

        assert!(!app.data_panel_has_content());
        let cached = app.tabs[0]
            .data_panel_state
            .track_map
            .gps_channels
            .get()
            .unwrap();
        assert_eq!(cached.file_index, 1);
        assert_eq!(cached.channels, None);
    }
}
