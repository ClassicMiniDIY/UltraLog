//! UltraLog theme runtime: builds the live `Tokens` from the user's settings and maps them onto egui.
//! Written against egui 0.31. Field names in `visuals()` may differ slightly in other versions.
use crate::ultralog_theme::{self as ut, Effects, Tokens};
use egui::{Color32, CornerRadius, Shadow, Stroke, Visuals};

/// User-facing settings that decide the theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeSettings {
    /// Ultra intensity, 0..=100. 0 Stealth, 35 Night Drive (default), 70 Neon, 100 Ultra.
    pub intensity: f32,
    pub color_blind: bool,
    pub high_contrast: bool,
}
impl Default for ThemeSettings {
    fn default() -> Self { Self { intensity: 35.0, color_blind: false, high_contrast: false } }
}

/// Export themes ignore every app setting.
pub enum ExportTheme { Paper, Graphite }
pub fn export_tokens(t: ExportTheme) -> Tokens { match t { ExportTheme::Paper => ut::PAPER, ExportTheme::Graphite => ut::GRAPHITE } }

/// The live token table for the app.
pub fn tokens_at(s: ThemeSettings) -> Tokens {
    if s.high_contrast { return ut::HC; }
    let i = s.intensity.clamp(0.0, 100.0);
    let mut t = if i <= 35.0 { lerp_tokens(&ut::STEALTH, &ut::NIGHT, i / 35.0) } else { lerp_tokens(&ut::NIGHT, &ut::ULTRA, (i - 35.0) / 65.0) };
    if s.color_blind {
        let c = ut::CVD;
        // Data + status come from the color-blind table; text is raised to its AAA values.
        copy_data(&mut t, &c);
        t.ink_muted = c.ink_muted; t.ink_subtle = c.ink_subtle; t.line_control = c.line_control; t.magenta = c.magenta;
        t.fx.bloom = 0.0; t.fx.plot_floor = false; t.plot_floor = Color32::TRANSPARENT;
    }
    t
}

fn copy_data(t: &mut Tokens, c: &Tokens) {
    t.success = c.success; t.success_soft = c.success_soft; t.warning = c.warning; t.warning_soft = c.warning_soft;
    t.danger = c.danger; t.danger_soft = c.danger_soft; t.info = c.info;
    t.series_1 = c.series_1; t.series_2 = c.series_2; t.series_3 = c.series_3; t.series_4 = c.series_4; t.series_5 = c.series_5;
    t.series_6 = c.series_6; t.series_7 = c.series_7; t.series_8 = c.series_8; t.series_9 = c.series_9; t.series_10 = c.series_10;
    t.seq_0 = c.seq_0; t.seq_1 = c.seq_1; t.seq_2 = c.seq_2; t.seq_3 = c.seq_3; t.seq_4 = c.seq_4; t.seq_5 = c.seq_5; t.seq_6 = c.seq_6;
    t.on_seq_0 = c.on_seq_0; t.on_seq_1 = c.on_seq_1; t.on_seq_2 = c.on_seq_2; t.on_seq_3 = c.on_seq_3; t.on_seq_4 = c.on_seq_4; t.on_seq_5 = c.on_seq_5; t.on_seq_6 = c.on_seq_6;
    t.div_0 = c.div_0; t.div_1 = c.div_1; t.div_2 = c.div_2; t.div_3 = c.div_3; t.div_4 = c.div_4;
    t.on_div_0 = c.on_div_0; t.on_div_1 = c.on_div_1; t.on_div_2 = c.on_div_2; t.on_div_3 = c.on_div_3; t.on_div_4 = c.on_div_4;
    t.cell_plate = c.cell_plate; t.plot_cursor = c.plot_cursor; t.plot_selection = c.plot_selection;
}

/// Series colour by selection index (0-based). Returns (colour, dashed).
pub fn series(t: &Tokens, idx: usize, color_blind_like: bool) -> (Color32, bool) {
    let s = [t.series_1, t.series_2, t.series_3, t.series_4, t.series_5, t.series_6, t.series_7, t.series_8, t.series_9, t.series_10];
    (s[idx % 10], color_blind_like && idx >= 8)
}

