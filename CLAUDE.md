# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Structure

This project uses two repositories:

1. **UltraLog** (main repo) - Contains all source code, build configuration, and tests
2. **UltraLog.wiki** (separate repo) - Contains GitHub wiki documentation

The wiki is managed as a separate Git repository (standard GitHub wiki setup). When updating documentation:

- Code documentation (README.md, CLAUDE.md) stays in the main repo
- User-facing wiki pages (User-Guide.md, Supported-ECU-Formats.md, etc.) go in the wiki repo
- The wiki repo is typically located adjacent to the main repo (e.g., `../UltraLog.wiki/`)

## Project Overview

UltraLog is a high-performance ECU (Engine Control Unit) log viewer written in pure Rust. It parses log files from automotive ECUs (Haltech, ECUMaster, RomRaider, Speeduino, rusEFI, AiM, Link, etc.) and displays channel data as interactive time-series graphs with support for computed/virtual channels derived from mathematical formulas.

## Build Commands

The crate is on **edition 2024** with `rust-version = "1.95"`. The MSRV is set by
egui/eframe 0.36, not by the edition (2024 itself only needs 1.85) — so bumping
egui is what moves it. Edition 2024 means let-chains (`if a && let Some(b) = ..`)
are available and clippy's `collapsible_if` will ask for them, and that rustfmt
sorts imports with the 2024 style ordering.

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Run the application
cargo run --release

# Run the test parser CLI utility
cargo run --bin test_parser

# Run tests
cargo test

# Check formatting
cargo fmt --all -- --check

# Run clippy lints
cargo clippy -- -D warnings
```

## Architecture

### Source Structure

```text
src/
├── main.rs           # Application entry point
├── lib.rs             # Library exports and module declarations
├── app.rs             # Main application state and eframe::App impl
├── state.rs           # Core data types and constants
├── settings.rs        # UserSettings — persisted user preferences (see Settings Persistence below)
├── units.rs           # Unit preference types and conversions
├── normalize.rs       # Field name normalization system
├── computed.rs        # Computed channels data types and library
├── colormap.rs        # Colour-map entry point: theme Sequential / Diverging maps (no rainbow LUTs)
├── laps.rs            # GPS coordinate-format normalization, track sanitization, lap detection
├── tiles.rs           # Map tile providers, bounded worker pool, disk/texture caches
├── expression/
│   ├── mod.rs         # Formula parsing, channel refs, time shifts, evaluation
│   └── engine.rs      # Built-in expression compiler/evaluator (replaced meval)
├── updater.rs          # Auto-update functionality
├── analytics.rs        # Privacy-respecting analytics
├── i18n.rs             # rust-i18n language/locale selection
├── adapters/
│   ├── mod.rs        # Adapter module exports
│   ├── types.rs      # OpenECU Alliance spec types (AdapterSpec, ProtocolSpec, etc.)
│   ├── api.rs        # API client for fetching specs from openecualliance.org
│   ├── cache.rs      # Local disk cache at {app_data_dir}/UltraLog/oecua_specs/
│   └── registry.rs   # Spec registry with fallback chain (cache -> embedded -> API)
├── analysis/
│   ├── mod.rs         # Analyzer trait + shared analysis framework
│   ├── afr.rs         # AFR/Lambda analysis (fuel trim drift CUSUM, rich/lean zones)
│   ├── derived.rs      # Derived metrics (Volumetric Efficiency, injector duty cycle, ...)
│   ├── filters.rs      # Signal-processing filters (moving average, etc.)
│   ├── statistics.rs   # Descriptive statistics and correlation analysis
│   └── tables/         # Table generators (issues #3, #4) — see Table Generators below
│       ├── mod.rs          # TableAnalyzer trait, TableEvent/RejectReason/RunReport, TableAccumulator
│       ├── stats.rs        # median/MAD/percentile, update instants, effective update interval, robust sigma
│       ├── binning.rs      # AxisSpec (edge lists), CellStats, Confidence, TableGrid, uniform_bin
│       ├── channel_map.rs  # ChannelRole, ChannelMapping, three-tier auto-suggestion
│       ├── events.rs       # invalid-sample masking, steadiness, interpolated crossings, rate runs
│       ├── lambda_delay.rs # Lambda delay table generator (#4)
│       ├── accel_enrich.rs # Acceleration enrichment table generator (#3)
│       ├── export.rs       # CSV / clipboard TSV rendering, delay unit conversion
│       └── synthetic.rs    # Synthetic-log builder + xorshift RNG for ground-truth tests
├── ipc/
│   ├── mod.rs          # IPC module exports, DEFAULT_IPC_PORT
│   ├── commands.rs     # IpcCommand/IpcResponse wire types shared with mcp/client.rs
│   ├── handler.rs      # UltraLogApp command handlers (executes commands from the MCP server)
│   └── server.rs       # TCP IPC server — background thread, wakes the GUI via a repaint callback
├── mcp/
│   ├── mod.rs          # MCP module exports, DEFAULT_MCP_PORT
│   ├── client.rs       # TCP client the MCP server uses to talk to the running GUI's IPC server
│   └── server.rs       # Embedded MCP HTTP server (rmcp + axum) for Claude Desktop integration
├── parsers/
│   ├── mod.rs                     # Parser module exports
│   ├── types.rs                   # Core parser types (Log, Channel, Value, EcuType, etc.)
│   ├── haltech.rs                 # Haltech ECU log parser
│   ├── ecumaster.rs               # ECUMaster EMU Pro CSV parser
│   ├── romraider.rs               # RomRaider CSV parser
│   ├── speeduino.rs               # Speeduino/rusEFI MLG binary parser
│   ├── aim.rs                     # AiM XRK/DRK binary parser
│   ├── link.rs                    # Link ECU LLG binary parser
│   ├── woolich.rs                 # Woolich Racing Tuned CSV parser (motorcycle ECUs)
│   ├── emerald.rs                 # Emerald K6/M3D ECU (.lg1/.lg2) binary parser
│   ├── locomotive.rs               # Locomotive datalogger CSV parser (TimeStamp/Customer/UnitNumber header)
│   ├── megasquirt.rs               # MegaSquirt (MS1/MS2/MS3) TunerStudio CSV parser
│   ├── mhd.rs                      # MHD Tuning CSV parser (BMW N54/N55/S55/B58)
│   ├── msl.rs                      # TunerStudio MSL (legacy ASCII, tab-delimited) parser
│   ├── motorsport_electronics.rs   # Motorsport Electronics ME221/ME442 (ME Tuner) CSV parser
│   ├── racechrono.rs               # RaceChrono CSV v3 session export parser (GPS + sensors + OBD/CAN)
│   ├── torque.rs                   # Torque Pro (Android OBD-II app) trackLog CSV parser
│   ├── bluedriver.rs               # BlueDriver OBD-II scan tool CSV parser
│   └── dynamicefi.rs               # DynamicEFI EBL WhatsUp (GM TBI) CSV parser
└── ui/
    ├── mod.rs                        # UI module exports
    ├── activity_bar.rs               # VS Code-style vertical icon strip for panel navigation
    ├── side_panel.rs                 # Container that routes to the active panel by activity-bar selection
    ├── files_panel.rs                # File management, loading, and file list (current files panel)
    ├── tools_panel.rs                # Analysis tools, computed channels library, export options
    ├── settings_panel.rs             # Consolidated settings (display, units, normalization, updates)
    ├── tool_properties_panel.rs      # Dynamic panel showing controls for the active tool (channels / histogram / scatter)
    ├── analysis_panel.rs             # Window for running analysis algorithms (src/analysis) on the active log
    ├── table_generator.rs            # Table tools: Tool Properties setup + central heatmap / inspector view
    ├── table_export.rs               # ExportSheet model for generated tables (`table_sheet`)
    ├── data_panel.rs                 # Right-side data panel hosting DataWidget panes (rail, header, hide/restore)
    ├── widgets/
    │   ├── mod.rs                    # DataWidget trait + static widget registry
    │   └── track_map.rs              # GPS Track Map widget (polyline, laps, tile backgrounds)
    ├── channels.rs                   # Selected-channel cards
    ├── chart.rs                      # Chart rendering, legends, LTTB algorithm
    ├── timeline.rs                   # Timeline scrubber and playback controls
    ├── menu.rs                       # Menu bar (Units, Tools, Help menus)
    ├── toast.rs                      # Toast notification system
    ├── tab_bar.rs                    # Multi-file tab interface
    ├── fonts.rs                      # Font families (per weight), script/CJK fallbacks, the type scale as TextStyles
    ├── formula_syntax.rs             # Formula tokenizer + LayoutJob colouring (editor, help window)
    ├── plot_style.rs                 # Chart chrome: series strokes, bloom, Ultra floor, cursor, custom legend, direct labels
    ├── heatmap.rs                    # Heatmap/table cells: seq/div fill, value plate, hatch, count badges, ring, ColorScale
    ├── components/
    │   ├── mod.rs                    # Night Drive components; re-exports Phosphor as `icon`
    │   ├── button.rs                 # primary / secondary / ghost / danger / sm / icon buttons
    │   ├── controls.rs               # segmented, checkbox, Ultra intensity slider
    │   ├── notice.rs                 # Status (icon + word), notice, status and ECU badges
    │   └── window.rs                 # window(): Saira title; finish_window(): redline + Esc
    ├── theme/
    │   ├── mod.rs                    # Night Drive runtime: tokens_at, series/seq/div, egui Visuals, install/current
    │   ├── tokens.rs                 # GENERATED from design/tokens/tokens.json — never edit by hand
    │   └── codegen.rs                # tokens.json -> tokens.rs generator (cargo run --example gen_theme)
    ├── tool_switcher.rs              # Switch between the five tools (ActiveTool::ALL)
    ├── scatter_plot.rs               # XY scatter plot visualization
    ├── histogram.rs                  # 2D histogram/heatmap view for channel distributions
    ├── export.rs                     # Builds the ExportSheet per tool; file export and Copy image
    ├── export_dialog.rs              # ExportDialog: format, Paper/Graphite, Copy image / Export
    ├── about_window.rs               # About window (Help menu): lockup on bg_void, version, links
    ├── export_sheet/                 # ExportSheet layout + Canvas back ends (see Export Contract)
    │   ├── mod.rs                    # Sheet / ChartModel / GridModel, draw_sheet, render_png/pdf
    │   ├── canvas.rs                 # Canvas trait (points, top-left origin, baseline text)
    │   ├── text.rs                   # skrifa layout, font fallback, glyph outlines
    │   ├── png.rs                    # vello_cpu back end (2×), lockup composited after render
    │   └── pdf.rs                    # printpdf back end, bundled TTFs embedded
    ├── normalization_editor.rs       # Custom field mapping editor
    ├── computed_channels_manager.rs  # Computed channels library UI
    ├── formula_editor.rs             # Formula creation and editing
    └── update_dialog.rs              # Auto-update notification dialog
