# Data visualization

Charts are the product. These rules keep every view — line plots, stacked plots, scatter density, histograms, delay and enrichment tables, the GPS track map — speaking one visual language.

## Series colours

- Assign `series-1`…`series-10` in the order channels are selected, and keep a channel's colour for the life of the session even if others are removed (no reshuffling).
- The same colour appears in three places only: the plot line, the legend swatch and the channel card strip + swatch. Channel names stay `ink`.
- Lines are `stroke-series` (1.5px); the hovered or soloed series goes to 2px and the others drop to 40% opacity.
- Night Drive order alternates warm and cool and light and dark so neighbours never blur: magenta, gold, sky, lime, orange, violet, mint, sand, coral, lilac.
- No series is ever `cyan`: cyan belongs to the cursor.
- Color-blind safe, High contrast and both export themes switch to the Okabe-Ito set; series 9-10 repeat 1-2 dashed, and lines get direct end labels.
- Ultra intensity never changes series colours; at Neon and above it adds `bloom-series` under each line.

## Plot chrome

- Canvas `plot-bg` (`bg-void`); grid `plot-grid`, every fifth line `plot-grid-major`; "Show grid" opacity scales the grid only.
- Ticks in `data-sm`, `plot-axis`. Time axis in seconds or `m:ss.mmm`; never mix in one view.
- The cursor is a full-height `plot-cursor` line at `stroke-cursor`. The playback time readout uses the same colour so the two read as one.
- Legend: floating `plot-legend-bg` box top-right, Cursor row first, then each channel with swatch, name and live value in mono.
- Stacked plots share one x-axis and cursor; each panel keeps its own series colours from the global assignment.

## Colour maps

**Sequential** (`seq-0`…`seq-6`) for magnitudes: hit counts, dead time, RPM on the track map, scatter density. Night Drive uses *Sunset* (indigo → violet → magenta → coral → orange → gold), which rises steadily in lightness, so it reads in greyscale too. Replace the current Turbo/jet rainbow everywhere: rainbows invent boundaries and fail colour-blind users.

**Diverging** (`div-0`…`div-4`) for signed values around a target: AFR or λ versus target, suggested correction %, fuel trim. Rich / negative is cool (`div-0`), on-target is the neutral `div-2`, lean / positive is hot (`div-4`). Cool-vs-hot is blue-vs-orange in both themes.

Interpolate between stops in OKLab. Value text inside cells uses `on-seq-N` / `on-div-N` on top of `cell-plate`. Color-blind safe, High contrast and Graphite show the plate so every value reaches 7:1; Paper runs the sequential map light → dark so sparse tables cost little ink.

## Tables and heatmaps

- Empty cells: `cell-empty` with a 45° hatch.
- Confidence: event-count badge fill style (filled high, outlined medium, dashed low), never hue alone.
- Selected cell: 2px `cyan` ring with a `bg-void` gap; stats for it appear under the table.
- Always show a ColorScale beside the table with the real min / max.

## Track map

GPS traces colour by the sequential map at 3px with a 1px `bg-void` casing so the line separates from a satellite background. Start/finish is a `ink` tick; the current position is a `plot-cursor` dot with a 2px `bg-void` ring.
