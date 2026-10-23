# Handoff: UltraLog desktop app (Rust + egui)

Implement the Night Drive design system in the UltraLog desktop app. The design is final; this brief says what to build and how to check it. The brand book is `design-system/README.md`; component specs are `design-system/components/<Name>/README.md`, each with a `preview.html` and a render in `reference/<Name>-night.png`.

## Ground rules

- **Never hard-code a colour, size or radius.** Every value comes from `Tokens` (`rust/ultralog_theme.rs`) or its `space`, `radius`, `stroke`, `text` modules.
- **Stay inside egui's normal theme system** for widgets (`Visuals`, `Style`, `Button::fill`). Custom painting is allowed only for the four helpers in `design-system/6-implementation.md`: redline, glow (built-in `Shadow`), series bloom, perspective floor.
- **Data colours never depend on intensity.** Series, colour maps, status and cursor come from the palette (standard or color-blind safe), not from the intensity slider.
- **Text contrast is tested, not hoped for.** Keep `theme.rs`'s tests passing and add `tools/check_contrast.py` to CI.
- Match the reference renders closely, but the app's real layout wins where they disagree; the renders are component studies, not full screens.
- One commit (or PR) per phase, app building and running at every step.

## Phase 0: Explore and plan

Report before changing anything: egui / eframe / egui_plot versions; where `Visuals`, `Style` and fonts are set; how settings are persisted (and the existing Color blind mode / Wong palette code); how series colours are assigned; how the plot, scatter, histogram, lambda-delay and accel-enrichment views draw colour (find every Turbo/jet usage); how PNG/PDF export renders. Produce a plan that maps each phase below onto real files.

## Phase 1: Tokens and theme runtime

