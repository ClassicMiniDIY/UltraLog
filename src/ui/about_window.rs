//! About window (`design/HANDOFF-APP.md`: the lockup on `bg_void` with the redline): version,
//! licence, links and Check for Updates.

use eframe::egui::{self, Align2, CornerRadius, RichText, Sense, vec2};
use rust_i18n::t;

use crate::app::UltraLogApp;
use crate::ui::components::{self, Kind, icon};
use crate::ui::export_sheet;
use crate::ui::theme::tokens::{radius, space};
use crate::ui::theme::{self, ExportTheme};

const WEBSITE: &str = "https://ultralog.co";
const REPOSITORY: &str = "https://github.com/ClassicMiniDIY/UltraLog";

/// The dark lockup as a texture, uploaded once per context.
fn lockup_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::new("about_lockup_texture");
    if let Some(tex) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return tex;
    }
    let img = export_sheet::lockup(ExportTheme::Graphite);
    let color = egui::ColorImage::from_rgba_unmultiplied(
        [img.width() as usize, img.height() as usize],
        img.as_raw(),
    );
    let tex = ctx.load_texture("about_lockup", color, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

impl UltraLogApp {
    pub fn render_about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let mut open = true;
        let mut check_updates = false;
        let checking = matches!(
            self.update_state,
            crate::updater::UpdateState::Checking | crate::updater::UpdateState::Downloading
        );
        let response = components::window(t!("about.title"))
            .id(egui::Id::new("about_window"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .default_width(400.0)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let t = theme::current(ui.ctx());
                ui.spacing_mut().item_spacing.y = space::SPACE_2;

                // The lockup on bg_void.
                let (band, _) =
                    ui.allocate_exact_size(vec2(ui.available_width(), 96.0), Sense::hover());
                ui.painter()
                    .rect_filled(band, CornerRadius::same(radius::RADIUS_MD), t.bg_void);
                let tex = lockup_texture(ui.ctx());
                let [w, h] = tex.size_vec2().into();
                let height = 40.0;
                ui.painter().image(
                    tex.id(),
                    egui::Rect::from_center_size(band.center(), vec2(height * w / h, height)),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    theme::image_tint(1.0),
                );

                ui.add_space(space::SPACE_2);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(t!("about.tagline")).color(t.ink_muted));
                    ui.label(
                        RichText::new(t!("menu.version", version = env!("CARGO_PKG_VERSION")))
                            .text_style(egui::TextStyle::Monospace)
                            .color(t.ink),
                    );
                    ui.label(
                        RichText::new(t!("about.license", license = env!("CARGO_PKG_LICENSE")))
                            .text_style(egui::TextStyle::Small)
                            .color(t.ink_subtle),
                    );
                });
                ui.add_space(space::SPACE_2);

                let links = [
                    (icon::GLOBE, t!("about.website"), WEBSITE.to_string()),
                    (
                        icon::BOOK_OPEN,
                        t!("menu.documentation"),
                        format!("{REPOSITORY}/wiki"),
                    ),
                    (
                        icon::GITHUB_LOGO,
                        t!("about.source"),
                        REPOSITORY.to_string(),
                    ),
                    (
                        icon::BUG,
                        t!("menu.report_issue"),
                        format!("{REPOSITORY}/issues"),
                    ),
                    (
                        icon::HEART,
                        t!("menu.support_development"),
                        "https://github.com/sponsors/ClassicMiniDIY".to_string(),
                    ),
                ];
                ui.horizontal_wrapped(|ui| {
                    for (glyph, label, url) in links {
                        let link = components::button(ui, Kind::Ghost, Some(glyph), label)
                            .on_hover_text(&url);
                        if link.clicked() {
                            let _ = open::that(&url);
                        }
                    }
                });

                ui.separator();
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if checking {
                        t!("menu.checking_for_updates")
                    } else {
                        t!("menu.check_for_updates")
                    };
                    ui.add_enabled_ui(!checking, |ui| {
                        let check = components::button(
                            ui,
                            Kind::Secondary,
                            Some(icon::ARROWS_CLOCKWISE),
                            label,
                        );
                        if check.clicked() {
                            check_updates = true;
                        }
                    });
                });
            });
        components::finish_window(ctx, response.as_ref().map(|r| &r.response), &mut open);
        self.show_about = open;
        if check_updates {
            self.start_update_check();
        }
    }
}
