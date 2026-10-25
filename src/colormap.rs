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

/// `map` pre-sampled at 256 steps, for loops that colour many values (scatter bins, track
/// points): an OKLab sample costs a few `cbrt`/`powf` calls, a lookup costs an index. Build one
/// per frame or per cache rebuild from the live tokens; never keep it across a theme change.
pub struct ColormapLut([Color32; 256]);

impl ColormapLut {
    pub fn new(tokens: &Tokens, map: Colormap) -> Self {
        Self(std::array::from_fn(|i| {
            sample(tokens, map, i as f32 / 255.0)
        }))
    }

    /// The colour at `t`, with the same clamping as [`sample`] (non-finite reads as 0).
    pub fn get(&self, t: f32) -> Color32 {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.0[(t * 255.0).round() as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_matches_sample_at_its_steps_and_ends() {
        let t = ut::NIGHT;
        for map in [Colormap::Sequential, Colormap::Diverging] {
            let lut = ColormapLut::new(&t, map);
            assert_eq!(lut.get(0.0), sample(&t, map, 0.0));
            assert_eq!(lut.get(1.0), sample(&t, map, 1.0));
            assert_eq!(lut.get(128.0 / 255.0), sample(&t, map, 128.0 / 255.0));
            assert_eq!(lut.get(f32::NAN), sample(&t, map, f32::NAN));
            assert_eq!(lut.get(-2.0), sample(&t, map, 0.0));
        }
    }
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
