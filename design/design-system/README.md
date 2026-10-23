UltraLog is a free, open-source ECU log viewer for tuners, from weekend Speeduino builds to professional motorsport. The brand is **Night Drive**: a dark cockpit at dusk where the data glows. The chrome stays quiet and nearly black so the channels, the cursor and the colour maps are the brightest things on screen; the sunset colours of the logo show up only where the brand speaks, and the user decides how loud that is with the **Ultra intensity** dial.

## Principles

1. **Data is the brightest thing on screen.** Chrome lives in the `bg-*` indigo-blacks and `ink-muted`; saturated colour is reserved for series, colour maps, the cursor and the one primary action.
2. **One accent does the work.** `cyan` means "interactive and current": primary button, active tab, focus ring, playhead. `magenta` is the brand voice and the mark of computed channels. Nothing else is interactive colour.
3. **Never colour alone.** Every status carries an icon and a word; confidence is a fill style; dashed lines and direct labels extend the palette. Color-blind safe and High contrast meet WCAG AAA.
4. **Numbers are mono.** Every value a tuner compares is set in `font-mono` with tabular figures.
5. **Neon is a dial, not a texture.** Ultra intensity scales glow, gradient and grid from Stealth to full synthwave. It never touches data colours or text contrast.
6. **What leaves the app is safe for anyone.** Exports default to light Paper and always use the color-blind safe palette.

## Themes

Seven token tables share every name. The app composes three independent settings, and the themes here are the anchor presets for each:

| Setting | Values | Themes |
| --- | --- | --- |
| Ultra intensity (slider 0-100) | Stealth 0 · **Night Drive 35** · Neon 70 · Ultra 100 | `stealth`, `night` (default), `ultra`; values in between interpolate in OKLab |
| Color blind mode (checkbox) | off · on | `cvd`: Okabe-Ito data palette, cividis maps, AAA text, direct labels |
| High contrast (checkbox) | off · on | `hc`: true black, white text, 3:1+ borders everywhere, AAA; locks intensity effects off |
| Export theme (per export) | **Paper** · Graphite | `paper`, `graphite`: export-only, always color-blind safe |

Composition rule: chrome tokens (`bg-*`, `line-*`, `ink-*`, glows, `redline-*`, `grid-floor`) come from intensity; data tokens (`series-*`, `seq-*`, `div-*`, `on-*`, `cell-plate`, `plot-cursor`, status) come from Color blind mode; High contrast overrides both. Color blind mode also switches off the two effects that touch data (`bloom-series`, `plot-floor`) and raises `ink-muted`/`ink-subtle` to their AAA values.

## Voice and copy

UltraLog talks like a sharp tuner at the track: short, confident, specific. Lead with the benefit, back it with the number.

- Website: "Unlock your performance data." "14 analyzers. 18 log formats. Free, forever."
- App: name the thing and the count. "66 accepted events from 1 log · 29/104 cells filled." Not "Analysis completed successfully!"
- Errors say what happened, then what to do: "Unknown channel "Boost Pres". Check the name or pick from Available Channels."
- **Casing:** Title Case for places: mode nav, panel and window titles ("Lambda Delay", "New Computed Channel"). Sentence case for things you do or read: buttons ("Select a file", "Re-run current file"), form labels, settings ("Color blind mode"), help text and notices.
- **Units:** SI-correct casing with a space: `6297 rpm`, `-36.5 kPa`, `92 °C`, `13.4 %`, `14.7 AFR`, `0.98 λ`, `123 ms`. Channel names may stay as the ECU names them (RPM, MAP, TPS).
- **Numbers:** thin-space thousands in counters ("223 559"), fixed decimals per channel, minus sign "−" in display copy.
- **You / we:** address the user as "you" sparingly; the app never says "we". No emoji, no exclamation marks.
- **Credit the ECU:** name ECUs and formats exactly as their makers do (Haltech, ECUMaster EMU Pro, Link, AiM, rusEFI, Speeduino, MegaSquirt, TunerStudio).

## Colour

Grounds step up in lightness as surfaces come forward: `bg-void` (plots, title bar) → `bg-base` (content) → `bg-panel` (sidebars, nav, playback) → `bg-raised` (cards, windows). Inputs sink into `bg-field`. Hover is `bg-hover`, a selected neutral row is `bg-active`. Stealth makes the grounds neutral grey; Ultra pushes them toward deep purple.

- Text: `ink` for content, `ink-muted` for labels and units, `ink-subtle` for help text and axis ticks. All pass 4.5:1 on every ground in every theme, 7:1 in Color-blind safe and High contrast. `ink-disabled` is for disabled states only.
- Borders: `line` around panels and cards, `line-subtle` for internal dividers, `line-control` (3:1+) around anything you can click or type into.
- Accent: `cyan` with `cyan-soft` behind cyan text and `on-cyan` on cyan fills.
- Brand: `magenta`, `violet`, `orange`, `gold`, the stops of the sunset. In the app, `magenta` marks computed channels (ƒ) and `gold` marks favourites; `violet` and `orange` appear only as formula syntax colours.
- Status: `success`, `warning`, `danger`, `info`, each with a `*-soft` fill and always with an icon and a word. Wherever the data palette is color-blind safe, success is blue and danger is vermillion-orange.
- Data: `series-*`, `seq-*`, `div-*`, `on-seq-*`, `on-div-*`, `cell-plate`, `cell-empty` and the `plot-*` chrome. See *Data visualization*.

