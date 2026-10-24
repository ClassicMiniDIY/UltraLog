//! Timeline scrubber and playback controls UI.

use eframe::egui;
use rust_i18n::t;

use crate::analytics;
use crate::app::UltraLogApp;
use crate::ui::components::icon;
use crate::ui::fonts::{self, style};
use crate::ui::theme::tokens::{radius, space};

/// `n` with thin-space thousands separators ("223 559"), the brand's counter format.
pub fn group_thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{2009}');
        }
        out.push(ch);
    }
    out
}

impl UltraLogApp {
    /// Render the timeline scrubber: start and end times at the track ends in mono, a 4px
    /// track filled in `cyan` up to the cursor, and an 18px `ink` thumb bar.
    pub fn render_timeline_scrubber(&mut self, ui: &mut egui::Ui) {
        let Some((min_time, max_time)) = self.get_time_range() else {
            return;
        };

        let total_duration = max_time - min_time;
        if total_duration <= 0.0 {
            return;
        }
        let t = self.tokens;
        let small = fonts::text_style(style::DATA_SM).resolve(ui.style());
        let current_time = self.get_cursor_time().unwrap_or(min_time);

        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 20.0),
            egui::Sense::click_and_drag(),
        );
        let painter = ui.painter();
        let start =
            painter.layout_no_wrap(Self::format_time(min_time), small.clone(), t.ink_subtle);
        let end = painter.layout_no_wrap(Self::format_time(max_time), small, t.ink_subtle);
        let track = egui::Rect::from_min_max(
            egui::pos2(
                rect.left() + start.size().x + space::SPACE_3,
                rect.center().y - 2.0,
            ),
            egui::pos2(
                rect.right() - end.size().x - space::SPACE_3,
                rect.center().y + 2.0,
            ),
        );
        painter.galley(
            egui::pos2(rect.left(), rect.center().y - start.size().y / 2.0),
            start,
            t.ink_subtle,
        );
        painter.galley(
            egui::pos2(
                track.right() + space::SPACE_3,
                rect.center().y - end.size().y / 2.0,
            ),
            end,
            t.ink_subtle,
        );

        let k = ((current_time - min_time) / total_duration).clamp(0.0, 1.0) as f32;
        let thumb_x = track.left() + track.width() * k;
        let pill = egui::CornerRadius::same(radius::RADIUS_PILL);
        painter.rect_filled(track, pill, t.line);
        painter.rect_filled(
            egui::Rect::from_min_max(track.left_top(), egui::pos2(thumb_x, track.bottom())),
            pill,
            t.cyan,
        );
        painter.rect_filled(
            egui::Rect::from_center_size(
                egui::pos2(thumb_x, rect.center().y),
                egui::vec2(4.0, 18.0),
            ),
            egui::CornerRadius::same(radius::RADIUS_XS),
            t.ink,
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }

        if let Some(pos) = response.interact_pointer_pos()
            && (response.dragged() || response.clicked())
        {
            let k = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0) as f64;
            let time = min_time + k * total_duration;
            // Stop playback when user manually scrubs
            self.is_playing = false;
            self.last_frame_time = None;

            self.set_cursor_time(Some(time));
            let record = self.find_record_at_time(time);
            self.set_cursor_record(record);
            // Force repaint to update legend values
            ui.ctx().request_repaint();
        }
    }

    /// Render the record/time indicator bar with playback controls
    pub fn render_record_indicator(&mut self, ui: &mut egui::Ui) {
        let t = self.tokens;
        ui.horizontal(|ui| {
            let button_size = egui::vec2(28.0, 28.0);
            let corner = egui::CornerRadius::same(radius::RADIUS_MD);
            let fill_family = egui::FontFamily::Name(fonts::PHOSPHOR_FILL.into());

            // Play / pause: the one cyan-filled transport button
            let play_icon = if self.is_playing {
                icon::PAUSE
            } else {
                icon::PLAY
            };
            let play_button = egui::Button::new(
                egui::RichText::new(play_icon)
                    .family(fill_family.clone())
                    .size(14.0)
                    .color(t.on_cyan),
            )
            .fill(t.cyan)
            .corner_radius(corner)
            .min_size(button_size);

            let play_tip = if self.is_playing { "Pause" } else { "Play" };
            if ui.add(play_button).on_hover_text(play_tip).clicked() {
                self.is_playing = !self.is_playing;
                if self.is_playing {
                    // Track playback start for analytics
                    analytics::track_playback_started(self.playback_speed);
                    // Reset frame time when starting playback
                    self.last_frame_time = Some(std::time::Instant::now());
                    // Initialize cursor if not set
                    if self.get_cursor_time().is_none()
                        && let Some((min, _)) = self.get_time_range()
                    {
                        self.set_cursor_time(Some(min));
                        let record = self.find_record_at_time(min);
                        self.set_cursor_record(record);
                    }
                }
            }

            // Stop button (resets to beginning)
            let stop_button = egui::Button::new(
                egui::RichText::new(icon::STOP)
                    .family(fill_family)
                    .size(12.0)
                    .color(t.ink),
            )
            .corner_radius(corner)
            .min_size(button_size);

            if ui.add(stop_button).on_hover_text("Stop").clicked() {
                self.is_playing = false;
                self.last_frame_time = None;
                // Reset cursor to beginning
                if let Some((min, _)) = self.get_time_range() {
                    self.set_cursor_time(Some(min));
                    let record = self.find_record_at_time(min);
                    self.set_cursor_record(record);
                }
            }

            ui.separator();

            // Playback speed selector
            ui.label(
                egui::RichText::new(t!("timeline.speed").trim_end_matches(':').to_string())
                    .text_style(fonts::text_style(style::LABEL))
                    .color(t.ink_muted),
            );

            let speed_options = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
            egui::ComboBox::from_id_salt("playback_speed")
                .selected_text(
                    egui::RichText::new(format!("{}\u{00D7}", self.playback_speed)).monospace(),
                )
                .width(60.0)
                .show_ui(ui, |ui| {
                    for speed in speed_options {
                        ui.selectable_value(
                            &mut self.playback_speed,
                            speed,
                            egui::RichText::new(format!("{speed}\u{00D7}")).monospace(),
                        );
                    }
                });

            ui.separator();

            // Current time: data-lg in cyan, the same colour as the plot cursor
            if let Some(time) = self.get_cursor_time() {
                ui.label(
                    egui::RichText::new(Self::format_time(time))
                        .text_style(fonts::text_style(style::DATA_LG))
                        .color(t.plot_cursor),
                );
            }

            // Record counter with thin-space thousands
            if let Some(record) = self.get_cursor_record()
                && let Some(tab_idx) = self.active_tab
            {
                let file_index = self.tabs[tab_idx].file_index;
                if file_index < self.files.len() {
                    let total_records = self.files[file_index].log.data.len();
                    ui.label(
                        egui::RichText::new(t!(
                            "timeline.record",
                            current = group_thousands(record + 1),
                            total = group_thousands(total_records)
                        ))
                        .text_style(egui::TextStyle::Monospace)
                        .color(t.ink_muted),
                    );
                }
            }
        });
    }

    /// Update playback state - advances cursor based on elapsed time
    pub fn update_playback(&mut self, ctx: &egui::Context) {
        if !self.is_playing {
            return;
        }

        let Some((min_time, max_time)) = self.get_time_range() else {
            self.is_playing = false;
            return;
        };

        let now = std::time::Instant::now();
        let delta = if let Some(last) = self.last_frame_time {
            now.duration_since(last).as_secs_f64()
        } else {
            0.0
        };
        self.last_frame_time = Some(now);

        // Advance cursor by delta * playback_speed
        if let Some(current_time) = self.get_cursor_time() {
            let new_time = current_time + (delta * self.playback_speed);

            if new_time >= max_time {
                // Reached end - stop playback
                self.set_cursor_time(Some(max_time));
                let record = self.find_record_at_time(max_time);
                self.set_cursor_record(record);
                self.is_playing = false;
                self.last_frame_time = None;
            } else {
                self.set_cursor_time(Some(new_time));
                let record = self.find_record_at_time(new_time);
                self.set_cursor_record(record);
            }
        } else {
            // No cursor set, start from beginning
            self.set_cursor_time(Some(min_time));
            let record = self.find_record_at_time(min_time);
            self.set_cursor_record(record);
        }

        // Request continuous repaint during playback
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::group_thousands;

    #[test]
    fn counters_group_with_thin_spaces() {
        assert_eq!(group_thousands(7), "7");
        assert_eq!(group_thousands(40_301), "40\u{2009}301");
        assert_eq!(group_thousands(223_559), "223\u{2009}559");
        assert_eq!(group_thousands(1_000_000), "1\u{2009}000\u{2009}000");
    }
}
