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
├── csv_export.rs      # CSV serialization of time-aligned channel columns
├── expression.rs      # Formula parsing and evaluation engine
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
│   └── statistics.rs   # Descriptive statistics and correlation analysis
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
│   ├── motorsport_electronics.rs   # Motorsport Electronics ME221/ME442 (ME Tuner) CSV parser
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
    ├── sidebar.rs                    # Legacy files panel, superseded by files_panel (kept, not wired into render path)
    ├── channels.rs                   # Selected-channel cards; legacy channel-picker superseded by tool_properties_panel
    ├── chart.rs                      # Chart rendering, legends, LTTB algorithm
    ├── timeline.rs                   # Timeline scrubber and playback controls
    ├── menu.rs                       # Menu bar (Units, Tools, Help menus)
    ├── toast.rs                      # Toast notification system
    ├── icons.rs                      # Custom icon drawing utilities
    ├── tab_bar.rs                    # Multi-file tab interface
    ├── tool_switcher.rs              # Switch between Log Viewer, Scatter Plot, and Histogram tools
    ├── scatter_plot.rs               # XY scatter plot visualization
    ├── histogram.rs                  # 2D histogram/heatmap view for channel distributions
    ├── export.rs                     # PNG, PDF, and CSV export functionality
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
  - Color palette constants (`CHART_COLORS`, `COLORBLIND_COLORS`)

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

- **`expression.rs`** - Formula evaluation engine:
  - Parses mathematical expressions using meval
  - Extracts channel references with time-shift syntax
  - Validates formulas against available channels
  - Evaluates formulas across all log records with proper time-shifting

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

### IPC + MCP System (src/ipc/, src/mcp/)

UltraLog embeds an MCP (Model Context Protocol) HTTP server so Claude Desktop can drive the running GUI — select channels, add computed channels, and query log data.

- **`mcp/server.rs`** - The MCP server itself (`rmcp` + `axum`), served over HTTP at `http://localhost:52385/mcp` (`DEFAULT_MCP_PORT`). This is what Claude Desktop connects to.
- **`mcp/client.rs`** - A TCP client the MCP server uses internally to talk to the GUI's IPC server. Each command opens a fresh TCP connection (no persistent/stale connections).
- **`ipc/server.rs`** - A TCP server (`DEFAULT_IPC_PORT` = 52384) that runs in a background thread inside the GUI process and receives commands from `mcp/client.rs`.
- **`ipc/commands.rs`** - The shared `IpcCommand`/`IpcResponse` wire types.
- **`ipc/handler.rs`** - `UltraLogApp` methods that execute each `IpcCommand` against live app state.

**Load-bearing contract:** the IPC server wakes the GUI via a repaint callback (`request_repaint()`), not a polling timer — see `IpcServer::start_with_repaint` in `src/ipc/server.rs` and its wiring in `UltraLogApp::new`. Incoming commands are drained in `UltraLogApp::process_ipc_commands`, which caps processing at **10 commands per frame** to avoid blocking the UI thread; if more are queued, it requests another repaint to continue next frame.

### UI Modules (src/ui/)

UI rendering is split into focused modules that implement methods on `UltraLogApp`. The current layout is a VS Code-style activity bar + side panel; a couple of pre-activity-bar modules remain in the tree but are superseded (noted below):

