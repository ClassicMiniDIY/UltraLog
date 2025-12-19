# UltraLog

A high-performance ECU log viewer written in Rust.

![CI](https://github.com/SomethingNew71/UltraLog/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)

## Overview

UltraLog is a cross-platform desktop application for viewing and analyzing ECU (Engine Control Unit) log files from automotive performance tuning systems. Built with performance in mind, it handles large log files smoothly using advanced downsampling algorithms.

## Features

- **Multi-channel visualization** - Plot up to 10 data channels simultaneously with normalized overlay
- **High-performance rendering** - LTTB (Largest Triangle Three Buckets) downsampling for smooth performance with large datasets
- **Interactive timeline** - Click-to-seek, timeline scrubber, and playback controls (0.25x to 8x speed)
- **Cursor tracking mode** - Keep the cursor centered while scrubbing through data
- **Unit preferences** - Select between metric/imperial units for temperature, pressure, speed, distance, fuel economy, volume, flow rate, and acceleration
- **Min/Max legend** - See peak values for each channel at a glance
- **Colorblind mode** - Accessible color palette based on Wong's optimized palette
- **Drag and drop** - Simply drop log files onto the window to load them
- **Real-time values** - Legend displays live values at cursor position with units
- **Cross-platform** - Runs on Windows, macOS, and Linux

## Supported ECU Formats

- **Haltech** - Full support for Haltech CAN protocol log files with automatic unit conversion
- **ECUMaster** - Support for EMU Pro CSV exports with automatic unit inference
  - CSV files (semicolon or tab-delimited) exported from EMU Pro software
  - Automatic unit detection based on channel paths (RPM, temperature, pressure, lambda, etc.)
  - Note: The native `.emuprolog` binary format is not currently supported; please use CSV export from EMU Pro software
- More formats planned (MegaSquirt, AEM, Speeduino, etc.)

## Installation

### Pre-built Binaries

Download the latest release for your platform from the [Releases](https://github.com/SomethingNew71/UltraLog/releases) page:

- `ultralog-windows.exe` - Windows x64
- `ultralog-macos-intel` - macOS Intel
- `ultralog-macos-arm64` - macOS Apple Silicon
- `ultralog-linux` - Linux x64

### Building from Source

**Prerequisites:**

- [Rust](https://rustup.rs/) (latest stable)
- Platform-specific dependencies (see below)

**Linux dependencies:**

```bash
sudo apt-get install -y \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    libxkbcommon-dev \
    libssl-dev \
    libgtk-3-dev \
    libglib2.0-dev \
    libatk1.0-dev \
    libcairo2-dev \
    libpango1.0-dev \
    libgdk-pixbuf2.0-dev
```

**Build:**

```bash
git clone https://github.com/SomethingNew71/UltraLog.git
cd UltraLog
cargo build --release
```

The binary will be at `target/release/ultralog` (or `ultralog.exe` on Windows).

## Usage

1. Launch UltraLog
2. Click "Select a file" or drag and drop a log file onto the window
3. Select channels from the right panel to visualize (click to toggle)
4. Use the timeline scrubber or click on the chart to navigate
5. Use playback controls to animate through the data at various speeds
6. Configure unit preferences via the **Units** menu (temperature, pressure, speed, etc.)
7. Enable "Cursor Tracking" in View Options to keep the cursor centered while scrubbing
8. Enable "Color Blind Mode" in View Options for accessible colors

### Supported File Types

- `.csv` - CSV log files
- `.log` - Standard log files
- `.txt` - Text-based log files

## Tech Stack

- **Language:** Rust
- **GUI Framework:** [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) / [egui](https://github.com/emilk/egui)
- **Charting:** [egui_plot](https://github.com/emilk/egui/tree/master/crates/egui_plot)
- **File Dialogs:** [rfd](https://github.com/PolyMeilex/rfd)
- **Serialization:** serde / serde_json
- **Error Handling:** thiserror / anyhow
- **Logging:** tracing / tracing-subscriber

## Development

```bash
# Run in debug mode
cargo run

# Run with release optimizations
cargo run --release

# Run tests
cargo test

# Check formatting
cargo fmt --all -- --check

# Run clippy lints
cargo clippy -- -D warnings
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Author

Cole Gentry

## Related Projects

- [Classic Mini DIY](https://classicminidiy.com) - Classic Mini enthusiast website with tools and resources