```

### Core Modules

- **`app.rs`** - Main `UltraLogApp` struct with application state. Contains:
  - File loading (background threads via `std::sync::mpsc`)
  - Channel management (add/remove/color assignment)
  - Cursor and time range tracking
  - eframe::App implementation

- **`state.rs`** - Core data types:
  - `LoadedFile` - Represents a parsed log file
  - `SelectedChannel` - A channel selected for visualization
  - `CacheKey`, `LoadResult`, `LoadingState`, `DownsampleViewKey` - Internal state types
  - No colour constants: series colours come from `theme::series` (see the Theme Contract)

- **`settings.rs`** - `UserSettings` persistence (see [Settings Persistence Contract](#settings-persistence-contract) below)

- **`i18n.rs`** - Internationalization: the `Language` enum and locale selection for `rust-i18n`

- **`units.rs`** - Unit preference system:
  - Enums for each unit type (Temperature, Pressure, Speed, etc.)
  - `UnitPreferences` struct for storing user selections
  - Conversion methods between metric/imperial units

- **`normalize.rs`** - Field name normalization:
  - Maps ECU-specific channel names to standardized names
  - Built-in mappings for common channels across ECU systems
  - Custom mapping support via UI editor

- **`computed.rs`** - Computed/virtual channels:
  - `ComputedChannelTemplate` - Reusable formula templates with metadata
  - `ComputedChannel` - Instantiated channel with bindings and cached data
  - `ComputedChannelLibrary` - Global persistent library stored as JSON
  - Support for time-shifting: index offsets (e.g., `RPM[-1]`) and time offsets (e.g., `RPM@-0.1s`)

- **`expression/`** - Formula evaluation engine:
  - `engine.rs` - Self-contained expression compiler/evaluator (zero deps; replaced the unmaintained meval crate while preserving its exact grammar: `+ - * / % ^` with standard precedence, right-assoc `^`, `-2^2 == -4`, variadic `min`/`max`, `pi`/`e`/`tau`/`phi` constants)
  - Formulas compile once to an RPN instruction list; per-record evaluation fills variable slots from log data with no parsing, hashing, or allocation in the loop
  - Extracts channel references with time-shift syntax (regex preprocessing, unchanged)
  - Validates formulas against available channels
  - Statistical variables (`_mean_X`, `_stdev_X`, ...) resolve to constants at compile time — callers must pass statistics via `evaluate_all_records_with_stats` when `formula_uses_statistics()` is true (a formula using them without statistics is now an error instead of silent zeros)

- **`updater.rs`** - Auto-update system:
  - Checks GitHub releases for new versions
  - Downloads platform-specific binaries
  - Handles installation on Windows, macOS, and Linux
  - Supports seamless background updates

### Adapter System (src/adapters/)

The adapter system integrates with the **OpenECU Alliance** specification ecosystem, providing runtime access to adapter and protocol definitions with automatic updates.

- **`types.rs`** - OpenECU Alliance spec types:
  - `AdapterSpec` - Log file format adapter definitions with channel specifications
  - `ProtocolSpec` - CAN/network protocol definitions for real-time streaming
  - `ChannelSpec` - Channel metadata (name, category, unit, min/max, precision)
  - `MessageSpec`, `SignalSpec` - CAN message and signal definitions
  - `ChannelCategory` - Categorization enum (Engine, Fuel, Ignition, etc.)

- **`api.rs`** - OpenECU Alliance API client:
  - Fetches specs from `https://openecualliance.org/api/`
  - Endpoints: `/api/adapters`, `/api/adapters/{vendor}/{id}`, `/api/protocols`, `/api/protocols/{vendor}/{id}`
  - HTTP client using `ureq` with custom user agent
  - Error handling for network and parsing failures

- **`cache.rs`** - Local disk cache:
  - Cache location: `{app_data_dir}/UltraLog/oecua_specs/adapters/` and `.../protocols/`
  - Cache metadata tracks last fetch timestamp, version, and counts
  - 24-hour staleness threshold (configurable)
  - Individual JSON files per adapter/protocol (e.g., `haltech-haltech-nsp.json`)
  - Cache clearing and age inspection utilities

- **`registry.rs`** - Spec registry with multi-tier fallback:
  - **Embedded specs** - Compile-time YAML files from `spec/OECUASpecs/` git submodule
  - **Cache layer** - Loads from disk cache if fresh (< 24 hours old)
  - **API refresh** - Background thread fetches updates on app startup (non-blocking)
  - **Normalization maps** - Builds source_name → display_name mappings for field normalization
  - **Metadata lookup** - Provides channel metadata (category, unit, min/max, precision)
  - Thread-safe `RwLock` for dynamic spec updates without restart

### Analysis System (src/analysis/)

Trait-based framework (`Analyzer`) for algorithms that process log data and can feed results back into the computed-channels system:

- **`afr.rs`** - AFR/Lambda analysis: fuel trim drift detection (CUSUM), rich/lean zone detection, AFR deviation. Supports both AFR and Lambda units with automatic detection.
- **`derived.rs`** - Derived engine metrics: Volumetric Efficiency (from MAF or MAP/IAT speed-density), injector duty cycle, and similar calculated values.
- **`filters.rs`** - Signal-processing filters for smoothing and noise reduction (e.g., moving average).
- **`statistics.rs`** - Descriptive statistics and correlation analysis.

The `analysis_panel.rs` UI module (see below) hosts these analyzers, with category tabs and configurable parameters per algorithm.

### Table Generators (src/analysis/tables/)

