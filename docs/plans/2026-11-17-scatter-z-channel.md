# Scatter Plot: colour by a Z channel (issue #114)

Status: plan · 2026-11-17

## Request

> Scatter Plot Z axis based on another channel instead of HITS. This could be useful for RPM ×
> Boost with AFR as Z to check the AFR, not just as an average like in the histogram.

Today the scatter plot always colours each of its 512 × 512 cells by hit count (log scale). The
Tool Properties panel shows a read-only "Z Axis: *Hits*". `ScatterPlotConfig::z_channel` already
exists in `src/state.rs` but nothing reads it.

The difference from the Histogram's *Average Z* is resolution and choice of statistic. The
Histogram averages over a coarse 16–64 grid. The scatter plot has 512 bins, so most populated
cells hold one or a few samples. The scatter plot then shows close to the individual readings,
and a **Min** or **Max** statistic shows the leanest or richest point instead of hiding it in a
mean.

## Design

### Behaviour

- **Z source** per plot (left and right independently): `Hits` (default, today's behaviour) or
  any channel of the file, **including computed channels**. A lambda-error formula is an obvious
  Z.
- **Cell value** when Z is a channel: `Mean` (default) · `Min` · `Max`. For AFR or lambda, Min is
  the richest reading and Max the leanest.
- **Colour scale**: auto range = finite min..max of the populated cells' values, with an optional
  custom range (two `DragValue`s plus *Auto*, the same pattern as the Histogram's
  `custom_x_range`). With a fixed 10–16 AFR range, left and right plots and different logs can be
  compared directly. Use `MapKind::Diverging` when the range spans zero, otherwise Sequential.
  This is the same rule as `render_table_tool_view` and the table export (Theme Contract: no new
  ramps; colours from `ColormapLut` / `heatmap`).
- **Hits in Z mode**: cells are still drawn only where hits > 0, and a cell whose Z samples are
  all NaN/sentinel draws as empty. The hover read-out and the selected-point card show
  `X · Y · Z (mean of n)`, so the sample count is never hidden.
- **Title**: `"{Y} vs {X} vs {Z}"`, where `{Z}` is the channel name plus a statistic suffix, or
  "Hits". This replaces the hard-coded `"vs Hits"`.
- **Legend**: the Z name instead of "Hits:", the gradient bar of the chosen map kind, and
  `lo–hi` in the channel's precision.
- Values are raw channel values, as the scatter axes are today (no unit-preference conversion).
  Unit conversion for all three axes is a separate follow-up.

### State (`src/state.rs`)

```rust
pub enum ScatterZ { Hits, Channel { index: usize, stat: ZStat } }   // replaces z_channel: Option<usize>
pub enum ZStat { Mean, Min, Max }
pub struct ScatterPlotConfig { …, pub z: ScatterZ, pub z_range: Option<(f64, f64)>, … }
pub struct SelectedHeatmapPoint { …, pub z_value: Option<f64> }
```

Scatter state is per-tab and session-only (not in `UserSettings`), so nothing new to persist.

### One binning function shared by screen and export

Today the on-screen and export binning have drifted apart, against the Export Contract's "the
export matches the screen":

| | On screen (`scatter_plot.rs`) | Export (`export.rs::scatter_grid`) |
|---|---|---|
| Data | `file.log.get_channel_data` (raw channels only) | `self.get_channel_data` (computed too) |
| Binning | `round()` on `(v - min) / range · (N - 1)` | `binning::uniform_bin` |
| NaN | not filtered (`NaN as usize` = bin 0, bounds can be wrong) | skipped; `finite_bounds` |

Add a pure function in `src/ui/scatter_plot.rs`, or a small `src/analysis/scatter.rs` if it
grows:

```rust
pub fn bin_scatter(x: &[f64], y: &[f64], z: Option<&[f64]>, bins: usize) -> Option<ScatterGrid>
// ScatterGrid { hits: Vec<u32>, z_sum/z_min/z_max: Option<Vec<f32>> (flat, NaN = empty),
//               x_min, x_max, y_min, y_max, max_hits }
```

- It uses `finite_bounds` (now private in `export.rs`; move it beside `uniform_bin` in
  `analysis::tables::binning`) and `binning::uniform_bin` (the histogram and table generators use
  it too), skips samples whose X or Y is not finite, and skips the Z contribution of a non-finite
  or `events::is_invalid_sample` Z.
- It computes sum, min and max in one pass, so switching Mean/Min/Max does not rebuild anything.
- On screen it is called with `HEATMAP_BINS` (512); `scatter_grid` calls it with `SCATTER_BINS`
  (96). The export keeps its deliberately coarser grid, but the rules are now the same.
- It requires `x.len() == y.len() == z.len()`, the same alignment rule as `require_aligned`.
  A ragged computed channel refuses with a "channels have different lengths" placeholder instead
  of zipping misaligned data.

### Cache (`scatter_histogram_cache`)

- Key becomes `(file, x, y, z: Option<usize>)`. Statistic and colour range are applied at draw
  time, not in the key.
- Memory: one entry is ~1 MiB of hits plus 3 MiB of Z planes at 512². Store flat `Vec`s instead
  of today's `Vec<Vec<u32>>`. The cache is unbounded today. Clear entries for other `(x, y, z)`
  of the same plot when its selection changes, so it holds at most two live entries per tab.
- **Invalidation bug to fix first:** `remove_computed_channel` (`src/app.rs`) clears
  `downsample_cache` and `minmax_cache` but **not** `scatter_histogram_cache`, although CLAUDE.md
  says it does. It has not mattered because the scatter plot could not show computed channels.
  Once it can, clear the cache there, and wherever a computed channel's `cached_data` is
  re-evaluated (formula edit, binding change). Otherwise a stale grid renders against a shifted
  index.

### UI (`render_scatter_plot_controls`)

- The left and right sections are copy-pasted. Fold them into one
  `render_scatter_axis_section(ui, side: Side)` before adding a third combo, which would
  otherwise triple the duplication.
- Channel lists include computed channels, built as `histogram.rs` builds them
  (`base_channel_count + computed_idx`). Use a Phosphor icon, not the `📊` emoji the histogram
  uses (Theme Contract: no emoji).
- Z combo: "Hits" first, then channels. Under it, when Z is a channel: a `components::segmented`
  for Mean / Min / Max and the range row (Auto checkbox + two DragValues).
- Keep the Histogram unchanged.

### Export (`export.rs::scatter_grid`)

- In Z mode: `norm` from the chosen statistic over `z_range` (custom or auto), `kind`
  Sequential/Diverging by the same rule as the screen, `scale_title` = Z name + statistic,
  `scale_min/max` = range, `rich_lean: false`. The caption becomes `"{Y} vs {X} · {Z}"`.
- In Hits mode: output unchanged.

### MCP / IPC (small, backward compatible)

`IpcCommand::ShowScatterPlot` and `ShowScatterPlotRequest` gain
`#[serde(default)] z_channel: Option<String>` and `z_stat: Option<String>` (`mean|min|max`).
Old clients keep working. Add a test beside `show_scatter_plot_targets_the_requested_files_tab`.

### i18n

New keys under `scatter:` in all 15 `i18n/*.yaml`: `z_source_hits`, `cell_value`, `stat_mean`,
`stat_min`, `stat_max`, `z_range`, `z_auto`, `mixed_lengths`. Translate them with the
i18n-translator agent. Also move the hard-coded `"Select X and Y axes"`, `"Left plot"` /
`"Right plot"` and the `"X: … Hits: …"` tooltip into keys while touching them.

## Tests

- `bin_scatter`: known points land in the expected cells; Mean/Min/Max per cell; NaN X/Y skipped;
  NaN or sentinel Z excluded from Z but counted in hits; constant channel (zero range) does not
  divide by zero; length mismatch returns `None`.
- Screen and export agree: the same inputs at `bins = SCATTER_BINS` give the same cell values the
  export draws.
- Cache: removing a computed channel clears `scatter_histogram_cache` (regression test for the
  bug above).
- Diverging/sequential choice: range −0.1..0.1 → Diverging, 10..16 → Sequential.
- IPC: `show_scatter_plot` with and without `z_channel`.
- Visual check: `EXPORT_SHEET_DUMP=<dir> cargo test --lib dump_ -- --ignored` with a Z-mode
  scatter added to the dump test.

## Docs

- `CLAUDE.md`: fix the cache-invalidation paragraph (scatter cache key and the call sites),
  describe the scatter plot as "hit count or a Z channel (mean/min/max)" in the UI module list and
  Key Features, and note in the Export Contract that screen and export share `bin_scatter`.
- Wiki `User-Guide.md` Scatter Plot section: the Z selector, the statistics, the fixed range.

## Order of work

1. `bin_scatter` + tests; switch screen and export to it (no visible change except the NaN fix).
2. Computed channels in the scatter pickers + cache-invalidation fix + test.
3. `ScatterZ` state, Z controls (after folding the left/right duplication), rendering, legend,
   tooltip, selected point.
4. Export Z mode.
5. IPC/MCP parameters.
6. i18n, CLAUDE.md, wiki.

Steps 1–2 can land on their own as a bug-fix PR. Steps 3–6 are the feature.
