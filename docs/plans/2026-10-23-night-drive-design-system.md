# Night Drive Design System — Desktop App Implementation Plan

- **Date:** 2026-10-23
- **Status:** Revised after the skeptical review; decisions resolved 2026-10-23 — awaiting go-ahead for Phase 0.5
- **Branch:** `origin/release/3.0.0` (the handoff worktree's local branch is `night-drive-3.0.0`,
  because `release/3.0.0` is checked out in another worktree). Ships in 3.0.0 with the six fixes
  already on the branch.
- **Design source:** `design/` — brief `design/HANDOFF-APP.md`, brand book `design/design-system/README.md`, tokens `design/tokens/tokens.json`

## Summary

The handoff defines the Night Drive look: an indigo-black chrome, one cyan accent, a sunset
redline, three typefaces, and seven token themes composed from three settings (Ultra intensity,
Color blind mode, High contrast) plus two export themes (Paper, Graphite). The app today has
none of it: stock `Visuals::dark()`, panel fills hard-coded over it, about 450 hard-coded
colours, Turbo/jet colour maps and an olive/rust series palette. This plan maps the eight phases
of `HANDOFF-APP.md` onto real files, one commit (or a short run of commits) per phase, the app
building and running after each.

## Phase 0 report — what exists today

| Question | Finding |
| --- | --- |
| Versions | eframe/egui **0.36.1**, egui_plot **0.37.0**, edition 2024, MSRV 1.95 |
| Bundle runtime vs egui 0.36 | `design/rust/theme.rs` + `ultralog_theme.rs` compile **unchanged** on egui 0.36.1; the three tests pass. `clippy -D warnings` fails with 32 `excessive_precision` errors (OKLab constants), and both files fail `rustfmt --check` |
| Visuals | `cc.egui_ctx.set_visuals(egui::Visuals::dark())` in `UltraLogApp::new` (`app.rs:327`). `set_visuals` writes only the current theme slot and the app never pins `ThemePreference::Dark`, so an OS appearance change can swap in egui's light style. Central/side/activity panel frames hard-code fills over Visuals (`app.rs:2344-2365`). `menu.rs` overrides `text_styles` per menu |
| Fonts | `app.rs:295-324`: Outfit Regular as proportional default; an Outfit Bold `"Bold"` family is defined but never used. No monospace face (egui's Hack), no display face. Sizes are inline: 97 `scaled_font()` calls (the `FontScale` multiplier) feeding ~277 `FontId`/`.size()` sites |
| Non-Latin text | **No fallback fonts are loaded.** egui's defaults cover Latin, Greek and Cyrillic only, so `ja`, `zh-CN`, `hi`, `bn`, `ar`, `ur` strings render as missing-glyph boxes today. epaint 0.36 shapes with harfrust, so Devanagari/Bengali should render once a font is present; Arabic/Urdu still lack right-to-left layout |
| Settings | `UserSettings` (`settings.rs`), JSON, `version: 1`, `serde(default)` per field; `color_blind_mode: bool` exists and is toggled in `settings_panel.rs:114` and the legacy `sidebar.rs:270`. Three-place sync rule (struct, `UltraLogApp::new`, `eframe::App::save`) |
| Series colours | `CHART_COLORS` (olive/rust) and `COLORBLIND_COLORS` (Wong, slot 8 black) in `state.rs:46-72`. Read through `get_channel_color` (`app.rs:398`) and directly in `chart.rs:288`, `:659`, `export.rs:181`, `:318`, `:394`. `ipc/handler.rs:129` sends the colour as hex over MCP. New channels take the first free slot of 10 (`app.rs:1211`, `:1461`, `computed_channels_manager.rs:663`, `analysis_panel.rs:734`) — no recolouring, as the spec wants — but stacked mode allows 60 channels (`MAX_TOTAL_CHANNELS`) and everything past 10 falls back to slot 0 |
| Plot | `chart.rs`: egui_plot `Line`s, cursor `VLine` (`:309`, `:678`), grid via `grid_color_with_opacity` (`:1101`). egui_plot 0.37 has one grid colour, takes its background from `extreme_bg_color`, and its built-in `Legend` cannot show a Cursor row, live mono values or a custom fill |
| Heatmaps | Histogram: 11-stop **jet** (`histogram.rs:1728`); scatter density: jet (`scatter_plot.rs:472`); table generators: Viridis (`table_generator.rs:1028`), colour-coded confidence (`:134`); cell text via `get_aaa_text_color` (`histogram.rs:107`, also `table_generator.rs:1030`); track map: user picker **Viridis / Turbo** (`colormap.rs`, `track_map.rs:409`), its `color_cache` keyed by `ColorSignature` (`state.rs:753`), which holds the `Colormap` enum only |
| Exports | `export.rs` (chart, histogram, scatter → PNG via `image`, PDF via `printpdf` 0.12) with their own jet ramps (`:909`, `:949`); `table_export.rs` (tables, Viridis, `text_on()`). **PNG has no font rasterizer**; PDF text is built-in Helvetica (`~` becomes an arrow). Render functions are `UltraLogApp` methods; tests exist only in `table_export.rs` and assert Viridis values |
| Icons | Three hand-drawn icons in `icons.rs`; ~23 labels use Unicode glyphs. `egui-phosphor` **0.14** targets egui 0.36 |
| App icon | `assets/icons/{mac.png,windows.ico,windows.png,linux.png}`; `release.yml` builds the `.icns` from `mac.png`; `main.rs:53` loads `windows.png` as the runtime icon. There is no About window and no splash |
| Windows | 6 `egui::Window::new` sites |
| Font rasterizers in the tree | `skrifa`, `vello_cpu`, `harfrust` (via epaint) |
| Contrast tool | `python3 design/tools/check_contrast.py design/tokens/tokens.json` → all pairs pass in all 7 themes |

## Architecture decisions

1. **Module:** `src/ui/theme/mod.rs` (runtime, from `design/rust/theme.rs`),
   `src/ui/theme/tokens.rs` (generated, from `design/rust/ultralog_theme.rs`, declared with
   `#[rustfmt::skip] mod tokens;`), `src/ui/theme/codegen.rs` (generator library),
   `src/ui/theme/paint.rs` (custom painters). Bundle files stay in `design/rust/` as reference.
2. **Generator:** `codegen::generate(json: &str) -> String` lives in the library;
   `examples/gen_theme.rs` calls it and writes `tokens.rs`. A unit test runs `generate` on
   `design/tokens/tokens.json` and fails if the committed file differs. No Python in the build.
3. **Live theme:** `UltraLogApp::tokens: Tokens`. Each frame the app derives
   `(ThemeSettings, FontScale)` from its live fields; on any difference from the last applied
   value, `apply_theme()` runs: `ctx.set_theme(ThemePreference::Dark)`, then
   `ctx.set_style_of(Theme::Dark, style)` with `theme::visuals(&t)` **and** the text styles, and
   stores a copy with `ctx.data_mut(|d| d.insert_temp(..))` so free functions and `DataWidget`s
   read `theme::current(ctx)`. Deriving per frame covers every place a setting is toggled
   without hooking each one. A `theme_generation: u64` counter increments on each apply.
4. **Type scale:** `ultralog_theme::text` onto `Style::text_styles` (body 14, button 13, small 12,
   heading 15, monospace 13, plus `TextStyle::Name` for `title`, `overline`, `data_lg`,
   `data_sm`), multiplied by `FontScale`. egui families have no weight axis, so each weight is a
   family: `ui` (Outfit 400, = Proportional), `ui-medium` (500), `ui-semibold` (600),
   `display` (Saira 600), `display-bold-italic` (700 i), `display-black-italic` (800 i),
   Monospace (JetBrains Mono 400), `mono-medium` (500). Call sites move from
   `FontId::proportional(scaled_font(n))` to `TextStyle`s, which removes most inline sizes and the
   `menu.rs` overrides.
5. **Colour maps:** `colormap.rs` stays the single entry point but wraps `theme::seq` and a new
   `theme::div`; the Viridis and Turbo LUTs are deleted. `ColorSignature` gains
   `theme_generation`, so the track-map cache rebuilds when the palette changes (added to the
   cache-invalidation contract in `CLAUDE.md`).
6. **Series:** `CHART_COLORS` / `COLORBLIND_COLORS` are deleted. `get_channel_color` returns
   `theme::series(&tokens, idx, cvd_like) -> (Color32, dashed)` where `cvd_like` = Color blind
   mode, High contrast or export. All call sites listed in Phase 0 switch, including the IPC hex
   (same wire format, new values) and exports (export tokens). Past 10 channels: Decision 4.
7. **Persistence:** new `UserSettings` fields `ultra_intensity: u8` with
   `#[serde(default = "default_ultra_intensity")]` returning **35** (a plain default of 0 would
   turn every upgrading user to Stealth), `high_contrast: bool`, `export_theme`,
   `export_format`. `color_blind_mode` keeps its name; `version` stays 1. A test loads a
   2.16-era settings file and asserts intensity 35.
8. **Custom painters:** the brief allows four (redline, glow, bloom, floor). This plan needs
   three more — focus ring, plot legend overlay, major grid lines — see Decision 2.

## Phases

Each phase is one commit unless noted, pushed to `release/3.0.0`. `cargo fmt`, `cargo clippy
--all-targets -D warnings` and `cargo test` pass after every commit. New strings in any phase go
into all 15 locale files in that phase's commit (i18n-translator agent).

### Phase 0.5 — Remove legacy modules

Delete the superseded `sidebar.rs` (not wired into rendering; 12 colours, a second
`color_blind_mode` toggle) and the legacy `render_channel_selection` in `channels.rs`, so no sweep
touches dead code. Update the `CLAUDE.md` module list.

### Phase 1 — Tokens and theme runtime

- Add `src/ui/theme/` per decisions 1-3, `examples/gen_theme.rs`, the drift test, `theme::div`;
  `#[allow(clippy::excessive_precision)]` on the OKLab constants.
- `apply_theme()` replaces `Visuals::dark()`; the hard-coded panel frames in `app.rs:2344-2365`
  read tokens. Settings are not wired yet (fixed Night Drive · 35).
- CI: contrast checker step in `.github/workflows/ci.yml`.

### Phase 1b — Colour sweep of files Phase 4 does not rewrite

Mechanical, one commit per file group, no layout changes. Replace every hard-coded colour with
its semantic token in `app.rs`, `state.rs` (`ToastType` colours), `tool_properties_panel.rs`,
`tools_panel.rs`, `settings_panel.rs`, `analysis_panel.rs`, `menu.rs`, `normalization_editor.rs`,
`update_dialog.rs`, `data_panel.rs`. Skip the files that Phase 4 or 5 rewrite (`tool_switcher`,
`activity_bar`, `tab_bar`, `files_panel`, `toast`, `computed_channels_manager`,
`formula_editor`, `chart`, `channels`, `histogram`, `scatter_plot`, `table_generator`,
`track_map`, `timeline`): each of those gets one combined pass (colours + type + component) in
its phase.

Gate for the whole plan, checked in Phase 8 and run as a CI grep:
`grep -rnE 'Color32::|Rgba\(|Rgb::new|\[u8; 3\]' src/` matches only `src/ui/theme/` and
`Color32::TRANSPARENT`.

### Phase 2 — Settings

- `settings_panel.rs` Display section: **IntensityControl** (slider 0-100, named stops, "Night
  Drive · 35" label, caption), **Color blind mode** (now drives the `cvd` data palette and AAA
  text), **High contrast** (new; disables the slider and says why).
- Three-place persistence per decision 7. Analytics events for the new toggles, matching
  `track_colorblind_mode_toggled`.

### Phase 3 — Fonts and type

- `assets/fonts/`: the eight static TTFs from decision 4, full coverage, with each family's
  `OFL.txt`. `scripts/fonts/build_static_fonts.sh` records how they were made
  (`fonttools varLib.instancer` from google/fonts); outputs are committed.
- Register the families; drop the unused `"Bold"` family.
- Text styles per decision 4; convert the Phase 1b files' call sites. Phase 4/5 files convert in
  their own pass. Numbers in tables, legends, channel cards and the playback bar use Monospace.
- Non-Latin fallback per Decision 1, after the brand faces in every family's list.

### Phase 4 — Chrome components

> **Status (2026-10-23): done** — commits `c1b7afb`…`a3cc9a3`, each checked against
> `design/reference/` in the running app. Left for later phases: the files Phase 5 rewrites
> (`chart`, `channels`, `histogram`, `scatter_plot`, `table_generator`, `track_map`, `timeline`)
> still use egui defaults, emoji and hard-coded colours; the generic egui `Slider` keeps its own
> look with a value field (only Ultra intensity has the custom track); `TextEdit` is the egui
> widget on `bg_field` with the Visuals border (no 28px height or `radius-sm` override).

In the brief's order, each per `design/design-system/components/<Name>/README.md` and
`design/reference/<Name>-night.png`:

| Component | File | Work |
| --- | --- | --- |
| ModeNav | `tool_switcher.rs` | Saira overline, uppercase, cyan active bar + glow, focus ring |
| ActivityRail | `activity_bar.rs` | 20px Phosphor icons, active state, focus ring |
| FileTabs | `tab_bar.rs` | ECU source badge, middle-truncated names (keep the scrolling row) |
| Button variants | new `src/ui/components/button.rs` | primary (`cyan` + `on_cyan`), secondary, ghost, danger, icon |
| Segmented, TextField/select, Checkbox, Slider | Visuals + helpers in `src/ui/components/` | |
| Window | helper wrapping `egui::Window` (6 sites) | redline top edge, display-font title, Esc closes |
| Notice, Badge | `toast.rs` + `src/ui/components/` | icon + word for every status |
| DropZone | `files_panel.rs` | perspective floor |
| LibraryItem, FormulaEditor | `computed_channels_manager.rs`, `formula_editor.rs` | magenta ƒ, gold favourite; `TextEdit` layouter syntax colours |

Also in this phase:

- `egui-phosphor = "0.14"`; replace the Unicode glyph labels and the `icons.rs` drawings.
- Focus ring painter (Decision 2) on every interactive widget, 5px in High contrast.
- High contrast "outlined surfaces": panel and card frames get a `line` stroke when `hc`.
- Motion: `Style::animation_time` 0.12 s; windows 0.18 s; reduced-motion drops glow transitions.

### Phase 5 — Data views

- Series per decision 6; dashed 9-10 with `LineStyle::Dashed { length: 6.0 }` (≈6/4); hover/solo
  draws the hovered line at 2px and fades the others to 40 % (custom, since egui_plot's
  `highlight` doubles width).
- `chart.rs`: plot background `plot_bg` (scope `extreme_bg_color` inside the plot `Ui`),
  `plot_grid` via `grid_color` plus a painted `plot_grid_major` every fifth line, cursor `VLine`
  in `plot_cursor`, custom floating legend (Cursor row first, live mono values, `plot_legend_bg`;
  top-left plus direct end labels when `cvd_like`). Bloom copy: unnamed, `allow_hover(false)`,
  hidden with its series. Floor by `fx.plot_floor`.
- `channels.rs` / `tool_properties_panel.rs`: swatch (line glyph, dashed when dashed) + 2px strip,
  mono values.
- Heatmaps (`histogram.rs`, `scatter_plot.rs`, `table_generator.rs`): `seq`/`div`;
  `get_aaa_text_color` replaced by `on_seq_N`/`on_div_N` on `cell_plate`; hatched `cell_empty`;
  confidence by fill style; selected-cell ring with `bg_void` gap; ColorScale beside every map.
  AFR/λ-vs-target and correction-% measures use `div`.
- `track_map.rs`: picker per Decision 5; 3px trace with 1px `bg_void` casing; start/finish
  `ink` tick; position dot in `plot_cursor` with 2px `bg_void` ring. Remove the
  `track_map.colormap.viridis/turbo` keys in all locales.
- `timeline.rs`: PlaybackBar — cyan play button, cyan mono readout, thin-space record counter.

### Phase 6 — Exports

- **Text first.** PDF: embed Outfit and JetBrains Mono with printpdf `ParsedFont` /
  `add_font` (fixes `−`, thin space, λ and the `~` arrow). PNG: rasterise text with `skrifa` +
  `vello_cpu`, already in the tree via epaint (direct dependencies pinned to epaint's versions).
  This retires the "PNG has no text" note in `CLAUDE.md` and changes the table-export confidence
  markers back to the spec's glyphs.
- Move the `export.rs` renderers from `UltraLogApp` methods to free functions taking data +
  `Tokens`, so tests can assert they use export tokens regardless of in-app settings.
- **ExportDialog** (new window): PDF/PNG, Paper (default) / Graphite with live thumbnails,
  cvd notice; remembers the last choice per format. Cmd/Ctrl+E keeps exporting PNG with the
  remembered theme.
- `export.rs` and `table_export.rs` render with `theme::export_tokens(..)`; both jet ramps,
  `text_on()` and the Viridis test expectations go.
- **ExportSheet** layout: redline, lockup, title, time range + counts, chart, legend/ColorScale,
  footer (ultralog.co · version · date). Graphite PDFs keep a white margin; PNG at 2×.
- **Copy for paste:** the chart only, as a PNG on the clipboard (`arboard::ImageData`).
- Lockups: rasterise `design/logos/ultralog-lockup{,-paper}.svg` once to PNG with a script,
  commit to `assets/brand/`, embed with `include_bytes!` — no `resvg` at runtime.

### Phase 7 — App icon and brand assets

- Regenerate `assets/icons/{mac.png,windows.ico,windows.png,linux.png}` from
  `design/logos/ultralog-app-icon-1024.png` (paths unchanged; `release.yml` and `main.rs` need no
  change).
- About window: new (Decision 6). Splash: out of scope (eframe opens fast enough).

### Phase 8 — Verify and document

- `cargo test`, `cargo clippy --all-targets -D warnings`, contrast checker, colour gate grep.
- Screenshots of each tool in Night Drive, Stealth, Ultra, Color-blind safe and High contrast,
  compared with `design/reference/` (drive the app over MCP).
- Checks: no series equals the cursor colour; keyboard focus visible everywhere; every status
  shows icon + word.
- `CLAUDE.md`: new *Theme contract* section (tokens only, generator + drift test, data colours
  ignore intensity, `Tokens` in app state and `ctx.data`, the painters, the colour gate); update
  Settings Persistence fields, the cache contract (`theme_generation`), the export text notes and
  the UI module list. Wiki and README screenshots per the release checklist.
- Last commit on the branch: `chore(release): bump to 3.0.0`.

## Testing

- Bundle tests: `anchors_are_exact`, `text_contrast_holds_across_the_slider`,
  `data_colors_ignore_intensity`.
- New: generator drift; `series()` never returns `plot_cursor` in any theme; `seq`/`div`
  monotonic in OKLab lightness; old settings file loads at intensity 35; settings round-trip;
  export renderers use export tokens whatever the in-app settings; track-map cache rebuilds on a
  theme change; PDF text with `−`/λ round-trips through the embedded font.
- `table_export.rs` tests updated from Viridis to export-token values.

## Decisions (resolved 2026-10-23)

1. **Non-Latin fonts:** bundle Noto Sans Devanagari, Bengali and Arabic (≈1 MB total); load a
   system CJK font at runtime per locale (no bundled CJK). Arabic/Urdu stay without right-to-left
   layout (egui limitation).
2. **Extra painters:** approved — focus ring (egui has no gap ring or 5px focus style), plot
   legend overlay, major grid lines, as small helpers in `paint.rs` beside the brief's four.
3. **Scope:** all phases ship in 3.0.0.
4. **Series past 10:** channels 11-20 repeat the 10 colours dashed, 21+ dotted, in every theme
   (cvd keeps its own dashed 9-10 rule within the first 10).
5. **Track map picker:** Sequential / Diverging (default taken; easy to change).
6. **About window:** a small new one (lockup, version, links) in Phase 7 (default taken).

## Out of scope

- The ultralog.co website rebuild (`design/HANDOFF-WEBSITE.md`) — separate repo and task.
- Splash screen.
- Layout changes beyond what the component specs require.
