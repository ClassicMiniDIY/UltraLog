# UltraLog

<img width="500" alt="UltraLog Banner" src="https://github.com/user-attachments/assets/9924e8ae-ace8-4b16-a8d6-cbf456a8bc62" />

A high-performance, cross-platform ECU log viewer written in Rust.

![CI](https://github.com/SomethingNew71/UltraLog/actions/workflows/ci.yml/badge.svg)
![License](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)
![Version](https://img.shields.io/badge/version-2.0.0-green.svg)

---

## Table of Contents

- [UltraLog](#ultralog)
  - [Table of Contents](#table-of-contents)
  - [Overview](#overview)
  - [Features](#features)
    - [Data Visualization](#data-visualization)
    - [Timeline and Playback](#timeline-and-playback)
    - [Multi-File Support](#multi-file-support)
    - [Unit Conversion](#unit-conversion)
    - [Export Options](#export-options)
    - [Computed Channels](#computed-channels)
    - [Additional Tools](#additional-tools)
    - [Accessibility](#accessibility)
  - [Supported ECU Formats](#supported-ecu-formats)
    - [Haltech - Full Support](#haltech---full-support)
    - [ECUMaster EMU Pro - Full Support](#ecumaster-emu-pro---full-support)
    - [RomRaider - Full Support](#romraider---full-support)
    - [Speeduino / rusEFI - Full Support](#speeduino--rusefi---full-support)
    - [Coming Soon](#coming-soon)
  - [Installation](#installation)
    - [Pre-built Binaries](#pre-built-binaries)
    - [Building from Source](#building-from-source)
  - [Quick Start Guide](#quick-start-guide)
  - [User Guide](#user-guide)
    - [Loading Log Files](#loading-log-files)
    - [Visualizing Data](#visualizing-data)
    - [Timeline and Playback](#timeline-and-playback-1)
    - [Unit Preferences](#unit-preferences)
    - [Field Normalization](#field-normalization)
    - [Computed Channels](#computed-channels-1)
    - [Exporting Charts](#exporting-charts)
    - [Scatter Plot Tool](#scatter-plot-tool)
    - [Accessibility Features](#accessibility-features)
  - [Keyboard Shortcuts](#keyboard-shortcuts)
  - [Tech Stack](#tech-stack)
  - [Development](#development)
    - [Project Structure](#project-structure)
  - [Troubleshooting](#troubleshooting)
    - ["File format not recognized"](#file-format-not-recognized)
    - ["Application won't start on macOS"](#application-wont-start-on-macos)
    - ["Chart is slow or laggy"](#chart-is-slow-or-laggy)
    - ["Channels show wrong units"](#channels-show-wrong-units)
    - ["My ECU format isn't supported"](#my-ecu-format-isnt-supported)
  - [Legal Notices](#legal-notices)
    - [License](#license)
    - [Trademark Disclaimer](#trademark-disclaimer)
    - [Interoperability Statement](#interoperability-statement)
    - [Open Invention Network](#open-invention-network)
  - [Author](#author)
  - [Related Projects](#related-projects)
  - [Contributing](#contributing)
  - [Acknowledgments](#acknowledgments)

---

## Overview

UltraLog is an **independent, open-source** desktop application designed for automotive tuners, engineers, and enthusiasts who need to analyze ECU (Engine Control Unit) log data. Built with Rust for maximum performance, it handles large log files (millions of data points) smoothly using advanced downsampling algorithms while maintaining visual accuracy.

**Purpose:** UltraLog exists to provide **data interoperability** for the automotive tuning community. It reads standard export formats (CSV, binary logs) from various ECU systems, enabling users to analyze their own vehicle telemetry data in a unified, cross-platform tool without vendor lock-in.

**Key Benefits:**
- **Fast** - Handles massive log files without lag using LTTB downsampling
- **Universal** - Supports multiple ECU formats in one unified interface for cross-platform data analysis
- **Cross-platform** - Runs natively on Windows, macOS, and Linux
- **Accessible** - Colorblind-friendly palette and clear visualization
- **Open** - AGPL-3.0 licensed with documented format specifications for community benefit

---

## Features

### Data Visualization
- **Multi-channel overlay** - Plot up to 10 data channels simultaneously on a single chart
- **Normalized display** - All channels scaled 0-1 for easy comparison regardless of original units
- **Min/Max legend** - Peak values displayed for each channel at a glance
- **Real-time cursor values** - Legend shows live values at cursor position with proper units
- **High-performance rendering** - LTTB (Largest Triangle Three Buckets) algorithm reduces millions of points to 2,000 while preserving visual fidelity

### Timeline and Playback
- **Interactive timeline** - Click anywhere on the chart or use the scrubber to navigate
- **Playback controls** - Play, pause, stop with adjustable speed (0.25x, 0.5x, 1x, 2x, 4x, 8x)
- **Cursor tracking mode** - Keep the cursor centered while scrubbing through data
- **Manual time input** - Type a specific time in seconds to jump directly to that position

### Multi-File Support
- **Tab-based interface** - Open multiple log files with Chrome-style tabs
- **Drag and drop** - Simply drop files onto the window to load them
- **Per-tab state** - Each tab maintains its own channel selections and view settings
- **Duplicate detection** - Prevents loading the same file twice

### Unit Conversion
Configurable units for 8 measurement categories:
- **Temperature** - Kelvin, Celsius, Fahrenheit
- **Pressure** - kPa, PSI, Bar
- **Speed** - km/h, mph
- **Distance** - km, miles
- **Fuel Economy** - L/100km, MPG
- **Volume** - Liters, Gallons
- **Flow Rate** - L/min, GPM
- **Acceleration** - m/s², g

### Export Options
- **PNG Export** - Save chart views as PNG images
- **PDF Export** - Generate PDF reports of your visualizations

### Computed Channels
- **Formula-based virtual channels** - Create custom channels from mathematical expressions
- **Time-shifting support** - Reference past/future values with index offsets (`RPM[-1]`) or time offsets (`Boost@-0.5s`)
- **Reusable library** - Save formulas as templates to use across different log files
- **Full expression support** - All standard math functions: `sin`, `cos`, `sqrt`, `abs`, `max`, `min`, etc.
- **Example formulas:**
  - `RPM * 0.5` - Simple arithmetic
  - `"Manifold Pressure" - "Barometric Pressure"` - Quoted channel names with spaces
  - `RPM[-1] - RPM` - RPM change from previous sample
  - `max(AFR1, AFR2)` - Maximum of two channels
  - `sqrt(TPS * MAP)` - Complex calculations

### Additional Tools
- **Scatter Plot** - XY scatter visualization for channel correlation analysis
- **Normalization Editor** - Create custom field name mappings for cross-ECU comparison
- **Field Normalization** - Maps ECU-specific channel names to standard names (e.g., "Act_AFR" → "AFR")

### Accessibility
- **Colorblind mode** - Wong's optimized color palette designed for deuteranopia, protanopia, and tritanopia
- **Custom font** - Clear, readable Outfit typeface
- **Toast notifications** - Non-intrusive feedback for user actions

---

## Supported ECU Formats

### Haltech - Full Support
- **File type:** CSV exports from Haltech NSP software
- **Features:** 50+ channel types with automatic unit conversion
- **Supported data:** Pressure, temperature, RPM, throttle position, boost, ignition timing, fuel trim, and more

### ECUMaster EMU Pro - Full Support
- **File type:** CSV exports (semicolon or tab-delimited) from EMU Pro software
- **Features:** Hierarchical channel paths, automatic unit inference
- **Note:** Native `.emuprolog` binary format not supported; export to CSV from EMU Pro

### RomRaider - Full Support
- **File type:** CSV exports from RomRaider ECU logging software
- **Features:** Automatic unit extraction from column headers, Subaru ECU parameter support
- **Supported data:** Engine speed, load, AFR corrections, timing, knock, temperatures, and all standard Subaru ECU parameters

### Speeduino / rusEFI - Full Support
- **File type:** MegaLogViewer binary format (`.mlg`)
- **Features:** Binary format parsing with field type detection
- **Supported data:** All standard Speeduino/rusEFI channels with timestamps

### AiM - Full Support
- **File type:** XRK/DRK binary format (`.xrk`, `.drk`)
- **Features:** Pure Rust binary parser for AiM motorsport data loggers
- **Supported devices:** MXP, MXG, MXL2, EVO5, MyChron5, and other AiM data acquisition systems
- **Supported data:** All logged channels with lap times, GPS data, and metadata

### Link ECU - Full Support
- **File type:** Link log format (`.llg`)
- **Features:** Binary format parser for Link G4/G4+/G4X ECUs
- **Supported data:** All ECU parameters including RPM, MAP, AFR, ignition timing, temperatures, and custom channels

### Coming Soon
- MegaSquirt
- AEM
- MaxxECU
- MoTeC

---

## Installation

### Pre-built Binaries

Download the latest release for your platform from the [Releases](https://github.com/SomethingNew71/UltraLog/releases) page:

| Platform            | Download                      | Notes              |
| ------------------- | ----------------------------- | ------------------ |
| Windows x64         | `ultralog-windows.zip`        | Windows 10/11      |
| macOS Intel         | `ultralog-macos-intel.tar.gz` | macOS 10.15+       |
| macOS Apple Silicon | `ultralog-macos-arm64.tar.gz` | M1/M2/M3/M4 Macs   |
| Linux x64           | `ultralog-linux.tar.gz`       | Most distributions |

**Windows:**
1. Download and extract `ultralog-windows.zip`
2. Run `ultralog-windows.exe`
3. You may see a SmartScreen warning on first run - click "More info" → "Run anyway"

**macOS:**
1. Download the appropriate `.tar.gz` for your Mac
2. Extract: `tar -xzf ultralog-macos-*.tar.gz`
3. On first run, right-click the file and select "Open" to bypass Gatekeeper
4. Or remove quarantine attribute: `xattr -d com.apple.quarantine ultralog-macos-*`

**Linux:**
1. Download and extract: `tar -xzf ultralog-linux.tar.gz`
2. Run: `./ultralog-linux`

### Building from Source

**Prerequisites:**
- [Rust](https://rustup.rs/) (latest stable version)
- Platform-specific build tools (see below)

**Linux Dependencies (Ubuntu/Debian):**

```bash
sudo apt-get update
sudo apt-get install -y \
    build-essential \
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

**macOS Dependencies:**

```bash
xcode-select --install
```

**Windows Dependencies:**
- Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- Select "Desktop development with C++" workload

**Build Steps:**

```bash
# Clone the repository
git clone https://github.com/SomethingNew71/UltraLog.git
cd UltraLog

# Build release version (optimized)
cargo build --release

# The binary will be at:
# - Windows: target/release/ultralog.exe
# - macOS/Linux: target/release/ultralog
```

---

## Quick Start Guide

1. **Launch UltraLog** - Double-click the application or run from terminal

2. **Load a log file** - Either:
   - Click the "Select a file" button in the left sidebar
   - Drag and drop a log file onto the window

3. **Select channels** - Click channel names in the right panel to add them to the chart (up to 10)

4. **Navigate the data** -
   - Click anywhere on the chart to move the cursor
   - Use the timeline scrubber at the bottom
   - Use playback controls to animate through the data

5. **Customize your view** -
   - Change units via the **Units** menu
   - Enable **Cursor Tracking** to keep the cursor centered
   - Enable **Colorblind Mode** for accessible colors

---

## User Guide

### Loading Log Files

**Supported file extensions:** `.csv`, `.log`, `.txt`, `.mlg`

UltraLog automatically detects the ECU format based on file contents:
- **Haltech:** Identified by `%DataLog%` header
- **ECUMaster:** Identified by semicolon/tab-delimited CSV with `TIME` column
- **RomRaider:** Identified by comma-delimited CSV starting with `Time` column
- **Speeduino/rusEFI:** Identified by `MLVLG` binary header

**Loading multiple files:**
- Each file opens in its own tab
- Switch between tabs by clicking them
- Close tabs with the × button
- The same file cannot be loaded twice

### Visualizing Data

**Selecting channels:**
1. Use the search box to filter channels by name
2. Click a channel name to add it to the chart (turns blue when selected)
3. Click again to remove it from the chart
4. Up to 10 channels can be displayed simultaneously

**Understanding the chart:**
- All channels are normalized to 0-1 range for easy comparison
- The legend shows:
  - Channel name with color indicator
  - Min/Max values for the entire log
  - Current value at cursor position with units

**Zooming and panning:**
- Scroll to zoom in/out on the time axis
- Click and drag to pan the view
- Double-click to reset zoom

### Timeline and Playback

**Timeline controls (bottom of window):**
- **Play/Pause** - Start or pause data playback
- **Stop** - Stop playback and return to beginning
- **Speed selector** - Choose playback speed (0.25x to 8x)
- **Timeline scrubber** - Drag to seek through the data
- **Time input** - Type a specific time in seconds

**Cursor tracking:**
When enabled (View menu → Cursor Tracking), the chart automatically scrolls to keep the cursor centered as you scrub through data.

### Unit Preferences

Access via **Units** menu. Changes apply immediately to all displayed values.

| Category     | Options                     |
| ------------ | --------------------------- |
| Temperature  | Kelvin, Celsius, Fahrenheit |
| Pressure     | kPa, PSI, Bar               |
| Speed        | km/h, mph                   |
| Distance     | km, miles                   |
| Fuel Economy | L/100km, MPG                |
| Volume       | Liters, Gallons             |
| Flow Rate    | L/min, GPM                  |
| Acceleration | m/s², g                     |

**Note:** Unit conversion is applied at display time only - original data is never modified.

### Field Normalization

Field normalization maps ECU-specific channel names to standardized names, making it easier to compare data across different ECU systems.

**Enable/Disable:** View menu → Field Normalization

**Example mappings:**
- "Act_AFR", "AFR1", "Aft" → "AFR"
- "MAP", "Boost_Press" → "Manifold Pressure"
- "RPM", "Engine_Speed" → "Engine RPM"

**Custom mappings:**
1. Open View menu → Normalization Editor
2. Add custom source → target mappings
3. Changes apply immediately to channel names

### Computed Channels

Computed channels (also called virtual or math channels) allow you to create custom data channels from mathematical formulas. These formulas can reference existing log channels and use standard math functions.

**Accessing the Manager:**
- Tools menu → Computed Channels

**Creating a Computed Channel:**
1. Click "+ New Channel" in the Computed Channels window
2. Enter a **Name** for your channel (e.g., "Boost Delta")
3. Enter a **Formula** using channel names and math operators
4. Enter a **Unit** for display (e.g., "kPa", "psi", "%")
5. Optionally add a **Description** for reference
6. Click "Save" to add to your library

**Formula Syntax:**

**Basic operators:** `+`, `-`, `*`, `/`, `^` (power)

**Math functions:** `sin`, `cos`, `tan`, `sqrt`, `abs`, `max`, `min`, `exp`, `ln`, `log`, `floor`, `ceil`, `round`

**Channel references:**
- Simple names: `RPM`, `TPS`, `MAP`
- Names with spaces (use quotes): `"Manifold Pressure"`, `"Coolant Temp"`

**Time-shifting:**
- Index offset: `RPM[-1]` (previous sample), `RPM[+1]` (next sample)
- Time offset: `Boost@-0.5s` (value 0.5 seconds ago), `AFR@-0.1s` (value 100ms ago)

**Example Formulas:**

```text
RPM * 0.5
Simple arithmetic - half the RPM value

"Manifold Pressure" - "Barometric Pressure"
Gauge pressure calculation (quoted names with spaces)

RPM - RPM[-1]
RPM change from previous sample

max(AFR1, AFR2)
Maximum of two AFR sensors

sqrt(TPS * MAP / 100)
Complex calculation

(Boost@-0.5s - Boost) / 0.5
Boost rate of change (kPa/second)
```

**Using Computed Channels:**
1. Create and save a formula in the Computed Channels manager
2. Click "Apply to File" to add it to the current log
3. The computed channel appears in the channel list
4. Select it like any other channel to add to the chart

**Library Management:**
- Templates are saved globally and persist between sessions
- Stored in: `~/.config/ultralog/computed_channels.json` (Linux), `~/Library/Application Support/UltraLog/computed_channels.json` (macOS), `%APPDATA%\UltraLog\computed_channels.json` (Windows)
- Edit or delete templates from the Computed Channels window
- Templates can be reused across different log files

### Exporting Charts

**PNG Export:**
1. File menu → Export → PNG
2. Choose save location
3. Current chart view is saved as a PNG image

**PDF Export:**
1. File menu → Export → PDF
2. Choose save location
3. Chart is exported as a PDF document

### Scatter Plot Tool

The scatter plot tool visualizes the relationship between two channels.

**To use:**
1. Click the tool switcher (top-right area) and select "Scatter Plot"
2. Select X-axis channel from the dropdown
3. Select Y-axis channel from the dropdown
4. Data points are plotted showing correlation between the two channels

**Use cases:**
- Correlate AFR vs. manifold pressure
- Compare throttle position vs. engine load
- Identify tuning anomalies

### Accessibility Features

**Colorblind Mode:**
- Enable via View menu → Colorblind Mode
- Uses Wong's optimized 8-color palette
- Designed to be distinguishable for deuteranopia, protanopia, and tritanopia

**Standard color palette:**
Blue, Orange, Green, Red, Purple, Brown, Pink, Gray, Yellow, Cyan

**Colorblind palette:**
Black, Orange, Sky Blue, Bluish Green, Yellow, Blue, Vermillion, Reddish Purple

---

## Keyboard Shortcuts

| Action     | Shortcut       |
| ---------- | -------------- |
| Open file  | `Ctrl/Cmd + O` |
| Close tab  | `Ctrl/Cmd + W` |
| Export PNG | `Ctrl/Cmd + E` |
| Play/Pause | `Space`        |
| Stop       | `Escape`       |

---

## Tech Stack

| Component        | Technology                                                                                                     |
| ---------------- | -------------------------------------------------------------------------------------------------------------- |
| Language         | Rust (Edition 2021)                                                                                            |
| GUI Framework    | [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) / [egui](https://github.com/emilk/egui) 0.29 |
| Charting         | [egui_plot](https://github.com/emilk/egui/tree/master/crates/egui_plot) 0.29                                   |
| File Dialogs     | [rfd](https://github.com/PolyMeilex/rfd) 0.15                                                                  |
| Image Processing | [image](https://github.com/image-rs/image) 0.25                                                                |
| PDF Generation   | [printpdf](https://github.com/fschutt/printpdf) 0.7                                                            |
| Serialization    | serde / serde_json 1.0                                                                                         |
| Error Handling   | thiserror 2.0 / anyhow 1.0                                                                                     |
| Logging          | tracing / tracing-subscriber 0.3                                                                               |

---

## Development

```bash
# Run in debug mode (faster compile, slower runtime)
cargo run

# Run in release mode (slower compile, faster runtime)
cargo run --release

# Run the parser test utility
cargo run --bin test_parser -- path/to/logfile.csv

# Run tests
cargo test

# Check code formatting
cargo fmt --all -- --check

# Run lints
cargo clippy -- -D warnings
```

### Project Structure

```
UltraLog/
├── src/
│   ├── main.rs          # Application entry point
│   ├── app.rs           # Main application state and logic
│   ├── state.rs         # Core data types and structures
│   ├── units.rs         # Unit conversion system
│   ├── normalize.rs     # Field name normalization
│   ├── parsers/         # ECU format parsers
│   │   ├── haltech.rs   # Haltech CSV parser
│   │   ├── ecumaster.rs # ECUMaster CSV parser
│   │   ├── romraider.rs # RomRaider CSV parser
│   │   └── speeduino.rs # Speeduino MLG parser
│   └── ui/              # User interface components
│       ├── sidebar.rs   # File list and options
│       ├── channels.rs  # Channel selection panel
│       ├── chart.rs     # Main chart and LTTB algorithm
│       ├── timeline.rs  # Playback controls
│       └── ...
├── assets/              # Icons and fonts
├── exampleLogs/         # Sample log files for testing
└── Cargo.toml           # Project manifest
```

---

## Troubleshooting

### "File format not recognized"
- Ensure the file is from a supported ECU system
- For ECUMaster, export to CSV from EMU Pro software (native `.emuprolog` not supported)
- Check that the file is not corrupted

### "Application won't start on macOS"
- Right-click the application and select "Open"
- Go to System Preferences → Security & Privacy and allow the app
- If downloaded from the internet, you may need to remove the quarantine flag:
  ```bash
  xattr -d com.apple.quarantine /path/to/ultralog
  ```

### "Chart is slow or laggy"
- UltraLog handles large files well, but extremely large files (100MB+) may need a moment to process
- Try closing other applications to free up memory
- Ensure you're running the release build, not debug

### "Channels show wrong units"
- Check your Unit Preferences in the Units menu
- Some ECU systems report data in specific units - UltraLog attempts to convert automatically but may need manual adjustment

### "My ECU format isn't supported"
- Open an issue on [GitHub](https://github.com/SomethingNew71/UltraLog/issues) with a sample log file
- Include the ECU system name and software version used to export

---

## Legal Notices

### License

This project is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0) - see the [LICENSE](LICENSE.md) file for details.

### Trademark Disclaimer

UltraLog is an independent, open-source project created for **interoperability purposes** under fair use principles. This software reads data log files exported by various ECU systems to enable users to analyze their own vehicle data.

The following trademarks are the property of their respective owners:

- **Haltech** is a trademark of Haltech Engine Management Systems
- **ECUMaster** and **EMU Pro** are trademarks of ECUMaster
- **AiM**, **MXP**, **MXG**, **MXL2**, **EVO5**, and **MyChron5** are trademarks of AiM Technologies
- **Link ECU** is a trademark of Link Engine Management Ltd
- **Speeduino** is a trademark of the Speeduino project
- **rusEFI** is a trademark of the rusEFI project
- **RomRaider** is a trademark of the RomRaider project
- **Subaru** is a trademark of Subaru Corporation
- **MegaSquirt** is a trademark of Bowling and Grippo
- **AEM** is a trademark of AEM Performance Electronics
- **MoTeC** is a trademark of MoTeC Pty Ltd
- **MaxxECU** is a trademark of MaxxECU

**UltraLog is not affiliated with, endorsed by, or sponsored by any of these companies or projects.** All product names, logos, and brands are property of their respective owners and are used solely for identification and interoperability purposes.

### Interoperability Statement

This software is developed under the principles established by [Sega v. Accolade](https://en.wikipedia.org/wiki/Sega_Enterprises,_Ltd._v._Accolade,_Inc.) and similar legal precedents that recognize the legitimacy of reverse engineering for interoperability. UltraLog:

1. **Reads publicly exported data** - Parses CSV, binary, and other log formats that users export from their own ECU software
2. **Does not circumvent copy protection** - Works only with user-accessible exported data files
3. **Enables data portability** - Allows users to analyze their vehicle telemetry in a unified, cross-platform tool
4. **Documents format specifications** - Publishes technical details for community benefit and defensive purposes

For technical format specifications, see [docs/FORMAT_SPECIFICATIONS.md](docs/FORMAT_SPECIFICATIONS.md).

### Open Invention Network

<a href="https://openinventionnetwork.com">
  <img src="https://openinventionnetwork.com/wp-content/uploads/2024/01/oin-logo-blue.png" alt="Open Invention Network Community Member" width="200">
</a>

UltraLog is a community member of the [Open Invention Network](https://openinventionnetwork.com) (OIN), a shared defensive patent pool with the mission to protect Linux and open-source software from patent aggression.

OIN's cross-license agreement is available royalty-free to any party that agrees not to assert its patents against the Linux System. As of 2024, OIN has grown to include over 3,800 community members committed to defending open-source software.

By joining OIN, UltraLog commits to:

- **Patent non-aggression** - Not asserting patents against the Linux System
- **Community defense** - Contributing to a shared defensive patent pool
- **Open-source values** - Supporting the free exchange of ideas and innovation

Learn more at [openinventionnetwork.com](https://openinventionnetwork.com).

---

## Author

**Cole Gentry**

- GitHub: [@SomethingNew71](https://github.com/SomethingNew71)
- Website: [Classic Mini DIY](https://classicminidiy.com)

---

## Related Projects

- [Classic Mini DIY](https://classicminidiy.com) - Classic Mini enthusiast website with tools, calculators, and resources

---

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

---

## Acknowledgments

- [egui](https://github.com/emilk/egui) - The immediate mode GUI library that makes this possible
- Wong's colorblind-safe palette for accessibility research
- The automotive tuning community for feedback and feature requests
