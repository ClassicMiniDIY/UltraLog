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
├── lib.rs            # Library exports and module declarations
├── app.rs            # Main application state and eframe::App impl
├── state.rs          # Core data types and constants
├── units.rs          # Unit preference types and conversions
├── normalize.rs      # Field name normalization system
├── computed.rs       # Computed channels data types and library
├── expression.rs     # Formula parsing and evaluation engine
├── updater.rs        # Auto-update functionality
├── analytics.rs      # Privacy-respecting analytics
├── parsers/
│   ├── mod.rs        # Parser module exports
│   ├── types.rs      # Core parser types (Log, Channel, Value, etc.)
│   ├── haltech.rs    # Haltech ECU log parser
│   ├── ecumaster.rs  # ECUMaster EMU Pro CSV parser
│   ├── romraider.rs  # RomRaider CSV parser
│   ├── speeduino.rs  # Speeduino/rusEFI MLG binary parser
│   ├── aim.rs        # AiM XRK/DRK binary parser
│   └── link.rs       # Link ECU LLG binary parser
└── ui/
    ├── mod.rs                        # UI module exports
    ├── sidebar.rs                    # File list and view options panel
    ├── channels.rs                   # Channel selection and cards
    ├── chart.rs                      # Chart rendering, legends, LTTB algorithm
    ├── timeline.rs                   # Timeline scrubber and playback controls
    ├── menu.rs                       # Menu bar (Units, Tools, Help menus)
    ├── toast.rs                      # Toast notification system
    ├── icons.rs                      # Custom icon drawing utilities
    ├── tab_bar.rs                    # Multi-file tab interface
    ├── tool_switcher.rs              # Switch between chart and scatter plot
    ├── scatter_plot.rs               # XY scatter plot visualization
    ├── export.rs                     # PNG and PDF export functionality
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
  - `CacheKey`, `LoadResult`, `LoadingState` - Internal state types
  - Color palette constants (`CHART_COLORS`, `COLORBLIND_COLORS`)

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

### UI Modules (src/ui/)

UI rendering is split into focused modules that implement methods on `UltraLogApp`:

- **`sidebar.rs`** - Left panel: file list, drop zone, view options (cursor tracking, colorblind mode, field normalization)
- **`channels.rs`** - Right panel: channel list with search, selected channel cards, computed channels
- **`chart.rs`** - Main chart with egui_plot, min/max legend overlay, LTTB downsampling, normalization
- **`timeline.rs`** - Bottom panel: playback controls (play/pause/stop), speed selector, timeline scrubber
- **`menu.rs`** - Top menu bar with Units submenu (8 unit categories), Tools menu, and Help menu
- **`toast.rs`** - Toast notification overlay for user feedback
- **`icons.rs`** - Custom icon drawing (upload icon for drop zone)
- **`tab_bar.rs`** - Chrome-style tabs for multi-file support
- **`tool_switcher.rs`** - Toggle between chart view and scatter plot view
- **`scatter_plot.rs`** - XY scatter plot for channel correlation analysis
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

**Supported ECU Systems:**

- Haltech (via NSP CSV export)
- ECUMaster EMU Pro (via CSV export)
- RomRaider (Subaru ECUs)
- Speeduino (MLG binary)
- rusEFI (MLG binary)
- AiM (XRK/DRK binary)
- Link ECU (LLG binary)

To add a new ECU format:

1. Create a new module in `src/parsers/` (e.g., `megasquirt.rs`)
2. Define format-specific channel types and metadata structs
3. Implement the `Parseable` trait
4. Add enum variants to `Channel`, `Meta`, and wire up in `mod.rs`
5. Update detection logic in `app.rs` file loading

### Data Flow

1. Files are loaded asynchronously via `start_loading_file()` → background thread
2. Parser converts file (CSV or binary) to `Log` struct with channels, times, and data vectors
3. Field normalization optionally applied to standardize channel names
4. User selects channels (raw or computed) → added to `selected_channels` with unique color assignment
5. Computed channels evaluate formulas across all records with time-shifting support
6. Chart renders downsampled data from cache, limited to 2000 points per channel using LTTB algorithm
7. Unit conversions applied at display time based on `unit_preferences`

## Key Features

- **Multi-ECU Support** - Supports Haltech, ECUMaster, RomRaider, Speeduino, rusEFI, AiM, and Link ECU formats
- **Computed Channels** - Create virtual channels from mathematical formulas with time-shifting (e.g., `RPM[-1]`, `Boost@-0.5s`)
- **Unit Preferences** - Users can select display units for temperature, pressure, speed, distance, fuel economy, volume, flow rate, and acceleration
- **Field Normalization** - Maps ECU-specific channel names to standardized names for cross-ECU comparison
- **Scatter Plot Tool** - XY scatter plot for analyzing channel correlations
- **Export Options** - Export charts as PNG or PDF
- **Colorblind Mode** - Wong's optimized color palette for accessibility
- **Playback** - Play through log data at 0.25x to 8x speed
- **Cursor Tracking** - Lock view to follow cursor during playback/scrubbing
- **Min/Max Legend** - Shows peak values for each channel
- **Initial Zoom** - Charts start zoomed to first 60 seconds for better initial view
- **Multi-File Tabs** - Chrome-style tabs for working with multiple log files simultaneously
- **Auto-Update** - Automatic update checking and installation

## Key Dependencies

- **eframe/egui** (0.33) - Native GUI framework
- **egui_plot** (0.34) - Charting/plotting
- **rfd** (0.16) - Native file dialogs
- **open** (5) - Cross-platform URL/email opening
- **strum** (0.27) - Enum string conversion for channel types
- **regex** (1.12) - Log file parsing
- **meval** (0.2) - Mathematical expression evaluation for computed channels
- **ureq** (3.0) - HTTP client for auto-updates
- **semver** (1.0) - Version comparison
- **printpdf** (0.7) - PDF generation
- **image** (0.25) - PNG export
- **memmap2** (0.9) - Memory-mapped file loading for large files
- **rayon** (1.11) - Parallel iteration for parsing

## Test Data

Example log files are in `exampleLogs/` organized by ECU type:

- `exampleLogs/haltech/` - Haltech NSP CSV exports
- `exampleLogs/aim/` - AiM XRK/DRK files
- `exampleLogs/link/` - Link ECU LLG files
- Additional formats for parser testing
