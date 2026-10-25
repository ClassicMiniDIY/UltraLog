# Log compare (issue #98)

Status: steps 1-6 implemented on `release/3.0.0` (2026-10-25); step 7 (IPC gaps, verify, review) next. Ships in **3.0.0**.

Issue #98 asks to overlay a second log in the same tab, with each channel showing which log it
comes from. Typical use: two WOT pulls before and after a tune change, or the same pull on two
fuel maps.

## Decisions (interview 2026-10-25)

| # | Question | Decision |
|---|---|---|
| D1 | Release | In 3.0.0. PR #105 waits for it. |
| D2 | How log B looks | Same colour as its log-A partner, **dashed**. |
| D3 | Time alignment | Numeric offset for log B with nudge buttons, plus **Align at cursor**. No auto-align. |
| D4 | How many logs | Two per tab: **A** (the tab's file) and **B** (the comparison file). |
| D5 | Channel picking | **Auto-pair** by normalized name, plus an A/B switch in the channel list for B-only channels. |

Derived from the plan review (2026-10-25, Opus), not asked:

| # | Rule | Why |
|---|---|---|
| R1 | A pair shares **one y range**: the union of A's and B's min/max, on screen and in the export. | The chart scales every channel to its own 0..1. Without this, a 1.5 bar pull and a 1.8 bar pull draw at the same height and the comparison shows nothing. |
| R2 | A pair counts as **one** against `MAX_CHANNELS` (10) and `MAX_CHANNELS_PER_PLOT`; a B-only channel counts as one. | Pairing would otherwise halve the capacity to 5. Caps are checked for the pair as a unit before adding. |
| R3 | **Computed channels are not paired and not in the B list** in 3.0.0. | A formula exists in B only if it was applied to B's file, and `remove_computed_channel` shifts indices without fixing other tabs. |
| R4 | A legend value is **None outside its log's span**, the same rule for A and B. | `find_record_at_time` returns the nearest record, so an edge value would read as live in the union range. |

## What exists today

- `SelectedChannel` carries its own `file_index`, and `render_chart_single_mode` /
  `render_single_plot` already plot each channel from its own file
  (`compute_viewport_points(selected.file_index, ..)`). A tab can hold channels from two files
  without a model change.
- Parsers re-base time to 0 at the first record, so two logs line up by elapsed time.
- Gaps (found by the plan review; each one is a step in the implementation plan):
  - **Add guards** refuse a channel whose `file_index != tab.file_index`: `add_channel`
    (app.rs ~1213), `add_channel_to_plot` (app.rs ~1445), the chart drag-and-drop drop
    (chart.rs ~390). They must also accept `compare.file_index`.
  - **Cursor** — legend values read the tab's `cursor_record` (a record of file A) for every
    channel (chart.rs `series_rows`); the Min/Max jump on a channel card writes the card's file
    time and record into the tab cursor (channels.rs ~239, ~268).
  - **Ranges** — the tab range is file A's; the chart export clips to the app-wide
    `self.time_range` (export.rs ~271), which is every file at offset 0.
  - **Track map** "Color by" lists every selected channel (track_map.rs ~392) and
    `ensure_color_cache` indexes its values by file A's records (~923).
  - **`remove_file`** remaps channel indices only in tabs whose own `file_index > index`
    (app.rs ~1176), and dropping channels must also shift `plot_areas.channel_indices` and the
    track-map colour slot, which only `remove_channel_from_plot` does, for the active tab.
  - **Direct labels** (plot_style.rs `direct_labels`) print the name only.
  - **Open file** makes the new file's tab active (`check_loading_complete`, app.rs ~877).

## Scope

In: the Log Viewer (single and stacked mode), its legend and channel cards, the timeline range,
the chart export.

Out of 3.0.0 (unchanged, read file A only): scatter plot (already has a file per side), histogram,
table generators, analysis panel. The track map stays on file A and its "Color by" lists A
channels only. MCP/IPC gets no compare commands; three known gaps are fixed in step 7
(`get_cursor_values` for file B uses A's record, handler.rs ~713; `deselect_all_channels` leaves
plot areas and `compare`, ~475; `set_time_range` vs `update_tab_time_range`). Compare state is per
session; it is not saved in `UserSettings`.

## Design

### Model (`src/state.rs`)

```rust
pub struct CompareState {
    /// The comparison file (log B).
    pub file_index: usize,
    /// Seconds added to log B's times to place them on the tab's (log A's) time axis.
    pub offset_s: f64,
    /// True while "Align at cursor" waits for the click on log B's matching point.
    pub picking_alignment: bool,
}

pub struct Tab {
    // ...
    pub compare: Option<CompareState>,
}
```

Log role is derived, not stored: a selected channel is B when
`tab.compare.is_some_and(|c| c.file_index == selected.file_index)`. Helper
`Tab::log_role(&SelectedChannel) -> LogRole { A, B }`. The tab file cannot also be log B (the
picker leaves it out).

One time convention everywhere: **tab time** is log A's time. Log B's sample at file time `t` is
drawn at `t + offset_s`. Helper `UltraLogApp::record_at(file_index, file_time) -> Option<usize>`:
the last record with `time <= file_time` (binary search, `partition_point`), and `None` before
the first or after the last record (R4). A uses `file_time = T`, B uses `T - offset_s`.

One removal path: `UltraLogApp::remove_selected_channel(tab_idx, idx)` removes a selected
channel from any tab and shifts that tab's `plot_areas.channel_indices` and track-map colour slot
(today's `remove_channel_from_plot` logic, generalised from the active tab). Removing several
goes from the highest index down. Remove pair, choosing None, changing B, and `remove_file` all
use it.

### Choosing log B

- Tool Properties (Log Viewer) gets a **Compare** row above the channel list: a dropdown with
  "None", every other loaded file (name + ECU badge), and "Open file…". Opening loads the file
  through the normal path (it also gets its own tab, as today). The request stores the asking
  tab's `Tab::id`; when the load finishes, that tab gets the file as B and becomes active again.
  The request is dropped if that tab was closed.
- File menu: **Compare with…** opens the same choice.
- Choosing B: sets `tab.compare`, auto-pairs every already-selected A channel (D5), recomputes the
  tab time range. Choosing None removes the tab's B channels (through `remove_selected_channel`)
  and clears `compare`. Changing B to another file does the same first, then pairs again.

### Pairing (D5)

- Match key: `normalize_channel_name_with_custom(name)` lowercased, the same normalization the
  chart uses, so `Engine Speed` (Haltech) pairs with `RPM` (Speeduino) when normalization is on.
  First match wins. Log channels only (R3).
- Adding an A channel while comparing also adds B's match, with the **same `color_index`**. No
  match: A is added alone, and the card shows "not in B". Caps per R2.
- The channel list gets an **A | B** segmented switch while comparing. The B list adds B channels
  (B-only allowed; a B channel whose name matches a selected A channel takes that colour, else
  the next free slot, skipping slots whose colour is already on screen in Color blind mode, where
  the palette is 8).
- Removing a channel card removes only that half. The card's menu has "Remove pair".
- Stacked mode: an auto-paired B channel goes into its partner's plot area; a B-only channel goes
  where an A channel would.

### Drawing (D2)

- `theme::series` stays as it is. New `theme::compare_series(t, idx, cvd, role)`: the colour from
  slot `idx` as today; stroke `Solid` for A and `Dashed` for B. In compare mode the stroke shows the
  log, so slots past the palette repeat colours without changing stroke. This is the one exception
  to the "past the palette changes stroke" rule. With R2 it can show only in stacked mode (more
  than 10 colour slots on screen), and the A/B tag still tells the series apart. The Theme
  Contract in CLAUDE.md documents it.
- B points: `compute_viewport_points` gets the viewport shifted by `-offset_s` (so LTTB detail
  follows the zoom as today). The cache stores **file-time** points; `offset_s` is added to the
  returned copy, never to the cached points, so a nudge smaller than one LTTB bucket (or the
  `Full` view key) still moves the line.
- **Shared y range (R1).** Normalization uses the pair's union min/max for both halves. The cached
  points hold normalized values, so the pair range goes into the cache key (or the entry is
  cleared when the pair or B changes). An unpaired channel keeps its own range.

### Legend and cards

- `series_rows`: every value comes from `record_at` (R4) instead of the tab record. Each row gets
  a mono **A** / **B** tag before the name (`plot_style::legend_overlay` gains an optional tag
  field and counts it in its measure loop); hover shows the file name.
- `direct_labels` print the tag too: in Color blind mode and High contrast A and B share the
  colour, and two identical labels would be ambiguous.
- Channel cards: the same tag; B cards show the file name in the subtitle. The Min/Max jump on a
  B card sets the cursor to `time + offset_s` and the A record at that time.

### Time alignment (D3)

- Compare row (Tool Properties) shows **Offset** for B: a `DragValue` in seconds (0.01 s steps,
  mono), nudge buttons −1 s / −0.1 s / +0.1 s / +1 s, and Reset.
- **Align at cursor**: put the cursor on an event in log A (for example the TPS rise), click
  Align at cursor, then click log B's matching point on the chart. The new offset moves that B
  point under the cursor: `offset_s += cursor_time - clicked_time`. A notice shows "Click log B's
  matching point" while picking; Esc or a second click on the button cancels. Disabled when there
  is no cursor. Pick mode takes the chart click before both cursor-setting paths (single mode
  chart.rs ~293, stacked mode ~612) and checks Esc itself: `handle_keyboard_shortcuts` returns
  early while a widget has focus, and its Esc stops playback.
- The tab time range is the union of A and shifted B
  (`min(a0, b0 + off) .. max(a1, b1 + off)`), recomputed on every compare or offset change
  (`UltraLogApp::update_tab_time_range`). The cursor is clamped into the new range. Arrow keys
  still step log A records.

### Export

- `SeriesModel` gains `log: Option<LogRole>`. `draw_sheet` dashes B series (the export palette is
  `theme::series(.., true)` plus `compare_series`) and prints the tag in the legend
  (`A · RPM`). The meta line names both files: `A pull_1.csv · B pull_2.csv · B offset +1.25 s`.
- The sheet's range is the tab range (`get_time_range()`), not the app-wide `time_range`. B's data
  is sliced at `t0 - off .. t1 - off`, then shifted, and pairs use the shared y range (R1).

### Removing files (load-bearing)

`remove_file(index)`, in **every** tab (not only tabs whose own `file_index > index`): remove
selected channels with `file_index == index` through `remove_selected_channel`, highest index
first; clear `compare` when it pointed at `index`; decrement `tab.file_index`,
`compare.file_index` and channel `file_index` values above `index` independently. Then recompute
time ranges. Without this a B channel would silently render another file's channel (same class
as the cache invalidation contract).

## Implementation plan (on `release/3.0.0`, one commit per step)

1. **Model** — `CompareState`, `Tab::compare`, `LogRole`, `log_role`, `record_at` (R4),
   `remove_selected_channel`, `update_tab_time_range`, the `remove_file` remap in every tab, add
   guards that accept `compare.file_index`, caps per R2. Tests: `record_at` with offset and
   outside the span; union range; `remove_file` with B's index below A's, above A's, and in a
   stacked tab.
2. **Chart** — shifted viewport with the offset added after the cache, shared pair range (R1),
   `compare_series`, legend tags and per-log values, direct-label tags, drag-and-drop guard
   (the drop goes through `add_channel_to_plot`, so step 1's guard covers it). Tests: strokes; a sub-bucket nudge moves the points; pair range.
3. **Channel list and cards** — Compare row (picker, None, Open file… with the tab-id request),
   A | B switch, auto-pair (a paired B channel joins its partner's plot area in stacked mode),
   Remove pair, "not in B", B card jump, track-map "Color by" filter.
   File menu entry. Tests: pairing by normalized name and colour sharing; caps; B card jump;
   pending open request; track-map filter.
4. **Alignment** — offset controls, Align at cursor (both click paths, own Esc), cursor clamp.
   Test: offset math.
5. **Export** — tab range, B slice and shift, shared pair range, `SeriesModel::log`, dashed B,
   legend tags, meta line. Tests: negative offset keeps B's early part; dump a sheet with a pair.
6. **Strings and docs** — new keys in all 15 locales (machine-drafted, listed for native
   review); CLAUDE.md (compare contract + Theme Contract exception), README feature list,
   wiki (User Guide "Compare two logs", FAQ), PR #105 description.
7. **IPC gaps and verify** — the three IPC gaps from Scope; fmt, clippy
   `--all-targets -D warnings`, tests, colour gate, contrast check; dev-build check with two
   Haltech pulls (pair, offset, align, stacked, export, close log B); Opus code review; fix round.

## Risks

- **Index-keyed state.** Every cache and plot area is keyed by index; step 1's remap and tests
  carry the load.
- **Different sample rates.** B at 20 Hz beside A at 100 Hz is fine for plotting (each series has
  its own times). Legend values use last-sample-at-or-before, same as today.
- **Normalization off.** Pairing then needs identical names; the A | B switch covers the rest.
- **3.0.0 slip.** Steps 1-4 are the feature and R1, the add guards, the cursor fixes, the export
  range and the `remove_file` remap are not optional. If time runs short, cut in this order:
  B-only channels in stacked mode, then the export legend tags (exports still draw B dashed).
  Decide at step 5, not now.
- **Existing bug, not fixed here:** `remove_computed_channel` (app.rs ~1823) and the IPC delete
  (handler.rs ~569) shift computed-channel indices without fixing any tab's selected channels.
  R3 keeps compare clear of it.