**The sunset signature** is a 2px gradient through `redline-0`…`redline-4` (cyan → violet → magenta → orange → gold, stops at 0/30/55/80/100%). It appears on the top edge of floating windows, the welcome screen, export headers, the splash and the website. Stealth flattens it to a grey hairline.

## Typography

Three families, each with one job:

- **Saira** (`font-display`): the motorsport voice. `display` (48/48, 800 italic) and `headline` (28/32, 700 italic) for brand moments only; `title` (18/24, 600) for panel and window titles; `overline` (11/14, 600, uppercase, 0.08em) for the mode nav, list group headers and table axis titles.
- **Outfit** (`font-ui`): the interface. `heading` 15/20 600, `body` 14/20, `label` 13/18 500, `caption` 12/16.
- **JetBrains Mono** (`font-mono`): numbers, units, formulas, file formats. `data-lg` 18/22 500, `data` 13/18, `data-sm` 11/14. Always `font-variant-numeric: tabular-nums`.

The italic in Saira echoes the forward lean of the UltraLog wordmark; keep italics for brand moments so the interface stays calm.

## Space, shape and elevation

- 4px base: `space-1` 4, `space-2` 8, `space-3` 12, `space-4` 16, `space-6` 24, `space-8` 32, `space-12` 48. Panels pad `space-4`; cards pad `space-3`; forms gap `space-2`.
- Radii are tight and technical: `radius-xs` 2 (cells, badges), `radius-sm` 4 (inputs, tabs), `radius-md` 6 (buttons, cards), `radius-lg` 10 (windows, drop zone). `radius-pill` only for slider tracks and thumbs.
- Elevation is done with ground steps and borders. `shadow-window` only under floating windows and menus.
- **Focus:** every control shows `focus-ring` (a ground-coloured gap, then a solid `cyan` ring; 5px in High contrast) on keyboard focus.
- **Motion:** quick and mechanical. 120 ms ease-out for hover and toggles, 180 ms for windows opening, no bounce. The playhead moves at log rate, never eased. Reduced motion drops glow transitions; intensity changes apply instantly.

## Logo

The mark is the badge's hexagon shield with a single trace running through it: a log line that peaks. Its border runs the sunset (cyan → violet → magenta → red-orange) and the trace runs the hot end (lime → gold → orange → magenta), as on the original badge.

- `ultralog-lockup.svg`: mark + wordmark, "Ultra" in `ink`, "Log" in the hot gradient. Default on dark grounds: website nav, splash, Graphite exports, About window.
- `ultralog-lockup-paper.svg`: the same on white, with "Ultra" in paper ink and "Log" in paper magenta. Paper exports and documents.
- `ultralog-lockup-mono-ink.svg`, `ultralog-mark-mono-ink.svg`, `ultralog-mark-mono-white.svg`: single-ink versions for one-colour print, engraving, stickers and favicons on busy grounds.
- `ultralog-mark.svg`: the mark alone; favicon (≥16px), social avatar, in-app About.
- `ultralog-app-icon.svg` / `-1024.png`: macOS/Windows/Linux app icon. The mark glows over a dusk tile with the magenta grid floor.
- `ultralog-badge.png`: the original neon badge, kept for merch and nostalgia; not the primary logo.

Clear space around the mark equals the hexagon's side wall height. Never recolour the gradients, outline the wordmark, set "UltraLog" in another face, or separate "Ultra" and "Log" into two colours other than the ones given. The wordmark is outlined Saira ExtraBold Italic.

## Imagery

- The **hero art** (`hero-sunset-dash.png`) carries the synthwave world: sunset, neon dash, perspective grid. Use it for social cards and the release-notes header.
- Inside the app, the world is suggested, not painted: the perspective-grid floor (`grid-floor`) on the drop zone and the sunset redline on windows. Ultra adds the floor inside plots.
- The chamfered hexagon (`ul-hex`) frames website feature cards; app components stay rectangular.

## Iconography

Outline icons, 24px grid drawn at 16px (20px in the activity rail), 1.6px stroke, round caps and joins, `currentColor`. In the egui app use **Phosphor** (regular weight, via `egui-phosphor`), which matches this style; the previews here draw a small set of equivalents inline. Filled glyphs only for transport (play, stop) and the favourite star. Status shapes are fixed: check = success, triangle = warning, octagon = danger, circle-i = info. The italic Saira **ƒ** in `magenta` marks a computed channel. No emoji.