- **`activity_bar.rs`** - Vertical icon strip on the far left for switching between side-panel sections (Files / Tool Properties / Tools / Settings)
- **`side_panel.rs`** - Container that routes to the panel selected in the activity bar
- **`files_panel.rs`** - File management, loading, and file list (current files panel — see `sidebar.rs` note below)
- **`tools_panel.rs`** - Analysis tools, computed channels library, and export options, inline in the side panel
- **`settings_panel.rs`** - Consolidated settings: display, units, normalization, updates
- **`tool_properties_panel.rs`** - Dynamic panel showing controls for the active tool: channel selection for Log Viewer, axis/grid/filter controls for Histogram, controls for Scatter Plot
- **`analysis_panel.rs`** - Window for running `src/analysis` algorithms (filters, statistics, AFR, derived metrics) against the active log, with category tabs
- **`sidebar.rs`** - Legacy file list/drop-zone panel, superseded by `files_panel.rs` (module still compiles but its render methods are not called from `app.rs`)
- **`channels.rs`** - Selected-channel cards (`render_selected_channels`, still active) plus a legacy channel-picker list (`render_channel_selection`) superseded by `tool_properties_panel.rs`
- **`chart.rs`** - Main chart with egui_plot, min/max legend overlay, LTTB downsampling, normalization
- **`timeline.rs`** - Bottom panel: playback controls (play/pause/stop), speed selector, timeline scrubber
- **`menu.rs`** - Top menu bar (File, Edit, View, Help; Units submenu with 8 unit categories)
- **`toast.rs`** - Toast notification overlay for user feedback
- **`icons.rs`** - Custom icon drawing (upload icon for drop zone)
- **`tab_bar.rs`** - Chrome-style tabs for multi-file support
- **`tool_switcher.rs`** - Switch between Log Viewer, Scatter Plot, and Histogram tools
- **`scatter_plot.rs`** - XY scatter plot for channel correlation analysis
- **`histogram.rs`** - 2D histogram/heatmap view of channel distributions, with configurable cell coloring (average Z-value or hit count)
- **`export.rs`** - PNG and PDF export with chart rendering
- **`normalization_editor.rs`** - Dialog for creating custom field name mappings
- **`computed_channels_manager.rs`** - Window for managing computed channel library
- **`formula_editor.rs`** - Dialog for creating/editing computed channel formulas
- **`update_dialog.rs`** - Update notification and download progress dialog

### Parser System

The parser system uses a trait-based design for supporting multiple ECU formats:

- **`parsers/types.rs`** - Core types: `Log`, `Channel`, `Value`, `Meta`, `EcuType`, `ComputedChannelInfo`, and the `Parseable` trait
- **`parsers/haltech.rs`** - Haltech CSV parser (NSP exports)
- **`parsers/ecumaster.rs`** - ECUMaster EMU Pro CSV parser (semicolon/tab delimited)
- **`parsers/romraider.rs`** - RomRaider CSV parser with unit extraction from headers
- **`parsers/speeduino.rs`** - Speeduino/rusEFI MLG binary format parser
- **`parsers/aim.rs`** - AiM XRK/DRK binary format parser for motorsport data loggers
- **`parsers/link.rs`** - Link ECU LLG binary format parser
- **`parsers/woolich.rs`** - Woolich Racing Tuned CSV parser (HH:MM:SS.mmm timestamps, boolean channels)
- **`parsers/emerald.rs`** - Emerald K6/M3D ECU binary parser (`.lg2` channel definitions + `.lg1` 24-byte timestamped records)
- **`parsers/locomotive.rs`** - Locomotive datalogger CSV parser (detected via `TimeStamp:` / `Customer:` header lines, day-of-week-prefixed data rows)
- **`parsers/megasquirt.rs`** - MegaSquirt (MS1/MS2/MS3/MS3Pro) TunerStudio CSV parser
- **`parsers/motorsport_electronics.rs`** - Motorsport Electronics ME221/ME442 (ME Tuner) CSV parser
- **`parsers/bluedriver.rs`** - BlueDriver OBD-II scan tool CSV parser (UTF-16 with BOM)
- **`parsers/dynamicefi.rs`** - DynamicEFI EBL WhatsUp CSV parser (modified GM TBI systems)

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
- Motorsport Electronics ME221/ME442 (ME Tuner CSV export)
- Woolich Racing Tuned (WRT CSV export — motorcycle ECUs)
- BlueDriver (OBD-II scan tool CSV export)
- DynamicEFI EBL WhatsUp (modified GM TBI CSV export)
- Locomotive datalogger (CSV export)

To add a new ECU format:

1. Create a new module in `src/parsers/` (e.g., `newformat.rs`)
2. Define format-specific channel types and metadata structs
3. Implement the `Parseable` trait
4. Add enum variants to `Channel`, `Meta`, `EcuType`, and wire up in `mod.rs`
5. Update detection logic in `app.rs` file loading

**Haltech parser load-bearing behaviors** (`src/parsers/haltech.rs`, added for wall-clock-timestamped exports):

- **Last-known-value substitution** - Unparseable/blank fields are filled with the last successfully parsed value for that column (`0.0` before the first valid sample), matching the approach used in `parsers/ecumaster.rs`. This preserves column alignment instead of shifting subsequent columns when a field fails to parse.
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

