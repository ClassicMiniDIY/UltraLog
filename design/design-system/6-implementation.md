# Implementation

`tokens.json` is the single source. Generate platform code from it rather than copying hex values by hand.

## Desktop app (Rust + egui)

Generate a `theme.rs` from `tokens.json` in a build script: one `const` per colour token per theme, plus spacing and radius. Then map tokens onto `egui::Visuals` (field names as of egui 0.31):

```rust
pub fn visuals(t: &Tokens) -> egui::Visuals {
    use egui::{CornerRadius, Stroke, Visuals};
    let mut v = Visuals::dark();
    v.panel_fill = t.bg_panel;            // side panels, top bar
    v.window_fill = t.bg_raised;          // floating windows
    v.extreme_bg_color = t.bg_field;      // text edits, plot canvas uses t.plot_bg
    v.faint_bg_color = t.bg_base;
    v.code_bg_color = t.bg_field;
    v.hyperlink_color = t.cyan;
    v.selection.bg_fill = t.cyan_soft;
    v.selection.stroke = Stroke::new(1.0, t.cyan);
    v.window_stroke = Stroke::new(1.0, t.line);
    v.window_corner_radius = CornerRadius::same(10);   // radius-lg
    v.menu_corner_radius = CornerRadius::same(6);      // radius-md
    v.warn_fg_color = t.warning;
    v.error_fg_color = t.danger;

    let w = &mut v.widgets;
    w.noninteractive.bg_stroke = Stroke::new(1.0, t.line);
    w.noninteractive.fg_stroke = Stroke::new(1.0, t.ink_muted);
    w.inactive.weak_bg_fill = t.bg_raised;  w.inactive.bg_fill = t.bg_field;
    w.inactive.bg_stroke = Stroke::new(1.0, t.line_control);
    w.inactive.fg_stroke = Stroke::new(1.0, t.ink);
    w.hovered.weak_bg_fill = t.bg_hover;    w.hovered.bg_stroke = Stroke::new(1.0, t.line_control);
    w.active.weak_bg_fill = t.bg_active;    w.active.bg_stroke = Stroke::new(1.0, t.cyan);
    for s in [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        s.corner_radius = CornerRadius::same(6);
    }
    v
}
```

### How much custom painting

Stay inside egui's normal theme system for every widget: buttons, inputs, checkboxes, sliders, tabs, panels and windows all come from `Visuals` plus a handful of `Button::fill` calls. That keeps upgrades painless. Custom painting is limited to four small, isolated helpers, each a few dozen lines with the egui `Painter`:

1. **Redline**: a 2px `Mesh` with per-vertex colours along a window's top edge.
2. **Glow**: `egui::Shadow` (built in) with the token's blur and colour for the primary button hover and active tab; zero at Stealth.
3. **Series bloom**: draw each line twice in `egui_plot`, a wide translucent copy under the 1.5px line. Width and alpha come from intensity; off below Neon.
4. **Plot floor / drop-zone floor**: straight line segments from a vanishing point, painted before the plot data.

Everything else (chamfered hexagons, the striped sun) is website-only.

### Intensity at runtime

Store the seven anchor tables as generated consts and build the live table whenever a setting changes: `tokens_at(settings)` picks High contrast outright, otherwise blends Stealth → Night Drive (0-35) or Night Drive → Ultra (35-100) in OKLab, then, with Color blind mode on, copies the data and status tokens from `cvd`, raises the text tokens to their AAA values and switches off bloom and the plot floor. A reference implementation with tests (`theme.rs`, written against egui 0.31) ships in the design handoff bundle.

Exports call `Tokens::PAPER` or `Tokens::GRAPHITE` directly and ignore all three settings.

### Widgets

- **Primary buttons** are not a Visuals state: draw them with `egui::Button::new(..).fill(t.cyan)` and an `on_cyan` label.
- **Fonts:** egui loads TTF/OTF, not WOFF2. Embed Saira (600, 700 italic, 800 italic), Outfit (400/500/600) and JetBrains Mono (400/500) TTFs with `include_bytes!` and register `FontFamily::Name("display")`; make Outfit the proportional default and JetBrains Mono the monospace family. The WOFF2 files in `fonts/` are for the web.
- **Plots** (`egui_plot`): `Line::new(..).color(series[i]).width(1.5)`, `.style(LineStyle::dashed_loose())` for series 9-10 in cvd; cursor via `VLine` in `plot_cursor`; grid via `Plot::grid_spacing` and the `plot-grid` colours.
- **Sunset redline:** a `Mesh` with per-vertex colours along the window's top edge.
- **Color blind mode** swaps the whole `Tokens` struct (`Tokens::NIGHT` / `Tokens::CVD`) and calls `ctx.set_visuals`.

## Website (Nuxt / Vue)

Load the generated `tokens.css`; it declares every token as a CSS custom property on `:root`, with `[data-theme="cvd"]` overrides. Map Tailwind (or UnoCSS) theme keys to the variables (`colors: { cyan: 'var(--cyan)', ... }`) so utility classes and components share one source. Set `data-theme="cvd"` from a toggle stored in `localStorage` to mirror the app setting. Self-host the WOFF2 files from `fonts/` with `font-display: swap`.
