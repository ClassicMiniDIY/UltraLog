# Installation

This guide covers all methods for installing UltraLog on your system.

## Pre-built Binaries (Recommended)

Download the latest release for your platform from the [Releases page](https://github.com/SomethingNew71/UltraLog/releases).

### Available Downloads

| Platform | Filename | Notes |
|----------|----------|-------|
| Windows x64 | `ultralog-windows.zip` | Windows 10/11 |
| macOS Intel | `ultralog-macos-intel.tar.gz` | macOS 10.15+ (Catalina and later) |
| macOS Apple Silicon | `ultralog-macos-arm64.tar.gz` | M1/M2/M3/M4 Macs |
| Linux x64 | `ultralog-linux.tar.gz` | Most distributions (Ubuntu, Fedora, etc.) |

---

## Windows Installation

### Steps

1. Download `ultralog-windows.zip` from the [Releases page](https://github.com/SomethingNew71/UltraLog/releases)
2. Right-click the zip file and select **"Extract All..."**
3. Run `ultralog-windows.exe` from the extracted folder

### SmartScreen Warning

On first run, you may see a Windows SmartScreen warning because the application is not code-signed:

1. Click **"More info"**
2. Click **"Run anyway"**

This is a one-time warning and won't appear again for this file.

### Optional: Add to PATH

To run UltraLog from anywhere in the command line:

1. Move `ultralog-windows.exe` to a permanent location (e.g., `C:\Program Files\UltraLog\`)
2. Add that folder to your system PATH environment variable

---

## macOS Installation

### Steps

1. Download the appropriate `.tar.gz` file for your Mac:
   - **Intel Mac:** `ultralog-macos-intel.tar.gz`
   - **Apple Silicon (M1/M2/M3/M4):** `ultralog-macos-arm64.tar.gz`
2. Extract the archive (double-click or use Terminal):
   ```bash
   cd ~/Downloads
   tar -xzf ultralog-macos-arm64.tar.gz
   ```
3. Run the application: `./ultralog-macos-arm64` (or `ultralog-macos-intel`)

### Gatekeeper Warning

macOS may block the application because it's from an "unidentified developer":

**Method 1: Right-click to Open (Recommended)**
1. Right-click (or Control-click) the extracted file
2. Select **"Open"** from the context menu
3. Click **"Open"** in the dialog

**Method 2: System Preferences**
1. Try to open the app (it will be blocked)
2. Go to **System Settings** → **Privacy & Security**
3. Scroll down and click **"Open Anyway"** next to the UltraLog message

**Method 3: Remove Quarantine Attribute**
```bash
xattr -d com.apple.quarantine ~/Downloads/ultralog-macos-*
```

### Optional: Move to Applications

For easier access:
```bash
mv ~/Downloads/ultralog-macos-arm64 /Applications/UltraLog
```

---

## Linux Installation

### Steps

1. Download `ultralog-linux.tar.gz` from the [Releases page](https://github.com/SomethingNew71/UltraLog/releases)
2. Extract the archive:
   ```bash
   tar -xzf ultralog-linux.tar.gz
   ```
3. Run the application:
   ```bash
   ./ultralog-linux
   ```

### Optional: Install System-wide

```bash
sudo mv ultralog-linux /usr/local/bin/ultralog
```

Now you can run `ultralog` from anywhere.

### Desktop Entry (Optional)

Create a desktop entry for your application menu:

```bash
cat > ~/.local/share/applications/ultralog.desktop << 'EOF'
[Desktop Entry]
Name=UltraLog
Comment=ECU Log Viewer
Exec=/usr/local/bin/ultralog
Icon=ultralog
Terminal=false
Type=Application
Categories=Development;Engineering;
EOF
```

---

## Building from Source

If you prefer to build from source or need to modify the code:

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable version)
- Git
- Platform-specific build tools

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Linux Build Dependencies

**Ubuntu/Debian:**
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

**Fedora:**
```bash
sudo dnf install -y \
    gcc \
    libxcb-devel \
    libxkbcommon-devel \
    openssl-devel \
    gtk3-devel \
    glib2-devel \
    atk-devel \
    cairo-devel \
    pango-devel \
    gdk-pixbuf2-devel
```

**Arch Linux:**
```bash
sudo pacman -S --needed \
    base-devel \
    libxcb \
    libxkbcommon \
    openssl \
    gtk3
```

### macOS Build Dependencies

```bash
xcode-select --install
```

### Windows Build Dependencies

1. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
2. Select **"Desktop development with C++"** workload during installation

### Build Steps

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

### Run Without Building Release

For quick testing during development:

```bash
# Debug build (faster compile, slower runtime)
cargo run

# Release build (slower compile, faster runtime)
cargo run --release
```

---

## Verifying Installation

After installation, verify UltraLog works correctly:

1. Launch UltraLog
2. You should see the main window with:
   - Left sidebar (Files section)
   - Center area (empty chart)
   - Right sidebar (Channels section)
3. Try loading a sample log file to confirm file parsing works

---

## Updating UltraLog

### Pre-built Binaries

1. Download the new version from [Releases](https://github.com/SomethingNew71/UltraLog/releases)
2. Replace the old binary with the new one
3. On macOS/Linux, you may need to re-apply permissions (`chmod +x`)

### From Source

```bash
cd UltraLog
git pull
cargo build --release
```

---

## Uninstalling

### Pre-built Binaries

Simply delete the executable file.

### From Source

```bash
rm -rf /path/to/UltraLog
```

---

## Next Steps

- [[Getting-Started]] - Learn the basics of using UltraLog
- [[Supported-ECU-Formats]] - Check if your ECU system is supported
- [[Troubleshooting]] - Solutions for common installation issues