`UserSettings` (`src/settings.rs`) is a `Serialize`/`Deserialize` struct persisted as JSON at `{app_data_dir}/UltraLog/settings.json` (or `.../ultralog/settings.json` on Linux). It currently covers: `language`, `scroll_to_zoom`, `show_grid`, `grid_opacity`, `unit_preferences`, `font_scale`, `color_blind_mode`, `field_normalization`, `cursor_tracking`, `auto_check_updates`, and `custom_normalizations`.

- **Load** - `UltraLogApp::new` calls `UserSettings::load()` and copies each field into the corresponding live `UltraLogApp` field (e.g. `app.color_blind_mode = user_settings.color_blind_mode`).
- **Save** - `UltraLogApp` implements `eframe::App::save`, which eframe calls on its auto-save interval (~30s) and again at shutdown. `save()` rebuilds a `UserSettings` from the current live fields, and only writes to disk if it differs from the last-loaded/saved value.
- **Load-bearing invariant:** because the sync is manual field-by-field (not `#[derive]` magic), **any new persisted preference must be added in three places**: the field on the `UserSettings` struct, the copy in `UltraLogApp::new`, and the reconstruction in `eframe::App::save` in `src/app.rs`. Missing any one of the three means the preference silently resets on restart or never saves at all.

### Chart/Scatter Cache Invalidation Contract

Several `UltraLogApp` fields cache expensive per-file, per-channel computations across frames:

- **`downsample_cache`** (`(file_index, channel_index) -> (DownsampleViewKey, Vec<[f64; 2]>)`) - LTTB-downsampled chart points, tagged with the view key (zoom/bucket state) they were computed for. Read/written in `src/ui/chart.rs`.
- **`minmax_cache`** (`CacheKey -> (f64, f64)`) - Per-channel min/max for the legend overlay.
- **`scatter_histogram_cache`** (keyed by `(file, x_channel, y_channel)`) - Precomputed 2D histogram buckets for the scatter/heatmap view (`src/ui/scatter_plot.rs`).