Two generators mine events out of loaded logs into 2-D tuning tables (design doc:
`docs/plans/2026-07-16-tuning-table-generators.md`, revised per the 2026-09-17 review on issues #3/#4):

- **`lambda_delay.rs`** (issue #4) - injector pulse-width steps → first wideband crossing of
  `max(k·σ, min_delta)`, binned RPM × load (MAP or TPS). Primary value is dead time (ms); t63,
  response magnitude and step size are alternate measures. Strict/relaxed gating profiles.
- **`accel_enrich.rs`** (issue #3) - tip-ins from a native throttle-rate channel (scale detected;
  Haltech's parser already divides ×10) or a computed TPS/MAP derivative → delay-compensated
  signed excursion vs target/baseline, binned RPM × peak rate. Measures: correction %, depth,
  duration, area-based %, area, delay used. When an `AeActive` role is mapped the table kind is
  *additional* (multiply the ECU's current value) and events without ECU activity are rejected
  (`AeKindMismatch`) so one cell never mixes kinds.

They implement the sibling `TableAnalyzer` trait (not `Analyzer`, whose result is one value per
timestamp). Every event, accepted or rejected with a `RejectReason`, stays in the list so the
inspector and the run report's rejection breakdown ("41 events found · 39 rejected: 30 unsteady,
9 no response") can tell the user what to log next. `TableAccumulator` keeps raw per-event values
(medians/MADs cannot be merged incrementally) so logs can be added and removed, and re-bins per
measure on demand. Cells are median + MAD + `Confidence` (Empty / Low / Medium / High); empty and
low cells are never interpolated and export blank.

**Load-bearing behaviors:**

- **Sample-and-hold interpolation** (`events::find_crossing`, `stats::update_instants`) - CAN
  widebands often update at 10-20 Hz inside a 50-500 Hz log. Noise (`robust_sigma_diff`) and
  crossings are computed over distinct-value *update instants*, never raw samples (most raw first
  differences are exactly zero). The previous reading for interpolation is the last update, but no
  earlier than `t - effective_update_interval`: a flat baseline is still being sampled every
  interval, so interpolating from its last value change would place every crossing far too early.
- **Invalid-sample masking** (`events::mask_invalid`) - ECUs write an i32 sentinel family
  (`-2147483617`, `…637`, …) for "no reading"; those and out-of-band samples become `NaN` before any
  math, and a window with >10 % masked lambda rejects as `InvalidSamples`. Haltech sentinels never
  get this far: the Haltech parser fills them (see its load-bearing behaviors), so a Haltech wideband
  dropout reaches the generators as a held value and is caught by steadiness / band gating instead.
- **Alignment** (`tables::mapped_column`) - `Log::get_channel_data` drops ragged rows, so a column
  can be shorter than `times`; the generators refuse with `ComputationError` rather than index
  misaligned pairs (same contract as `channel_series` in the IPC handler).
- **Log identity** - accumulated events key on `LoadedFile::load_id`, a per-load nonce, never the
  file index (shifts when a tab closes) or the bare file name (every rusEFI install has a
  `Log1.mlg`). The UI's mapping cache keys on the same nonce.
- **Lambda-delay → AE composition** - `GeneratorContext::delay_table` carries the session's
  lambda-delay grid; an AE event uses the matching cell's median when that cell is Medium/High,
  else `assumed_delay_ms`, and records which in `TableEvent::note` and the `delay_used_ms` measure.
- **Axis cap** - `binning::MAX_BINS_PER_AXIS` (64) keeps any future MCP payload well under the
  512 KiB response guard. `histogram.rs` now calls `binning::uniform_bin` for its cell math so both
  tools agree on boundaries.
- **Tools, not a window** - `ActiveTool::LambdaDelay` / `ActiveTool::AccelEnrich` render like
  Histogram: setup in the Tool Properties panel (`render_table_tool_properties`), results in the
  central panel (`render_table_tool_view`). `ActiveTool::generator_kind()` is the one helper the
  match sites use, and `ActiveTool::ALL` fixes the switcher / View-menu / Cmd+1..5 order, so a new
  tool is appended there and nowhere else. Tool switches go through `UltraLogApp::set_active_tool`
  (analytics + table selection reset), not direct assignment.
- **State is app-level, not per-tab** - unlike `Tab::histogram_state`, `UltraLogApp::table_generator`
  holds the accumulators, mappings and export options for both generators. The tables are
  multi-log by design, so the active tab only decides which file *Run / Add current file* reads;
  switching tabs or tools does not lose a table.
- **Exports go through the ExportSheet** - `table_export::table_sheet` builds a `GridModel`
  (values, count badges whose fill style shows confidence, edge labels) and `ui::export_sheet`
  draws PNG and PDF from it (see the Export Contract below). Cell values go through
  `tables::export::cell_value`, so PNG, PDF, CSV and clipboard blank the same cells and convert
  delay units identically.
- **Auto-suggestion** (`channel_map::suggest_mapping`) - normalization hit (100) → strong name
  hints (50) → spec category + hint (60) → generic hints (40), then a data-plausibility veto on the
  channel median; `overall|avg|average` names lose 10 points so a single sensor beats an averaged
  one (averaging sensors with different transport delays smears the rise). Ties within 10 points
  are flagged ⚠ in the UI. New built-in normalization entries back this: injector on-time names →
  `Pulse Width`, and `TPS DOT` / `Throttle Position Derivative` / `TPS Delta` → `TPS Rate`.

Not yet implemented from the design: MCP tools for the generators, per-ECU mapping presets on
disk (mapping and accumulators are session-only), the windowed cross-correlation mode, and the
wiki page.

### IPC + MCP System (src/ipc/, src/mcp/)

UltraLog embeds an MCP (Model Context Protocol) HTTP server so Claude Desktop can drive the running GUI — select channels, add computed channels, and query log data.

- **`mcp/server.rs`** - The MCP server itself (`rmcp` + `axum`), served over HTTP at `http://localhost:52385/mcp` (`DEFAULT_MCP_PORT`). This is what Claude Desktop connects to.
- **`mcp/client.rs`** - A TCP client the MCP server uses internally to talk to the GUI's IPC server. Each command opens a fresh TCP connection (no persistent/stale connections).
- **`ipc/server.rs`** - A TCP server (`DEFAULT_IPC_PORT` = 52384) that runs in a background thread inside the GUI process and receives commands from `mcp/client.rs`.
- **`ipc/commands.rs`** - The shared `IpcCommand`/`IpcResponse` wire types.
- **`ipc/handler.rs`** - `UltraLogApp` methods that execute each `IpcCommand` against live app state.

**Load-bearing contract:** the IPC server wakes the GUI via a repaint callback (`request_repaint()`), not a polling timer — see `IpcServer::start_with_repaint` in `src/ipc/server.rs` and its wiring in `UltraLogApp::new`. Incoming commands are drained in `UltraLogApp::process_ipc_commands`, which caps processing at **10 commands per frame** to avoid blocking the UI thread; if more are queued, it requests another repaint to continue next frame.

**Response size contract (load-bearing):** every MCP tool result travels as a single
Server-Sent Event, and streamable-HTTP MCP clients cap one SSE event at **1 MiB**
(`DEFAULT_MAX_EVENT_SIZE_BYTES` in the reference client). An event over that cap is
discarded inside the client's SSE decoder — the caller receives no value *and* no error,
so the tool call simply never returns, even past the IPC layer's own 30s timeout. That
is issue #80's cousin for the MCP path: issue #88, where `evaluate_formula` worked up to
~22,000 rows and hung on everything larger.

Two defenses keep that from recurring, and both must stay in place:

1. **Sample budget** — `UltraLogApp::limit_samples` (`src/ipc/handler.rs`) reduces any
   per-record series to `DEFAULT_MAX_POINTS` (2000), clamped to `MAX_POINTS_LIMIT`
   (10,000), using the chart's LTTB so peaks and dropouts survive. `GetChannelData` and
   `EvaluateFormula` carry an optional `max_points`; responses report `total_samples` and
   `downsampled` so a caller can tell what it got. `FindPeaks` is capped at `MAX_PEAKS`
   (500) because peak count scales with channel noise rather than with anything the
   caller asked for; it *selects* by prominence but *returns* chronologically, and
   reports `total_peaks`/`truncated` — a bare truncated list is indistinguishable from a
   complete one, so a caller counting events would read exactly 500 and believe it.
2. **Byte guard** — `UltraLogMcpServer::json_result` (`src/mcp/server.rs`) serializes
   compactly (never `to_string_pretty`: one array element per line roughly doubles the
   payload for no benefit) and refuses anything over `MAX_RESPONSE_BYTES` (512 KiB) with
   an error naming `max_points` and the time range. An honest error beats silence.

**Corollary:** anything needing exact aggregates must read the *full* series, not a
downsampled response. `UltraLogApp::channel_series` is that accessor, and
`handle_get_channel_stats`, `handle_find_peaks` and `handle_correlate_channels` all go
through it. `handle_evaluate_formula` does not use it — it has already evaluated its own
series — but it holds the same invariant by calling `compute_stats` *before*
`limit_samples`. Either way the rule is the same: routing an aggregate through a
downsampled series would silently compute statistics over 2000 samples instead of 178,000.

`channel_series` also enforces that `times` and `values` are the same length
(`require_aligned`). `Log::get_channel_data` is a `filter_map` that drops a row missing
the column, and an analysis-derived `cached_data` is only as long as the algorithm made
it, so a ragged log yields a pair that is *misaligned*, not merely short — and
`downsample_lttb` indexes `values` off `times.len()`, which panics on the GUI thread.
`src/ui/chart.rs` refuses to plot that case; the data API refuses to serve it.

### UI Modules (src/ui/)

UI rendering is split into focused modules that implement methods on `UltraLogApp`. The layout is a VS Code-style activity bar + side panel:

- **`activity_bar.rs`** - Vertical icon strip on the far left for switching between side-panel sections (Files / Tool Properties / Tools / Settings)
- **`side_panel.rs`** - Container that routes to the panel selected in the activity bar
- **`files_panel.rs`** - File management, loading, and file list
- **`tools_panel.rs`** - Analysis tools, computed channels library, and export options, inline in the side panel
- **`settings_panel.rs`** - Consolidated settings: display, units, normalization, updates
- **`tool_properties_panel.rs`** - Dynamic panel showing controls for the active tool: channel selection for Log Viewer, axis/grid/filter controls for Histogram, controls for Scatter Plot
- **`analysis_panel.rs`** - Window for running `src/analysis` algorithms (filters, statistics, AFR, derived metrics) against the active log, with category tabs
- **`channels.rs`** - Selected-channel cards (`render_selected_channels`); channel picking lives in `tool_properties_panel.rs`
- **`chart.rs`** - Main chart with egui_plot, min/max legend overlay, LTTB downsampling, normalization
- **`timeline.rs`** - Bottom panel: playback controls (play/pause/stop), speed selector, timeline scrubber
- **`menu.rs`** - Top menu bar (File, Edit, View, Help; Units submenu with 8 unit categories)
- **`toast.rs`** - Toast notification overlay for user feedback
- **`tab_bar.rs`** - Chrome-style tabs for multi-file support
- **`tool_switcher.rs`** - Switch between the five tools in `ActiveTool::ALL` (Log Viewer, Scatter Plot, Histogram, Lambda Delay, Accel Enrichment)
- **`scatter_plot.rs`** - XY scatter plot for channel correlation analysis
- **`histogram.rs`** - 2D histogram/heatmap view of channel distributions, with configurable cell coloring (average Z-value or hit count)
- **`export.rs`** / **`export_dialog.rs`** / **`export_sheet/`** - the ExportDialog and the one ExportSheet layout every PNG/PDF export uses (see the Export Contract)
- **`normalization_editor.rs`** - Dialog for creating custom field name mappings
- **`computed_channels_manager.rs`** - Window for managing computed channel library
- **`formula_editor.rs`** - Dialog for creating/editing computed channel formulas
- **`update_dialog.rs`** - Update notification and download progress dialog

### Parser System

The parser system uses a trait-based design for supporting multiple ECU formats:

- **`parsers/types.rs`** - Core types: `Log`, `Channel`, `Value`, `Meta`, `EcuType`, `ComputedChannelInfo`, and the `Parseable` trait
- **`parsers/haltech.rs`** - Haltech CSV parser (NSP exports)
- **`parsers/ecumaster.rs`** - ECUMaster EMU Pro CSV parser (semicolon/tab delimited)
- **`parsers/romraider.rs`** - RomRaider CSV parser with unit extraction from headers. Also the
  catch-all for generic `Time,...` CSVs that no earlier parser claims — see the time-unit
  contract below.
- **`parsers/speeduino.rs`** - Speeduino/rusEFI MLG binary format parser
- **`parsers/aim.rs`** - AiM XRK/DRK binary format parser for motorsport data loggers
- **`parsers/link.rs`** - Link ECU LLG binary format parser
- **`parsers/woolich.rs`** - Woolich Racing Tuned CSV parser (HH:MM:SS.mmm timestamps, boolean channels)
- **`parsers/emerald.rs`** - Emerald K6/M3D ECU binary parser (`.lg2` channel definitions + `.lg1` 24-byte timestamped records)
- **`parsers/locomotive.rs`** - Locomotive datalogger CSV parser (detected via `TimeStamp:` / `Customer:` header lines, day-of-week-prefixed data rows)
- **`parsers/megasquirt.rs`** - MegaSquirt (MS1/MS2/MS3/MS3Pro) TunerStudio CSV parser
- **`parsers/mhd.rs`** - MHD Tuning CSV parser (BMW N54/N55/S55/B58 flashed with MHD)
- **`parsers/msl.rs`** - TunerStudio MSL parser (tab-delimited legacy ASCII datalog; header row + units row; also written by RealDash)
- **`parsers/motorsport_electronics.rs`** - Motorsport Electronics ME221/ME442 (ME Tuner) CSV parser
- **`parsers/racechrono.rs`** - RaceChrono CSV v3 session export parser (lap-timing app; GPS + phone sensors + OBD/CAN merged on a unix-time base)
- **`parsers/bluedriver.rs`** - BlueDriver OBD-II scan tool CSV parser (UTF-16 with BOM)
- **`parsers/dynamicefi.rs`** - DynamicEFI EBL WhatsUp CSV parser (modified GM TBI systems)
- **`parsers/torque.rs`** - Torque Pro trackLog CSV parser (Android OBD-II app; phone GPS + accelerometer + OBD-II/custom PIDs)

Note: `EcuType` also reserves `Aem`, `MaxxEcu`, and `MotEc` variants for formats without an implemented parser yet (no `detect()`/`Parseable` wiring in `app.rs`) — don't assume these are supported just because the enum has a slot for them.

**Supported ECU Systems:**

- Haltech (via NSP CSV export)
- ECUMaster EMU Pro (via CSV export)
- RomRaider (Subaru ECUs)
- Speeduino (MLG binary)
- rusEFI (MLG binary)
- AiM (XRK/DRK binary)
- Link ECU (LLG binary)
- Emerald K6/M3D (.lg1/.lg2 binary)
- MegaSquirt MS1/MS2/MS3 (TunerStudio CSV export)
- MHD Tuning (CSV export — BMW N54/N55/S55/B58)
- TunerStudio MSL (`.msl` legacy ASCII export — Speeduino/MegaSquirt via TunerStudio or RealDash)
- Motorsport Electronics ME221/ME442 (ME Tuner CSV export)
- RaceChrono / RaceChrono Pro (CSV v3 session export — lap-timing app)
- Woolich Racing Tuned (WRT CSV export — motorcycle ECUs)
- BlueDriver (OBD-II scan tool CSV export)
- DynamicEFI EBL WhatsUp (modified GM TBI CSV export)
- Torque Pro (Android OBD-II app trackLog CSV)
- Locomotive datalogger (CSV export)

To add a new ECU format:

1. Create a new module in `src/parsers/` (e.g., `newformat.rs`)
2. Define format-specific channel types and metadata structs
3. Implement the `Parseable` trait
4. Add enum variants to `Channel`, `Meta`, `EcuType`, and wire up in `mod.rs`
5. Update detection logic in `app.rs` file loading

**Text dispatch preprocessing contract** (`UltraLogApp::parse_text_content` in `src/app.rs`):

Before the text detection chain runs, the content is normalized twice, so individual parsers do not each have to handle it:

1. A leading **UTF-8 BOM** is stripped (`trim_start_matches('\u{FEFF}')`). Windows and mobile exporters commonly prepend one.
2. A leading block of **`#`-prefixed comment lines** is dropped via `parsers::strip_leading_comment_lines`, so the real CSV header row is the first line detection sees. MHD Tuning and OBD-II apps such as OBDLink write a metadata preamble there; without this the header is never found and the log loads with zero channels (issue #78). Only the *leading* contiguous block is removed — a `#` after the header row is data.

**Load-bearing invariant:** a parser whose own fingerprint lives inside that comment preamble must run its `detect()` against the **raw** text, before the strip. `Mhd::detect` is the current case, which is why it is dispatched ahead of `dispatch_text_content` rather than inside the chain. Anything added to the chain itself sees comment-stripped content.

**RomRaider time-column unit contract** (`src/parsers/romraider.rs`):

`RomRaider::detect` accepts any CSV whose first column starts with `time`, which makes it the
effective catch-all for generic OBD-II exports, not just Subaru logs. The unit is therefore not
implied by the format — it must be read from the header:

- `Time (msec)` / `Time(ms)` / `Time (milliseconds)` -> milliseconds
- `Time (sec)` / `Time (s)` / `Time (seconds)` -> seconds
- a bare `Time` with no parenthesised unit -> **milliseconds**, RomRaider's own convention

Milliseconds must be tested before seconds, since `sec` is a substring of `msec`. Hardcoding the
`/1000.0` is what made a 2049-second OBDLink log render as 2.05 seconds (issue #80). Note this
interacts with the dispatch order above: `MotorsportElectronics::detect` runs first precisely
because it also leads with a `Time` column, and only matches a first column of exactly `Time`.

**Motorsport Electronics detection contract** (`src/parsers/motorsport_electronics.rs`):

ME Tuner's channel list is whatever the user picks, so the column names cannot be the
fingerprint. A short ME442 log with a custom 8-channel list has none of the ME221
columns (`Sync status`, `MAP Raw`, ...), failed detection, and fell through to the RomRaider
catch-all, which read its bare `Time` as milliseconds: 1,878 s showed as 1.878 s (issue #95).
`MotorsportElectronics::detect` therefore accepts `Time` first plus `Marker` second (ME Tuner
writes the marker button state there in every export), and keeps the two-ME221-column rule
only as a fallback. Unit inference returns no unit for any name containing the word `raw` or
`status`: those are ADC counts and state enums, and a `kPa` or `°C` label would make the unit
preferences convert them.

**RaceChrono parser load-bearing behaviors** (`src/parsers/racechrono.rs`):

- **Time base is the unix `timestamp` column, never `elapsed_time`** - `elapsed_time` resets to zero at every fragment boundary (a fragment is one recording segment; sessions paused/resumed at the track have several), so re-basing `timestamp` to the first record is the only way to get a monotonic time axis. Monotonic times are required because computed-channel time-shift lookups binary-search the `times` vector.
- **Duplicate column names are disambiguated by source, unique names stay raw** - RaceChrono exports the same column name once per source (`speed` from GPS and calc, `device_update_rate` from every sensor). Duplicates get the sources-row label appended (`speed (gps)`); unique names — including `latitude`/`longitude` — must stay exactly as exported because `find_gps_channels` in `src/ui/widgets/track_map.rs` matches those names *exactly* (lowercased), and a suffix would silently disable the GPS Track Map for RaceChrono logs. When the short labels themselves collide (two location devices both labeled `gps`), the first occurrence keeps its bare name — so `latitude` still survives — and later ones carry the full source tag (`latitude (101: gps)`).
- **Detection accepts any `Format,N` version; `parse` rejects non-v3** - so a v1/v2 export surfaces a clear "re-export as CSV v3" error instead of falling through to the Haltech default parser and failing cryptically.

**Torque Pro parser load-bearing behaviors** (`src/parsers/torque.rs`):

- **Detection is the `GPS Time, Device Time` header pair, searched in the first 64 lines** - the
  first column is not `Time`, so without `Torque::detect` (dispatched right after RaceChrono) the
  file falls through to the Haltech default and fails. A file cut out of a longer log still
  detects when a restart header follows within 64 lines; the rows above it are skipped because
  they cannot be mapped to channels.
- **Time base is `Device Time`, never `GPS Time`** - `Device Time` (`05-Jan-2013 14:32:13.526`)
  has milliseconds; `GPS Time` is a Java `Date.toString()` with 1 s resolution and can lag the
  device clock by minutes. Epoch milliseconds stay `i64` until the first row is subtracted, so
  steps are exact. The source is chosen from the first 50 data rows (`choose_time_source`), so
  one corrupt first row cannot put the file on the 1 s GPS clock: full datetime if any row has
  one, else the clock in the last token (Excel re-saves the column as `mm:ss.0`, which wraps each
  hour; a non-English month name also lands here), else the `GPS Time` clock.
- **`TimeAxis::place` keeps `times` sorted without dropping data** - `Device Time` is local
  time, so a time-zone crossing or the DST fall-back steps it back. A clock step back of more
  than half its period is a wrap (one period added); any other step back over 1 s is a clock
  change that the offset absorbs, so the axis continues from the last row (same contract as
  MSL); 1 s or less is jitter and that row is dropped. Do not go back to "drop every row that
  steps backwards": that lost up to an hour of data after a DST change.
- **Restart gaps are lost in the clock-only modes** - an `mm:ss` or `HH:MM:SS` clock carries no
  date, so a restart hours or a day later shows a gap of only the clock difference. The data
  cannot recover it; do not try to "fix" it.
- **A repeated header row starts a new section** - Torque appends a restarted session to the same
  file and writes the header again, and the PID set can change. Channels are the union of all
  sections by raw header text; a channel missing from a section carries its last value (and one
  first seen in a later section reads 0.0 for the earlier ones). A header that names one column
  twice keys the second occurrence separately, so its `-` cannot overwrite the first reading.
- **Rows are stored sparse** - `(channel, reading)` pairs only, because with "log all PIDs" about
  90 % of fields are `-`; a dense `Vec<Option<f64>>` per row cost ~320 MB at 100k rows.
- **`-` means no reading; empty columns are dropped** - Torque polls PIDs round-robin and writes
  `-` until one answers, so readings carry forward (0.0 before the first). With "log all PIDs"
  the header lists ~220 PIDs, most of which the car never answers; a column with no numeric
  value anywhere is dropped (`TorqueMeta::empty_columns_dropped`).
- **Names: fixed columns from a table, PIDs split on the last parenthesis group** - PID headers
  are `Name(unit)`, and only the last group is the unit (`Air Fuel Ratio(Commanded)(:1)`). The
  fixed `G(x)` / `G(y)` / `G(z)` columns would all become `G` under that rule, so the leading
  GPS/G columns come from `FIXED_COLUMNS`. `Latitude` / `Longitude` must keep those exact names
  for `find_gps_channels` in `src/ui/widgets/track_map.rs`. Display names that still collide
  get the unit, then a counter (`unique_name`). `canonical_unit` maps `l/100km`, `l`, `:1`
  (AFR) and `lambda` to the spellings `UnitPreferences` converts.
- **A header-only file is an error, not an empty log** - the issue #97 reporter's first file had
  no data rows; the error says Torque recorded no samples.
- **No built-in normalization for Torque PID names** - `Engine RPM` and `Timing Advance` are
  already source or display names in the OECUA specs (Haltech, RomRaider, ...), and the built-in
  map in `normalize.rs` runs before the spec map, so adding them there would rename those
  parsers' channels too.

**TunerStudio MSL parser load-bearing behaviors** (`src/parsers/msl.rs`):

- **Detection requires the header/units row *pair*** - `.msl` is tab-delimited with `Time` as
  the first column, which several other formats also are. What makes MSL unambiguous is the
  second row: it holds a unit per column and names a time unit (`sec`, `s`, `ms`, ...) under
  `Time`. Dropping that requirement would make the parser claim tab-delimited ECUMaster and
  generic `Time`-first exports. `Msl::detect` runs after `MegaSquirt::detect` and before
  `EcuMaster::detect` in the `dispatch_text_content` chain.
- **Time scale comes from the units row, never assumed** - `sec`/`s`/`seconds` -> seconds,
  `ms`/`msec`/`milliseconds` -> milliseconds. Same class of bug as issue #80's RomRaider
  time-unit hardcode.
- **Non-numeric and blank fields carry the last known value forward** - MSL columns are not
  all numeric: RealDash writes `GPS Date` as `18.8.2026` and leaves fields blank until the GPS
  gets a fix. Substitution preserves column alignment, the same approach as
  `parsers/haltech.rs` and `parsers/ecumaster.rs`. Clock-style fields (`HH:MM`, `HH:MM:SS`)
  are the exception: they convert to seconds since midnight so `GPS Time` plots as a channel.
- **Times are forced monotonic** - a `.msl` can concatenate sessions, so a backwards jump in
  `Time` accumulates an offset rather than being written through. Monotonic times are required
  because computed-channel time-shift lookups binary-search the `times` vector.
- **GPS column names stay exactly as exported** - `GPS Latitude` / `GPS Longitude` are matched
  by name (lowercased) in `find_gps_channels` (`src/ui/widgets/track_map.rs`); renaming or
  suffixing them silently disables the Track Map for MSL logs.

**Speeduino/rusEFI MLG field-count contract** (`src/parsers/speeduino.rs`):

- **No fixed channel cap** - the field count is bounded by the file itself: the descriptor table
  (`offset + num_fields * field_length`) must end at or before `data_begin_index`, which must be
  inside the file. A hard limit of 1000 rejected every log from current rusEFI firmware, which
  writes 1096 fields (issue #100). In real rusEFI v2 logs the table ends exactly at
  `info_data_start`. Do not add a numeric cap back; the file-size bound already stops a crafted
  header from allocating more than the file can describe.

**Emerald parser load-bearing behaviors** (`src/parsers/emerald.rs`):

- **Channel IDs come from EM Soft's variable list, and most are unconfirmed** - the `[chanN]`
  value in the `.lg2` is an ID into EM Soft's logger variable list, which Emerald does not
  document. Only IDs 1, 15, 20, 24, 26, 31, 32 and 41 are confirmed (issue #93, reporter's EM
  Soft labels). The rest are original guesses. ID 1 is the K6 lambda input in mV (not TPS),
  41 is TPS, 31 is the 0-15 load site (not injector duty). Confirm an ID against an EM Soft
  screenshot before renaming it. A rename must also go into the `source_names` of
  `adapters/emerald/emerald-lg.adapter.yaml` in the ClassicMiniDIY/OECUASpecs repo (fetched
  by `build.rs` into the gitignored `spec/OECUASpecs/`); spec normalization matches on those
  names. Its `internal_id` field documents the IDs but UltraLog does not read it.
- **MAP scaling depends on `[ValU]`** - the second `[ValU]` value is the logger's pressure unit:
  `1` stores MAP in whole kPa, `2` in mbar. Hardcoding either one makes the other log read 10x
  wrong. Unknown codes keep the mbar scaling.
- **ID 1 is named "Lambda Sensor Voltage" on purpose** - it is a 0-1 V narrowband input where
  a higher reading is richer. The `"volt"` veto on `ChannelRole::Lambda` in
  `channel_map.rs` depends on that word, so the table generators never map it as lambda.
- **Repeated names get the channel ID appended** - IDs 21/24 and 45/46 share a name, and name
  lookups return the first match. `disambiguate_names` keeps the plain name on a
  `CONFIRMED_IDS` entry (else the first by slot) and appends ` (ID n)` to the others, so a
  formula bound to `Ignition Advance` does not change meaning with slot order.
- **The value column is the slot number, not the list position** - `[chanN]` reads bytes
  `8 + (N-1)*2` of the record. `parse_lg2` drops slots outside 1-8 and repeated slots, so an
  unreadable or missing `[chanN]` cannot shift later channels onto the wrong data.
- **Ign Adv (ID 24) offset is inferred** - `raw * 0.1 - 50` gives 5.5° at idle and 14-17° at
  230 kPa boost. It is not from Emerald documentation.

**Haltech parser load-bearing behaviors** (`src/parsers/haltech.rs`, added for wall-clock-timestamped exports):

- **Last-known-value substitution** - Unparseable/blank fields are filled with the last successfully parsed value for that column (`0.0` before the first valid sample), matching the approach used in `parsers/ecumaster.rs`. This preserves column alignment instead of shifting subsequent columns when a field fails to parse.
- **"No reading" sentinels are filled before scaling** - NSP writes an i32 sentinel family (`-2147483617`, `-2147483637`, ...) when a channel has no reading. The parser checks the *raw* value with `analysis::tables::events::is_invalid_sample` and treats a hit like a blank field (last known value, `0.0` before the first). It must run before `ChannelType::convert_value`: once scaled (lambda / 1000, pressure / 10) the sentinel drops below `SENTINEL_MAGNITUDE`, no longer matches, and showed up as a -31,568,009 AFR minimum on the channel card and a flattened chart.
- **Midnight rollover handling** - Timestamps are wall-clock time-of-day, so a logging session that crosses midnight wraps from ~86400 back toward 0. The parser tracks the previous raw timestamp and accumulates a 24h (`86_400.0`) offset on each backwards jump greater than 1 second (the 1s guard avoids tripping on timestamp jitter), keeping `times` monotonically increasing. This is the same technique used in `parsers/woolich.rs` (`Log Time` column) — monotonic times are required because computed-channel time-shift lookups binary-search the `times` vector.

### Data Flow

**Startup and Spec Loading:**

1. **App initialization** - `registry.rs` loads adapter/protocol specs via fallback chain:
   - If cache is fresh (< 24 hours old) → load from disk cache
   - Else → load embedded YAML specs from `spec/OECUASpecs/`
   - Background thread spawned to fetch latest specs from OpenECU Alliance API
2. **Background refresh** - Non-blocking API fetch updates cache and registry for next startup
3. **Normalization maps built** - Extract `source_names` from adapter specs to build field name mappings

**File Loading and Visualization:**

1. Files are loaded asynchronously via `start_loading_file()` → background thread
2. Parser converts file (CSV or binary) to `Log` struct with channels, times, and data vectors
3. Field normalization optionally applied using spec-driven or custom mappings to standardize channel names
4. User selects channels (raw or computed) → added to `selected_channels` with unique color assignment
5. Computed channels evaluate formulas across all records with time-shifting support
6. Chart renders downsampled data from cache, limited to 2000 points per channel using LTTB algorithm
7. Unit conversions applied at display time based on `unit_preferences`

**Adapter Spec Fallback Chain:**

```text
1. Startup (fast, non-blocking):
   Cache (< 24h old) → Embedded YAML → Background API fetch

2. Background API fetch (async):
   OpenECU Alliance API → Update cache → Rebuild normalization maps → Ready for next startup
```

### Settings Persistence Contract

`UserSettings` (`src/settings.rs`) is a `Serialize`/`Deserialize` struct persisted as JSON at `{app_data_dir}/UltraLog/settings.json` (or `.../ultralog/settings.json` on Linux). It currently covers: `language`, `scroll_to_zoom`, `show_grid`, `grid_opacity`, `unit_preferences`, `font_scale`, `color_blind_mode`, `ultra_intensity`, `high_contrast`, `field_normalization`, `cursor_tracking`, `auto_check_updates`, `custom_normalizations`, and the Track Map preferences (`tile_provider`, `tile_cache_max_mb`, `tiles_enabled`, `tile_opacity`, `tile_grayscale`, `tile_privacy_notice_seen`, `hidden_widgets`), and `export` (the ExportDialog's last format and theme per format).

- **Load** - `UltraLogApp::new` calls `UserSettings::load()` and copies each field into the corresponding live `UltraLogApp` field (e.g. `app.color_blind_mode = user_settings.color_blind_mode`).
- **Save** - `UltraLogApp` implements `eframe::App::save`, which eframe calls on its auto-save interval (~30s) and again at shutdown. `save()` rebuilds a `UserSettings` from the current live fields, and only writes to disk if it differs from the last-loaded/saved value.
- **Defaults for new fields must match the in-app default, not the type default.** `serde(default)` on a `u8` gives 0; `ultra_intensity` uses `default_ultra_intensity` (35) so a 2.x settings file opens at Night Drive, not Stealth (`test_settings_from_before_3_0_open_at_night_drive`).
- **Load-bearing invariant:** because the sync is manual field-by-field (not `#[derive]` magic), **any new persisted preference must be added in three places**: the field on the `UserSettings` struct, the copy in `UltraLogApp::new`, and the reconstruction in `eframe::App::save` in `src/app.rs`. Missing any one of the three means the preference silently resets on restart or never saves at all.

### Chart/Scatter Cache Invalidation Contract

Several `UltraLogApp` fields cache expensive per-file, per-channel computations across frames:

- **`downsample_cache`** (`(file_index, channel_index) -> (DownsampleViewKey, Vec<[f64; 2]>)`) - LTTB-downsampled chart points, tagged with the view key (zoom/bucket state) they were computed for. Read/written in `src/ui/chart.rs`.
- **`minmax_cache`** (`CacheKey -> (f64, f64)`) - Per-channel min/max for the legend overlay.
- **`scatter_histogram_cache`** (keyed by `(file, x_channel, y_channel)`) - Precomputed 2D histogram buckets for the scatter/heatmap view (`src/ui/scatter_plot.rs`).

**Load-bearing invariant:** these caches are keyed by index (file index / channel index), not by identity. Any code path that mutates or reindexes channel data — removing a file, or removing/editing a computed channel (which shifts later computed channels' indices down) — **must clear the relevant caches**, or stale entries will silently render the wrong data against a different channel's index. See `remove_computed_channel` and the file-removal path in `src/app.rs` for the current call sites (`self.downsample_cache.clear()`, `self.minmax_cache.clear()`, `self.scatter_histogram_cache.clear()`).

A cache that holds derived **colours** has a second key: `UltraLogApp::theme_generation`, which
increments on every theme install. The Track Map's coloured-polyline cache stores it
(`theme_generation` in `state.rs`) and rebuilds when it differs, so a change of intensity, Color
blind mode or High contrast never leaves the old palette on screen.

### Map Tile Fetching Contract (src/tiles.rs)

The Track Map widget can draw map tile backgrounds. Tiles are **opt-in** (off by default) and only two hardcoded HTTPS providers exist: Esri World Imagery and OpenStreetMap. Requests carry integer tile coordinates plus the UltraLog User-Agent, nothing else. The first time a user enables tiles, a one-time toast (`tile_privacy_notice_seen`) notes that the track's approximate location is shared with the provider.

**Load-bearing invariant:** network fetches are gated by per-provider permits (`FetchPermits` + `TileProvider::max_concurrent_fetches`). The OSM tile usage policy allows at most **2 simultaneous download connections** (<https://operations.osmfoundation.org/policies/tiles/>), so `OpenStreetMap::max_concurrent_fetches` returns 2. Do not raise it and do not bypass the permit gate — exceeding the policy risks a per-IP block that would hit every UltraLog user at once.

### Theme Contract (src/ui/theme/)

UI work follows the Night Drive design system in `design/` (brand book
`design/design-system/README.md`, brief `design/HANDOFF-APP.md`, implementation plan
`docs/plans/2026-10-23-night-drive-design-system.md`). It shipped in 3.0.0; every screen reads
the tokens, and the CI colour gate keeps it that way.

- **Never hard-code a colour, size or radius.** Read `UltraLogApp::tokens` (or
  `theme::current(ctx)` where there is no app reference, e.g. `DataWidget`s) and the
  `tokens::{space, radius, stroke, text}` modules.
- **`tokens.rs` is generated.** Edit `design/tokens/tokens.json`, then run
  `cargo run --example gen_theme`. The test `committed_tokens_match_json` fails when the two
  drift. The module is `#[rustfmt::skip]` so the generator's output is what gets committed.
  Resolution rules (night fallback, `{ref}` in-theme, effects from the shadow tokens) are
  documented at the top of `codegen.rs`; they match `design/tokens/tokens.ts`.
- **One install path.** `UltraLogApp::sync_theme` runs every frame from `logic()`, derives
  `ThemeSettings` from the live preference fields and reinstalls only on a change, so a setting
  toggled anywhere applies next frame without its own hook. `theme::install` pins
  `ThemePreference::Dark` (otherwise egui follows the OS appearance and swaps in its light
  style) and stores the tokens in `ctx.data`.
- **Data colours ignore intensity.** Series, colour maps, status and cursor follow Color blind
  mode only (tested by `data_colors_ignore_intensity`). The exception is by design: the diverging
  midpoint `div_2` is a ground-tinted neutral outside Color blind mode.
- **Series beyond the palette** change stroke, not colour: `theme::series` returns
  `SeriesStroke::Dashed` for the second lap and `Dotted` after (laps of 10, or of 8 when
  `color_blind_like` — Color blind mode, High contrast and exports).
- **Colour maps:** never add a rainbow (jet, Turbo) or a hand-rolled ramp; everything that colours by magnitude goes through `colormap::sample` / `theme::seq` / `theme::div`, cells through `ui::heatmap` (value plate, hatched empty cells, badge fill styles).
- **Caches of derived colours** must key on `UltraLogApp::theme_generation`, which increments on
  every install, or they keep the old palette after a theme change.
- **Fonts** (`src/ui/fonts.rs`): egui families have no weight axis, so each weight is its own
  family (`ui-medium`, `ui-semibold`, `display`, `display-bold-italic`, `display-black-italic`,
  `mono-medium`; Proportional = Outfit 400, Monospace = JetBrains Mono 400), built from static
  instances in `assets/fonts/` (`scripts/fonts/build_static_fonts.sh`; egui renders only a
  variable font's default instance). Every family falls back to egui's defaults (Greek,
  Cyrillic, emoji — Outfit has none of these), bundled Noto Devanagari/Bengali/Arabic, a 3 KB
  Noto SC subset holding only the language picker's 简体中文日本語, and for `ja`/`zh-CN` only a
  system CJK `.ttc` read at runtime (`cjk_candidates`; bundling CJK would add ~10 MB).
  `UltraLogApp::sync_fonts` reinstalls on a language change. The type scale lives in
  `Style::text_styles` (installed with the visuals, scaled by `FontScale`); use `TextStyle`s and
  `fonts::style::*` names rather than `FontId::proportional(scaled_font(n))`.
  Arabic/Urdu still lack right-to-left layout (an egui limitation).
- **Components** (`src/ui/components/`) read `theme::current(ui.ctx())`, so any `Ui` can call
  them. New floating windows go through `components::window(..)` and must call
  `components::finish_window` after `.show(..)` (redline, Esc closes the topmost window). Use
  `components::button(.., Kind::..)` instead of hand-built `Frame` buttons; one primary per view.
  Status is always icon + word (`components::Status`), never colour alone.
- **Charts** draw series through `plot_style::draw_series` and the legend through
  `plot_style::legend_overlay` (egui_plot's built-in `Legend` is off: it cannot show the Cursor row,
  live mono values or the solo fade). The soloed series is kept per plot in egui temp memory
  (`plot_style::solo_memory`). Colour slots are unbounded (`state::next_color_index`); past the
  palette `theme::series` repeats colours dashed, then dotted.
- **Custom painting** stays in `src/ui/theme/paint.rs`: redline, glow, floor, dashed outline and
  the one keyboard focus ring (`keyboard_focus_ring`, called once per frame at the end of
  `ui()`; it shows after Tab/arrow navigation or on a text field, not after a mouse click).
  Chart-specific painting (series strokes and Ultra bloom, the legend overlay, direct labels)
  lives in `src/ui/plot_style.rs`.
- **Icons are Phosphor** (`components::icon::*`, regular outline). The `phosphor` font sits right
  after each brand face in every family, so an icon codepoint works in any label; the filled
  set (`fonts::PHOSPHOR_FILL`) is only for transport glyphs and the favourite star. No emoji.
- **Brand assets are generated, not edited.** `assets/icons/*` come from
  `design/logos/ultralog-app-icon-1024.png` via `scripts/brand/build_app_icons.sh` (macOS keeps
  Apple's 824 px tile in the 1024 canvas; the paths are fixed because `main.rs`, `build.rs` and
  `release.yml` read them). `assets/brand/*` lockups come from `scripts/brand/rasterize_lockups.sh`.
- **Colour gate (CI):** `grep -rnE 'Color32::|Rgba\(|Rgb::new|\[u8; 3\]' src/` may match only
  `src/ui/theme/`, `Color32::TRANSPARENT`, the PDF/PNG type conversion in
  `src/ui/export_sheet/{pdf,png}.rs` and a byte triple in `parsers/link.rs`. Image tints go
  through `theme::image_tint`, translucent export colours through `theme::flatten`.
- **CI** runs `python3 design/tools/check_contrast.py design/tokens/tokens.json`. A token change
  that breaks a pair fails the build: fix the token, never relax the check.

### Export Contract (src/ui/export.rs, src/ui/export_sheet/)

Every PNG and PDF export is one ExportSheet (`design/design-system/4-exports.md`): sunset
redline, lockup, title, mono meta (time range, counts), the chart or grids, a legend or
ColorScale, and the footer (ultralog.co · version · export date).

- **One path.** `UltraLogApp::export_sheet` builds a `Sheet` for the active tool (chart, scatter,
  histogram, table via `table_export::table_sheet`); `export_to_file` and `copy_export_image` are
  the only writers. Menu, Tools panel and Cmd+E open the ExportDialog; do not add a direct
  per-tool export again.
- **Models, not app state, are drawn.** `draw_sheet` lays out `ChartModel` / `GridModel` once on a
  `Canvas`; `PngCanvas` (vello_cpu, 960 × 540 pt at 2×) and `PdfCanvas` (printpdf, A4 landscape)
  implement it. A layout change is made once and both formats follow.
- **Exports ignore app settings.** Colours come from `export_tokens(ExportTheme)` only, the series
  palette is always the colour-blind one (`theme::series(.., true)`), and there are no intensity
  effects. PDF has no cheap alpha, so translucent tokens go through `canvas::flatten`.
- **PDF text is embedded TTF, PNG text is outlines.** `PdfCanvas` embeds the bundled faces
  (`fonts::face_bytes`) so text stays selectable and `−`, `λ`, `°`, Cyrillic and Devanagari print;
  never go back to built-in Helvetica (its encoding mangles anything past Latin-1). PNG text is
  skrifa glyph outlines filled by vello_cpu. `text::layout` has no shaping or kerning and falls
  back per character Outfit/Saira → JetBrains Mono → Noto scripts → a system CJK face
  (`fonts::export_cjk_face`, any UI language), so Arabic prints unjoined. A character no face has
  prints as `?`, never `.notdef`: printpdf's `.notdef` width differs from skrifa's and shifted the
  text after it.
- **Exported scales match the screen.** The table export picks the diverging map for values on
  both sides of zero exactly as `render_table_tool_view` does, and only lambda-deviation measures
  (`MeasureSpec::lean_is_positive`) get the rich · target · lean labels; PW step does not.
- **The lockup is a pre-rasterized PNG** (`assets/brand/ultralog-lockup{,-paper}.png`,
  `scripts/brand/rasterize_lockups.sh`, rsvg-convert): there is no SVG renderer at runtime. PNG
  composites it after the vector render; PDF places it as an image XObject.
- **The export matches the screen.** The chart exports the visible range
  (`chart_last_x_bounds[0]`) normalized per channel like the chart; the histogram applies the
  same custom ranges, sample filters, min-hits blanking and `binning::uniform_bin` as
  `render_histogram_grid`. Change one and change the other.
- **Remembered choices** live in `UserSettings::export` (`ExportChoice`: last format, last theme
  per format), synced in the usual three places.
- **Visual check:** `EXPORT_SHEET_DUMP=<dir> cargo test --lib dump_ -- --ignored` writes sample
  chart, scatter and table sheets in both themes.

## Key Features

- **Multi-ECU Support** - Haltech, ECUMaster, RomRaider, Speeduino, rusEFI, AiM, Link, Emerald, MegaSquirt, TunerStudio MSL, MHD Tuning, Motorsport Electronics, RaceChrono, Torque Pro, Woolich Racing Tuned, BlueDriver, DynamicEFI, and Locomotive log formats
- **Computed Channels** - Create virtual channels from mathematical formulas with time-shifting (e.g., `RPM[-1]`, `Boost@-0.5s`)
- **Analysis Algorithms** - AFR/Lambda drift and zone detection, derived metrics (VE, injector duty cycle), signal filters, and descriptive statistics (`src/analysis/`)
- **Table Generators** - Lambda delay and acceleration enrichment tuning tables mined from one or more logs, each a top-level tool beside Histogram, with auto-suggested channel roles, confidence-tiered cells, an event inspector, and CSV/clipboard/PNG/PDF export (`src/analysis/tables/`, `src/ui/table_generator.rs`, `src/ui/table_export.rs`)
- **GPS Track Map** - Right-side data panel with a track map: lap detection, channel-colored polyline (theme Sequential / Diverging map with editable range, 3px with a bg_void casing), hover-scrub/click-seek cursor sync, and opt-in Esri/OSM tile backgrounds (`src/ui/widgets/track_map.rs`, `src/tiles.rs`, `src/laps.rs`). GPS coordinate encodings are auto-detected and normalized to decimal degrees (`GpsCoordSpec` in `src/laps.rs`): NMEA `DDMM.mmmm`, milli/micro/1e-7-scaled integer degrees, and 0-360 longitude. Detection is conservative - values already in valid degree ranges are never transformed, and radians are deliberately not detected (ambiguous with genuine near-equator degree tracks).
- **Claude Desktop / MCP Integration** - Embedded MCP server (`src/mcp/`) lets Claude control the running app over `http://localhost:52385/mcp` — select channels, add computed channels, query log data
- **Unit Preferences** - Users can select display units for temperature, pressure, speed, distance, fuel economy, volume, flow rate, and acceleration
- **Field Normalization** - Maps ECU-specific channel names to standardized names for cross-ECU comparison
- **Scatter Plot Tool** - XY scatter plot for analyzing channel correlations
- **Histogram Tool** - 2D heatmap view of channel distributions
- **Export Options** - Export the chart, scatter plots, histogram or a tuning table as PNG or PDF on the Paper or Graphite sheet, or copy it as an image for pasting
- **Night Drive design system** - one dark theme with an Ultra intensity slider (Stealth · Night Drive · Ultra), Color blind mode (Okabe-Ito series, cividis and blue/orange maps) and High contrast; tokens generated from `design/tokens/tokens.json` (see the Theme Contract)
- **About window** - Help → About UltraLog: version, licence, links, Check for Updates
- **Playback** - Play through log data at 0.25x to 8x speed
- **Cursor Tracking** - Lock view to follow cursor during playback/scrubbing
- **Min/Max Legend** - Shows peak values for each channel
- **Initial Zoom** - Charts start zoomed to first 60 seconds for better initial view
- **Multi-File Tabs** - Chrome-style tabs for working with multiple log files simultaneously
- **Internationalization** - `rust-i18n`-backed language selection (`src/i18n.rs`)
- **Settings Persistence** - Unit/display/theme/normalization/export preferences survive restarts (see [Settings Persistence Contract](#settings-persistence-contract))
- **Auto-Update** - Automatic update checking and installation

### Keyboard Shortcuts

Handled in `UltraLogApp::handle_keyboard_shortcuts` (`src/app.rs`); ignored while a text field has focus.

- **Cmd/Ctrl+O** - Open file
- **Cmd/Ctrl+W** - Close current tab
- **Cmd/Ctrl+,** - Open Settings panel
- **Cmd/Ctrl+1..5** - Switch tool, in `ActiveTool::ALL` order (Log Viewer / Scatter Plot / Histogram / Lambda Delay / Accel Enrichment)
- **Cmd/Ctrl+Shift+F/C/T** - Switch side panel (Files / Tool Properties / Tools)
- **Arrow Left/Right** - Step cursor one record (Shift = 10 records)
- **Home/End** - Jump cursor to start/end of log
- **Escape** - Stop playback
- **Cmd/Ctrl+E** - Open the ExportDialog for the current view
- **Space** - Toggle play/pause

## Key Dependencies

- **eframe/egui** (0.36) - Native GUI framework
- **egui_plot** (0.37) - Charting/plotting
- **rfd** (0.17) - Native file dialogs
- **open** (5) - Cross-platform URL/email opening
- **strum** (0.28) - Enum string conversion for channel types
- **regex** (1.13) - Log file parsing
- **ureq** (3.4) - HTTP client for auto-updates and OpenECU Alliance API
  - **TLS trusts `webpki-roots`, not the OS store** - ureq's rustls backend uses the root list
    compiled into the binary. When a CA is distrusted (eMudhra, 2026-10: `webpki-roots` 1.0.10
    dropped `emSign ECC Root CA - G3`), OS updates do not reach UltraLog users. Bump
    `webpki-roots`/`rustls-webpki` and ship a tagged release.
- **semver** (1.0) - Version comparison
- **serde_norway** (0.9) - YAML parsing for adapter/protocol specs. Replaced `serde_yml`, which was
  flagged unsound (RUSTSEC-2025-0068) and then deprecated; `serde_norway` is the maintained
  `serde_yaml` fork, and unlike `serde_yaml_ng` its libyaml backend is maintained too.
  Do not go back to `serde_yml` or `serde_yaml`.
- **printpdf** (0.12) - PDF generation
- **image** (0.25) - PNG encoding
- **skrifa** (0.44) / **vello_cpu** (0.1) - PNG export text and shapes: skrifa reads glyph outlines from the bundled fonts, vello_cpu fills them as paths (its glyph-atlas `text` feature is off)
- **memmap2** (0.9) - Memory-mapped file loading for large files
- **rayon** (1.12) - Parallel iteration for parsing
- **dirs** (6.0) - Cross-platform app data directory detection for cache and settings
- **rmcp** (3.1) - MCP server implementation (`src/mcp/`), used with `axum` as the HTTP transport.
  3.x fixes a DNS-rebinding hole in the streamable-HTTP transport; its `StreamableHttpService`
  config defaults `allowed_hosts` to localhost/127.0.0.1/::1, so keep passing `Default::default()`
  rather than clearing it.
- **tokio** (1) - Async runtime backing the MCP server
- **axum** (0.8) - HTTP server framework for the embedded MCP endpoint
- **schemars** (1.0) - JSON schema generation for MCP tool definitions
- **arboard** (3.6) - Clipboard support (histogram copy/paste)
- **rust-i18n** (4.2) - Internationalization / locale strings
- **uuid** (1.0) - Anonymous user ID generation for analytics
- **chrono** (0.4) - Date/time parsing (Locomotive, Woolich timestamps)
- **encoding_rs** (0.8) - Character encoding detection/conversion (e.g., UTF-16 BlueDriver exports)
- **thiserror** / **anyhow** - Error handling
- **tracing** / **tracing-subscriber** - Structured logging

Computed-channel formula evaluation is handled by the built-in `src/expression/engine.rs` (no external crate — do not re-add meval or an expression-evaluator dependency; meval was removed because it was unmaintained and its `nom` 1.2.4 transitive dep will be rejected by future Rust).

## Test Data

Example log files are in `exampleLogs/` organized by ECU type:

- `exampleLogs/haltech/` - Haltech NSP CSV exports
- `exampleLogs/aim/` - AiM XRK/DRK files
- `exampleLogs/link/` - Link ECU LLG files
- `exampleLogs/woolich/` - Woolich Racing Tuned CSV exports
- `exampleLogs/emerald/` - Emerald K6/M3D `.lg1`/`.lg2` files (`EM Log MG ZS Turbo boost run` is the issue #93 log, `[ValU]` pressure unit kPa; the others use mbar)
- `exampleLogs/megasquirt/` - MegaSquirt TunerStudio CSV exports, plus a `_gps.mlg` fixture with synthetic GPS channels (generated by `examples/inject_fake_gps_mlg.rs`; the coordinates are a fake closed loop, not a real location)
- `exampleLogs/mhd/` - MHD Tuning CSV exports (VIN redacted)
- `exampleLogs/msl/` - TunerStudio MSL excerpt from issue #86 (RealDash logging a Speeduino; trimmed to 2000 data rows, keeping the leading no-GPS-fix rows where `GPS Date`/`GPS Time` are blank)
- `exampleLogs/motorsportElectronics/` - Motorsport Electronics ME Tuner CSV exports (two ME221 logs with the full channel list; `ME442-V2B-PNP_...csv` is the issue #95 log, an ME442 with an 8-channel custom list over 1,878 s)
- `exampleLogs/racechrono/` - RaceChrono CSV v3 session excerpt (user-contributed track session, trimmed; covers blank-heavy pit-lane rows, the genuine fragment 0->1 boundary where `elapsed_time` resets, and fully-populated on-track rows)
- `exampleLogs/bluedriver/` - BlueDriver OBD-II CSV exports
- `exampleLogs/obdlink/` - OBDLink (iOS) OBD-II CSV exports (parsed by the RomRaider chain)
- `exampleLogs/dynamicEFI/` - DynamicEFI EBL WhatsUp CSV exports
- `exampleLogs/torque/` - Torque Pro trackLogs: `trackLog_header_only_issue97.csv` is the issue #97 reporter's file (a header row and no data; it carries the full "log all PIDs" header). `trackLog_synthetic_two_sessions.csv` is synthetic rows on that header (generated, fake GPS loop) with a repeated header after a 30 s restart
- `exampleLogs/locomotive/` - Locomotive datalogger CSV exports
- `exampleLogs/rusefi/`, `exampleLogs/speeduino/` - MLG binary logs
- `exampleLogs/ecumaster/`, `exampleLogs/RomRaider/` - CSV exports
- Additional directories (`aem/`, `HondaTuningStudio/`) hold sample data for formats without an implemented parser yet — don't assume a directory's presence means the format is supported
