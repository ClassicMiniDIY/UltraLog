# Frequently Asked Questions

Common questions about UltraLog.

---

## General

### What is UltraLog?

UltraLog is a high-performance, cross-platform desktop application for viewing and analyzing ECU (Engine Control Unit) log files. It's designed for automotive tuners, engineers, and enthusiasts who need to analyze data from performance tuning systems.

### Is UltraLog free?

Yes, UltraLog is free and open source under the AGPL-3.0 license. You can use it for personal or commercial purposes, with the requirement that modifications must be shared under the same license.

### What platforms does UltraLog support?

- Windows 10/11 (x64)
- macOS 10.15+ (Intel and Apple Silicon)
- Linux (most distributions, x64)

### Do I need to install anything else?

No additional software is required. UltraLog is a standalone application. On Linux, you may need to install some system libraries (see [[Installation]]).

---

## ECU Support

### Which ECU systems are supported?

Currently supported:
- **Haltech** - Full support (CSV from NSP)
- **ECUMaster EMU Pro** - Full support (CSV export)
- **Speeduino / rusEFI** - Full support (MLG binary)

Coming soon:
- MegaSquirt
- AEM
- MaxxECU
- MoTeC
- Link ECU

### My ECU isn't supported. Can you add it?

Yes! Open an issue on [GitHub](https://github.com/SomethingNew71/UltraLog/issues) with:
- ECU system name and model
- Software version used for logging
- A sample log file (if possible)
- Any documentation links

### Why do I need to export to CSV for some ECUs?

Some ECU software uses proprietary binary formats that aren't publicly documented. CSV export is universally supported and ensures accurate data parsing.

---

## File Handling

### What file types can UltraLog open?

- `.csv` - CSV log files
- `.log` - Standard log files
- `.txt` - Text-based log files
- `.mlg` - MegaLogViewer binary (Speeduino/rusEFI)

### How do I know which format my file is?

UltraLog automatically detects the format based on file contents, not the file extension. Just load the file and UltraLog will identify it.

### How large of files can UltraLog handle?

UltraLog uses LTTB downsampling to handle very large files efficiently:
- Files up to 50MB load in seconds
- Files 100MB+ may take longer but are fully supported
- Memory usage is approximately 1-2× file size

### Can I open multiple files at once?

Yes! Each file opens in its own tab. You can switch between tabs and each maintains its own channel selections.

### Why can't I load the same file twice?

This is intentional to prevent confusion. If you need to reload a file:
1. Close the existing tab
2. Load the file again

---

## Data Display

### Why are all channels shown on a 0-1 scale?

UltraLog normalizes all channels to a 0-1 range so you can compare data with vastly different scales on the same chart. For example, RPM (0-8000) and AFR (10-18) can be visually compared.

The actual values with proper units are always shown in the legend.

### How many channels can I display at once?

Up to 10 channels can be displayed simultaneously. This limit ensures the chart remains readable and maintains performance.

### Why do some channel names look different than in my ECU software?

If Field Normalization is enabled (View → Field Normalization), channel names are standardized for consistency. Disable this to see original ECU names.

### What do the Min/Max values in the legend mean?

- **Min** - The lowest value for that channel across the entire log
- **Max** - The highest value for that channel across the entire log

These help you understand the range of data at a glance.

### How do I see the exact value at a specific time?

1. Click on the chart at the desired time
2. The legend shows the value at the cursor position
3. For precise timing, type the time in seconds in the timeline input

---

## Units

### How do I change units?

Use the **Units** menu to change temperature, pressure, speed, and other measurements to your preferred units.

### Does changing units modify my data?

No. Unit conversion is display-only. Your original data is never modified. You can switch between units freely without affecting the underlying data.

### Why does my temperature show 363 instead of 90°C?

Some ECUs (like Haltech) store temperature in Kelvin. Check your unit preference - if set to Kelvin, you'll see 363K. Change to Celsius to see 90°C.

---

## Playback

### What playback speeds are available?

- 0.25x (quarter speed)
- 0.5x (half speed)
- 1.0x (real-time)
- 2.0x (double speed)
- 4.0x (4x speed)
- 8.0x (8x speed)

### What is Cursor Tracking mode?

When enabled, the chart automatically scrolls to keep the cursor centered as you scrub through data. This is useful for following data through long logs.

### Can I jump to a specific time?

Yes. Click on the time display in the timeline controls and type the time in seconds.

---

## Export

### What export formats are available?

- **PNG** - Image file of the chart
- **PDF** - PDF document of the chart

### How do I export the raw data?

Currently, UltraLog is focused on visualization. Data export to CSV/Excel is planned for a future release.

### Why is my exported image low resolution?

The export captures the current window view. Maximize the window before exporting for higher resolution.

---

## Performance

### Why is UltraLog so fast with large files?

UltraLog uses the LTTB (Largest Triangle Three Buckets) algorithm to reduce millions of data points to a maximum of 2,000 for display. This preserves visual accuracy while ensuring smooth rendering.

### How can I improve performance?

1. Use the release build (not debug)
2. Close tabs for files you're not using
3. Select only the channels you need
4. Ensure adequate system memory (8GB+ recommended)

### Does UltraLog use GPU acceleration?

UltraLog uses OpenGL (via glow) for rendering, which provides hardware acceleration on most systems.

---

## Accessibility

### Is there a colorblind-friendly mode?

Yes! Enable **View → Colorblind Mode** for Wong's optimized color palette, designed to be distinguishable for various types of color vision deficiency.

### Can I change the font size?

Font size is currently fixed. Custom font sizing is being considered for a future release.

---

## Technical

### What is UltraLog written in?

UltraLog is written in Rust using the egui/eframe GUI framework.

### Is UltraLog open source?

Yes, UltraLog is open source under the AGPL-3.0 license. The source code is available at [GitHub](https://github.com/SomethingNew71/UltraLog).

### Can I contribute to UltraLog?

Yes! Contributions are welcome. See [[Development]] for information on building from source and contributing.

### Does UltraLog send any data to the internet?

No. UltraLog is completely offline. No data is sent anywhere. All processing happens locally on your computer.

### Where does UltraLog store settings?

Currently, settings are stored in memory and reset when the application closes. Persistent settings storage is planned for a future release.

---

## Troubleshooting

### Where can I get help?

1. Check the [[Troubleshooting]] page
2. Search [GitHub Issues](https://github.com/SomethingNew71/UltraLog/issues)
3. Open a new issue if needed

### How do I report a bug?

Open an issue on [GitHub](https://github.com/SomethingNew71/UltraLog/issues) with:
- UltraLog version
- Operating system
- Steps to reproduce
- Sample file if applicable

### How do I request a feature?

Open an issue on [GitHub](https://github.com/SomethingNew71/UltraLog/issues) and describe:
- What feature you'd like
- Your use case
- Any examples or references

---

## Version and Updates

### What version am I running?

Check **Help → About** for version information.

### How do I update UltraLog?

1. Download the latest release from [GitHub Releases](https://github.com/SomethingNew71/UltraLog/releases)
2. Replace the old binary with the new one
3. On macOS/Linux, you may need to re-apply execute permissions

### Does UltraLog auto-update?

No. Updates are manual downloads from GitHub Releases. This ensures you control when updates are applied.

---

## Still Have Questions?

If your question isn't answered here:

1. Check the [[User-Guide]] for detailed feature documentation
2. See [[Troubleshooting]] for common issues
3. Open a GitHub Issue for support

---

## Next Steps

- [[Getting-Started]] - Quick introduction
- [[User-Guide]] - Complete feature reference
- [[Troubleshooting]] - Common issues and solutions
