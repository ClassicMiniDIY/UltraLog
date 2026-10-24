//! Colour maps for data: the single entry point for anything that colours a value by magnitude.
//!
//! The maps are the theme's (`design/design-system/3-data-visualization.md` → Colour maps):
//! **Sequential** for magnitudes (RPM on the track map, hit counts, dead time) and **Diverging**
//! for signed values around a target (AFR vs target, corrections). Both interpolate in OKLab
//! between the token stops and follow Color blind mode (cividis-style, blue ↔ amber), never
//! Ultra intensity. The old Viridis and Turbo LUTs were removed: rainbows invent boundaries and
//! fail colour-blind users.

use eframe::egui::Color32;

use crate::ui::theme::{self, Tokens};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum Colormap {
    #[default]
    Sequential,
    Diverging,
}

/// Sample `map` at `t` (clamped to `[0.0, 1.0]`; non-finite `t` reads as 0, so callers can
/// pass raw normalized values without pre-checking). For `Diverging`, 0.5 is on target.
pub fn sample(tokens: &Tokens, map: Colormap, t: f32) -> Color32 {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    match map {
        Colormap::Sequential => theme::seq(tokens, t),
        Colormap::Diverging => theme::div(tokens, t),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::tokens as ut;

    #[test]
    fn sample_clamps_out_of_range() {
        let t = ut::NIGHT;
        assert_eq!(sample(&t, Colormap::Sequential, -3.0), t.seq_0);
        assert_eq!(sample(&t, Colormap::Sequential, 7.0), t.seq_6);
        assert_eq!(sample(&t, Colormap::Diverging, 7.0), t.div_4);
    }

    #[test]
    fn sample_handles_non_finite() {
        let t = ut::NIGHT;
        for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(sample(&t, Colormap::Sequential, v), t.seq_0);
        }
    }

    #[test]
    fn diverging_midpoint_is_on_target() {
        let t = ut::CVD;
        assert_eq!(sample(&t, Colormap::Diverging, 0.5), t.div_2);
    }
}