/// Sequential colormap sample, v in 0..=1, interpolated in OKLab between the 7 stops.
pub fn seq(t: &Tokens, v: f32) -> Color32 {
    let s = [t.seq_0, t.seq_1, t.seq_2, t.seq_3, t.seq_4, t.seq_5, t.seq_6];
    let x = v.clamp(0.0, 1.0) * 6.0; let i = (x.floor() as usize).min(5);
    lerp_oklab(s[i], s[i + 1], x - i as f32)
}

pub fn visuals(t: &Tokens) -> Visuals {
    let mut v = Visuals::dark();
    v.dark_mode = true;
    v.panel_fill = t.bg_panel;
    v.window_fill = t.bg_raised;
    v.extreme_bg_color = t.bg_field;
    v.faint_bg_color = t.bg_base;
    v.code_bg_color = t.bg_field;
    v.hyperlink_color = t.cyan;
    v.warn_fg_color = t.warning;
    v.error_fg_color = t.danger;
    v.selection.bg_fill = t.cyan_soft;
    v.selection.stroke = Stroke::new(1.0, t.cyan);
    v.window_stroke = Stroke::new(1.0, t.line);
    v.window_corner_radius = CornerRadius::same(ut::radius::RADIUS_LG);
    v.menu_corner_radius = CornerRadius::same(ut::radius::RADIUS_MD);
    v.window_shadow = Shadow { offset: [0, 18], blur: 48, spread: 0, color: Color32::from_black_alpha(150) };
    v.popup_shadow = Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_black_alpha(120) };

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = t.bg_panel;
    w.noninteractive.weak_bg_fill = t.bg_panel;
    w.noninteractive.bg_stroke = Stroke::new(1.0, t.line);
    w.noninteractive.fg_stroke = Stroke::new(1.0, t.ink_muted);
    w.inactive.bg_fill = t.bg_field;
    w.inactive.weak_bg_fill = t.bg_raised;
    w.inactive.bg_stroke = Stroke::new(1.0, t.line_control);
    w.inactive.fg_stroke = Stroke::new(1.0, t.ink);
    w.hovered.bg_fill = t.bg_hover;
    w.hovered.weak_bg_fill = t.bg_hover;
    w.hovered.bg_stroke = Stroke::new(1.0, t.line_control);
    w.hovered.fg_stroke = Stroke::new(1.0, t.ink);
    w.active.bg_fill = t.bg_active;
    w.active.weak_bg_fill = t.bg_active;
    w.active.bg_stroke = Stroke::new(1.0, t.cyan);
    w.active.fg_stroke = Stroke::new(1.0, t.ink);
    w.open.bg_fill = t.bg_active;
    w.open.weak_bg_fill = t.bg_active;
    w.open.bg_stroke = Stroke::new(1.0, t.line_control);
    w.open.fg_stroke = Stroke::new(1.0, t.ink);
    for s in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        s.corner_radius = CornerRadius::same(ut::radius::RADIUS_MD);
    }
    v
}

/// Glow behind the primary button / active tab. Returns None when intensity turns glows off.
pub fn glow(t: &Tokens, color: Color32) -> Option<Shadow> {
    if t.fx.glow_blur <= 0.0 { return None; }
    let [r, g, b, _] = color.to_array();
    Some(Shadow { offset: [0, 0], blur: t.fx.glow_blur as u8, spread: 1, color: Color32::from_rgba_unmultiplied(r, g, b, (t.fx.glow_alpha * 255.0) as u8) })
}