**Load-bearing invariant:** these caches are keyed by index (file index / channel index), not by identity. Any code path that mutates or reindexes channel data — removing a file, or removing/editing a computed channel (which shifts later computed channels' indices down) — **must clear the relevant caches**, or stale entries will silently render the wrong data against a different channel's index. See `remove_computed_channel` and the file-removal path in `src/app.rs` for the current call sites (`self.downsample_cache.clear()`, `self.minmax_cache.clear()`, `self.scatter_histogram_cache.clear()`).

## Key Features

- **Multi-ECU Support** - Haltech, ECUMaster, RomRaider, Speeduino, rusEFI, AiM, Link, Emerald, MegaSquirt, Motorsport Electronics, Woolich Racing Tuned, BlueDriver, DynamicEFI, and Locomotive log formats
- **Computed Channels** - Create virtual channels from mathematical formulas with time-shifting (e.g., `RPM[-1]`, `Boost@-0.5s`)
- **Analysis Algorithms** - AFR/Lambda drift and zone detection, derived metrics (VE, injector duty cycle), signal filters, and descriptive statistics (`src/analysis/`)
- **Claude Desktop / MCP Integration** - Embedded MCP server (`src/mcp/`) lets Claude control the running app over `http://localhost:52385/mcp` — select channels, add computed channels, query log data
- **Unit Preferences** - Users can select display units for temperature, pressure, speed, distance, fuel economy, volume, flow rate, and acceleration
- **Field Normalization** - Maps ECU-specific channel names to standardized names for cross-ECU comparison
- **Scatter Plot Tool** - XY scatter plot for analyzing channel correlations
- **Histogram Tool** - 2D heatmap view of channel distributions
- **Export Options** - Export charts as PNG or PDF; export selected channel data as CSV (full log or visible range, normalized names + display units)
- **Colorblind Mode** - Wong's optimized color palette for accessibility
- **Playback** - Play through log data at 0.25x to 8x speed
- **Cursor Tracking** - Lock view to follow cursor during playback/scrubbing
- **Min/Max Legend** - Shows peak values for each channel
- **Initial Zoom** - Charts start zoomed to first 60 seconds for better initial view
- **Multi-File Tabs** - Chrome-style tabs for working with multiple log files simultaneously
- **Internationalization** - `rust-i18n`-backed language selection (`src/i18n.rs`)
- **Settings Persistence** - Unit/display/normalization preferences survive restarts (see [Settings Persistence Contract](#settings-persistence-contract))
- **Auto-Update** - Automatic update checking and installation

### Keyboard Shortcuts

Handled in `UltraLogApp::handle_keyboard_shortcuts` (`src/app.rs`); ignored while a text field has focus.

- **Cmd/Ctrl+O** - Open file
- **Cmd/Ctrl+W** - Close current tab
- **Cmd/Ctrl+,** - Open Settings panel
- **Cmd/Ctrl+1/2/3** - Switch tool (Log Viewer / Scatter Plot / Histogram)
- **Cmd/Ctrl+Shift+F/C/T** - Switch side panel (Files / Tool Properties / Tools)
- **Arrow Left/Right** - Step cursor one record (Shift = 10 records)
- **Home/End** - Jump cursor to start/end of log
- **Escape** - Stop playback
- **Cmd/Ctrl+E** - Export the current view (chart, scatter plot, or histogram) as PNG
- **Space** - Toggle play/pause

## Key Dependencies

- **eframe/egui** (0.34) - Native GUI framework
- **egui_plot** (0.35) - Charting/plotting
- **rfd** (0.17) - Native file dialogs
- **open** (5) - Cross-platform URL/email opening
- **strum** (0.28) - Enum string conversion for channel types
- **regex** (1.12) - Log file parsing
- **meval** (0.2) - Mathematical expression evaluation for computed channels
- **ureq** (3.3) - HTTP client for auto-updates and OpenECU Alliance API
- **semver** (1.0) - Version comparison
- **serde_yml** (0.0.12) - YAML parsing for adapter/protocol specs (replaces deprecated `serde_yaml`)
- **printpdf** (0.9) - PDF generation
- **image** (0.25) - PNG export
- **memmap2** (0.9) - Memory-mapped file loading for large files
- **rayon** (1.11) - Parallel iteration for parsing
- **dirs** (6.0) - Cross-platform app data directory detection for cache and settings
- **rmcp** (0.12) - MCP server implementation (`src/mcp/`), used with `axum` as the HTTP transport
- **tokio** (1) - Async runtime backing the MCP server
- **axum** (0.8) - HTTP server framework for the embedded MCP endpoint
- **schemars** (1.0) - JSON schema generation for MCP tool definitions
- **arboard** (3.6) - Clipboard support (histogram copy/paste)
- **rust-i18n** (3.1) - Internationalization / locale strings
- **uuid** (1.0) - Anonymous user ID generation for analytics
- **chrono** (0.4) - Date/time parsing (Locomotive, Woolich timestamps)
- **encoding_rs** (0.8) - Character encoding detection/conversion (e.g., UTF-16 BlueDriver exports)
- **thiserror** / **anyhow** - Error handling
- **tracing** / **tracing-subscriber** - Structured logging

## Test Data

Example log files are in `exampleLogs/` organized by ECU type:

- `exampleLogs/haltech/` - Haltech NSP CSV exports
- `exampleLogs/aim/` - AiM XRK/DRK files
- `exampleLogs/link/` - Link ECU LLG files
- `exampleLogs/woolich/` - Woolich Racing Tuned CSV exports
- `exampleLogs/emerald/` - Emerald K6/M3D `.lg1`/`.lg2` files
- `exampleLogs/megasquirt/` - MegaSquirt TunerStudio CSV exports
- `exampleLogs/motorsportElectronics/` - Motorsport Electronics ME Tuner CSV exports
- `exampleLogs/bluedriver/` - BlueDriver OBD-II CSV exports
- `exampleLogs/dynamicEFI/` - DynamicEFI EBL WhatsUp CSV exports
- `exampleLogs/locomotive/` - Locomotive datalogger CSV exports
- `exampleLogs/rusefi/`, `exampleLogs/speeduino/` - MLG binary logs
- `exampleLogs/ecumaster/`, `exampleLogs/RomRaider/` - CSV exports
- Additional directories (`aem/`, `HondaTuningStudio/`) hold sample data for formats without an implemented parser yet — don't assume a directory's presence means the format is supported