1. Add `rust/ultralog_theme.rs` and `rust/theme.rs` to the app crate (for example `src/ui/theme/`). They target egui 0.31; if the app is on another version, adapt field names (`CornerRadius` was `Rounding` before 0.31; `Shadow` fields changed around then too).
2. Keep `ultralog_theme.rs` generated: add a small build step or script that regenerates it from `design/tokens/tokens.json` (the bundle's generator logic is simple: one `Color32` per token per theme, alpha premultiplied). Commit the generated file so builds don't need Python.
3. Run `cargo test`: `anchors_are_exact`, `text_contrast_holds_across_the_slider`, `data_colors_ignore_intensity` must pass.

## Phase 2: Settings

In Settings → Display:
- **Ultra intensity** slider 0-100, default 35, with named stops (Stealth 0, Night Drive 35, Neon 70, Ultra 100); show "Night Drive · 35" style label; caption "How much neon the app wears. Never changes your data colours or text contrast." See `components/IntensityControl`.
- **Color blind mode** (existing checkbox): now drives the `cvd` data palette and AAA text through `theme::tokens_at`.
- **High contrast** (new checkbox). When on, disable the intensity slider and say why in its caption.
- Persist all three; migrate the existing color-blind setting. Recompute `Tokens` and call `ctx.set_visuals(theme::visuals(&t))` when any changes; store the current `Tokens` somewhere every view can read (context data or app state).

## Phase 3: Fonts and type

- Register three families: Outfit (proportional default), JetBrains Mono (monospace), Saira as `FontFamily::Name("display")`. Map the type scale in `ultralog_theme::text` to `TextStyle`s (body 14, button/label 13, small/caption 12, heading 15, plus named styles for title 18, overline 11 uppercase, data-lg 18 mono).
- egui needs TTF/OTF. `fonts/ttf/` holds Latin subsets for a quick start. For release, use full-coverage files: download Saira, Outfit and JetBrains Mono from https://github.com/google/fonts (`ofl/saira`, `ofl/outfit`, `ofl/jetbrainsmono`). Saira and Outfit there are variable fonts and egui renders only the default instance, so create static weights with `fonttools varLib.instancer` (Saira 600, 700 italic, 800 italic; Outfit 400/500/600). JetBrains Mono ships static TTFs.
- The app supports 15 languages: keep the existing fallback fonts (CJK, Cyrillic, etc.) after these in each family's fallback list.
- Numbers in tables, legends, channel cards and the playback bar use the monospace family.

## Phase 4: Chrome components

Implement, in this order, each per its `components/<Name>/README.md` and reference render: **ModeNav** (top workspace switch, uppercase Saira, cyan active bar with glow), **ActivityRail**, **FileTabs** (ECU source badge, middle-truncated names), **Button** variants (primary = `cyan` fill + `on_cyan`, secondary, ghost, danger, icon), **Segmented**, **TextField/select**, **Checkbox**, **Slider**, **Window** (floating tools: redline on top edge, title in display font), **Notice** and **Badge** (status = icon + word), **DropZone** (welcome screen with perspective floor), **LibraryItem** and **FormulaEditor** (syntax colours: channels cyan, numbers gold, functions violet, time shifts orange).

Icons: switch to Phosphor regular via `egui-phosphor` if the app isn't already on it. Status icons are fixed: check, warning triangle, octagon, info circle.

## Phase 5: Data views

- **Series assignment:** by selection order, stable for the session (removing a channel doesn't recolour others). Series 9-10 dashed when the palette is color-blind safe (`theme::series` returns the dashed flag).
- **Plot / stacked plots:** `plot_bg`, `plot_grid` / `plot_grid_major`, `plot_axis` ticks in mono, `plot_cursor` line (never a series colour), `plot_selection` for zoom/lap ranges, floating legend on `plot_legend_bg` with Cursor row first. With color-blind palette: direct end labels and legend top-left. Bloom (`fx.bloom` > 0): draw a wider translucent copy of each line underneath. Ultra (`fx.plot_floor`): perspective floor behind data.
- **ChannelCard / ChannelList:** series swatch (line glyph) + 2px top strip; names in ink, values mono.
- **Heatmaps** (Histogram, Lambda Delay, Accel Enrichment) and **scatter density / track map:** replace Turbo/jet with `theme::seq` (7-stop OKLab interpolation). Signed values around a target (AFR vs target, corrections) use the diverging map (`div_0..div_4`). Cell text uses `on_seq_N` / `on_div_N` of the nearest stop, on a `cell_plate` rectangle when the plate's alpha > 0. Empty cells hatched on `cell_empty`. Confidence badge by fill style (filled high, outlined medium, dashed low). Selected cell: 2px cyan ring with a `bg_void` gap. Always show the ColorScale.
- **PlaybackBar:** cyan play button, time readout in cyan mono matching the cursor, thin-space-grouped record counter.

## Phase 6: Exports

- **ExportDialog:** format PDF/PNG and theme Paper (default) / Graphite with thumbnails; info notice that exports use the color-blind safe palette. Remember last choice.
- Render exports with `theme::export_tokens(..)`, ignoring intensity, color blind mode and high contrast. Paper uses `logos/ultralog-lockup-paper.svg`, Graphite `logos/ultralog-lockup.svg` (rasterise once at build, e.g. with `resvg`, or ship PNGs).
- **ExportSheet layout:** redline top edge, lockup, title, time range + record/channel counts, chart, legend, footer (ultralog.co · version · date). Graphite PDFs keep a white page margin. PNG at 2×. "Copy for paste" copies the chart only.

## Phase 7: App icon and brand assets

- Generate `.icns` (macOS), `.ico` (Windows) and Linux PNG sizes from `logos/ultralog-app-icon-1024.png`; wire them into the bundler config.
- About window and splash: `ultralog-lockup.svg` on `bg_void` with the redline.

## Phase 8: Verify

- `cargo test`, `cargo clippy`, `python3 design/tools/check_contrast.py design/tokens/tokens.json`.
- Screenshot each mode in Night Drive, Stealth, Ultra, Color-blind safe and High contrast; compare with `reference/`. Check: no hard-coded colours remain (grep for `Color32::from_rgb` outside the theme module), no series ever equals the cursor colour, keyboard focus visible everywhere, every status shows an icon and a word.

## Acceptance criteria

- [ ] Intensity slider moves smoothly from Stealth to Ultra; data colours and text contrast don't change.
- [ ] Color blind mode: Okabe-Ito series, dashed 9-10, direct labels, cividis maps, value plates, AAA text, no bloom/floor.
- [ ] High contrast: black grounds, outlined surfaces, 5px focus ring, slider disabled with reason.
- [ ] No Turbo/jet colormap anywhere.
- [ ] Exports default to Paper, offer Graphite, always color-blind safe, carry the lockup and footer.
- [ ] New app icon in all platform bundles.
- [ ] All tests and the contrast checker pass in CI.