// ---------- interpolation ----------
fn lerp_tokens(a: &Tokens, b: &Tokens, k: f32) -> Tokens {
    let k = k.clamp(0.0, 1.0);
    macro_rules! mix { ($($f:ident),*) => { Tokens { $($f: lerp_oklab(a.$f, b.$f, k),)* fx: lerp_fx(a.fx, b.fx, k) } } }
    mix!(bg_void, bg_base, bg_panel, bg_raised, bg_field, bg_hover, bg_active, line_subtle, line, line_control, ink, ink_muted, ink_subtle, ink_disabled,
         cyan, cyan_soft, on_cyan, magenta, magenta_soft, on_magenta, violet, orange, gold, redline_0, redline_1, redline_2, redline_3, redline_4, grid_floor,
         success, success_soft, warning, warning_soft, danger, danger_soft, info,
         series_1, series_2, series_3, series_4, series_5, series_6, series_7, series_8, series_9, series_10,
         plot_bg, plot_grid, plot_grid_major, plot_axis, plot_cursor, plot_selection, plot_legend_bg, plot_floor,
         seq_0, seq_1, seq_2, seq_3, seq_4, seq_5, seq_6, on_seq_0, on_seq_1, on_seq_2, on_seq_3, on_seq_4, on_seq_5, on_seq_6,
         div_0, div_1, div_2, div_3, div_4, on_div_0, on_div_1, on_div_2, on_div_3, on_div_4, cell_empty, cell_plate)
}
fn lerp_fx(a: Effects, b: Effects, k: f32) -> Effects {
    let l = |x: f32, y: f32| x + (y - x) * k;
    Effects { glow_blur: l(a.glow_blur, b.glow_blur), glow_alpha: l(a.glow_alpha, b.glow_alpha), bloom: l(a.bloom, b.bloom),
              redline_gradient: if k < 0.5 { a.redline_gradient } else { b.redline_gradient },
              plot_floor: k >= 0.5 && b.plot_floor || k < 0.5 && a.plot_floor }
}

pub fn lerp_oklab(a: Color32, b: Color32, k: f32) -> Color32 {
    if a == b { return a; }
    let (la, aa) = (to_oklab(a), a.a() as f32); let (lb, ab) = (to_oklab(b), b.a() as f32);
    let m = [la[0] + (lb[0] - la[0]) * k, la[1] + (lb[1] - la[1]) * k, la[2] + (lb[2] - la[2]) * k];
    from_oklab(m, (aa + (ab - aa) * k).round() as u8)
}
fn lin(c: u8) -> f32 { let c = c as f32 / 255.0; if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) } }
fn srgb(c: f32) -> u8 { let c = c.clamp(0.0, 1.0); let v = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }; (v * 255.0).round() as u8 }
fn to_oklab(c: Color32) -> [f32; 3] {
    // Work on unmultiplied RGB.
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    let (r, g, b) = (lin(r), lin(g), lin(b));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    [0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s, 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s]
}
fn from_oklab(c: [f32; 3], a: u8) -> Color32 {
    let l = (c[0] + 0.3963377774 * c[1] + 0.2158037573 * c[2]).powi(3);
    let m = (c[0] - 0.1055613458 * c[1] - 0.0638541728 * c[2]).powi(3);
    let s = (c[0] - 0.0894841775 * c[1] - 1.2914855480 * c[2]).powi(3);
    let r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
    let g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
    let b = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;
    Color32::from_rgba_unmultiplied(srgb(r), srgb(g), srgb(b), a)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn anchors_are_exact() {
        let n = tokens_at(ThemeSettings::default());
        assert_eq!(n.bg_panel, ut::NIGHT.bg_panel);
        assert_eq!(tokens_at(ThemeSettings { intensity: 0.0, ..Default::default() }).bg_panel, ut::STEALTH.bg_panel);
        assert_eq!(tokens_at(ThemeSettings { intensity: 100.0, ..Default::default() }).bg_panel, ut::ULTRA.bg_panel);
    }
    fn lum(c: Color32) -> f32 { let [r, g, b, _] = c.to_srgba_unmultiplied(); 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b) }
    fn ratio(a: Color32, b: Color32) -> f32 { let (x, y) = (lum(a), lum(b)); (x.max(y) + 0.05) / (x.min(y) + 0.05) }
    /// Every intensity value keeps text at AA (AAA with Color blind mode).
    #[test] fn text_contrast_holds_across_the_slider() {
        for cb in [false, true] {
            let min = if cb { 7.0 } else { 4.5 };
            for i in 0..=100 {
                let t = tokens_at(ThemeSettings { intensity: i as f32, color_blind: cb, high_contrast: false });
                for ink in [t.ink, t.ink_muted, t.ink_subtle] {
                    for bg in [t.bg_void, t.bg_base, t.bg_panel, t.bg_raised, t.bg_field, t.bg_hover] {
                        assert!(ratio(ink, bg) >= min, "intensity {i} cb {cb}: {:.2}", ratio(ink, bg));
                    }
                }
            }
        }
    }
    #[test] fn data_colors_ignore_intensity() {
        for i in [0.0, 50.0, 100.0] {
            assert_eq!(tokens_at(ThemeSettings { intensity: i, ..Default::default() }).series_3, ut::NIGHT.series_3);
        }
    }
}
