#!/bin/bash
set -e

# =============================================================================
# UltraLog Local Release Build Script
# =============================================================================
# This script builds release binaries for all platforms from your Mac.
#
# PREREQUISITES (one-time setup):
#   ./scripts/build-release.sh --setup
#
# USAGE:
#   ./scripts/build-release.sh          # Build all platforms
#   ./scripts/build-release.sh macos    # Build macOS only (Intel + ARM)
#   ./scripts/build-release.sh linux    # Build Linux only
#   ./scripts/build-release.sh windows  # Build Windows only
#   ./scripts/build-release.sh --setup  # Install prerequisites
#   ./scripts/build-release.sh --clean  # Clean build artifacts
#   ./scripts/build-release.sh --install # Install locally (macOS only)
# =============================================================================

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_DIR/dist"
APP_NAME="UltraLog"
BUNDLE_ID="com.ultralog.app"
VERSION=$(grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')

print_header() {
    echo -e "\n${BLUE}═══════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════${NC}\n"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

# Check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Setup prerequisites
setup() {
    print_header "Setting up build prerequisites"

    # Check for Homebrew
    if ! command_exists brew; then
        print_error "Homebrew not found. Install from https://brew.sh"
        exit 1
    fi

    # Install Rust targets
    echo "Installing Rust targets..."
    rustup target add x86_64-apple-darwin
    rustup target add aarch64-apple-darwin
    rustup target add x86_64-unknown-linux-gnu
    rustup target add x86_64-pc-windows-msvc
    print_success "Rust targets installed"

    # Install cargo-zigbuild for Linux cross-compilation
    echo "Installing cargo-zigbuild (for Linux builds)..."
    if ! command_exists cargo-zigbuild; then
        cargo install cargo-zigbuild
    fi
    print_success "cargo-zigbuild installed"

    # Install zig (required by cargo-zigbuild)
    echo "Installing zig..."
    if ! command_exists zig; then
        brew install zig
    fi
    print_success "zig installed"

    # Install cargo-xwin for Windows cross-compilation
    echo "Installing cargo-xwin (for Windows builds)..."
    if ! command_exists cargo-xwin; then
        cargo install cargo-xwin
    fi
    print_success "cargo-xwin installed"

    # Install create-dmg for DMG creation
    echo "Installing create-dmg..."
    if ! command_exists create-dmg; then
        brew install create-dmg
    fi
    print_success "create-dmg installed"

    echo ""
    print_success "Setup complete! You can now run: ./scripts/build-release.sh"
}

# Clean build artifacts
clean() {
    print_header "Cleaning build artifacts"
    rm -rf "$OUTPUT_DIR"
    cargo clean
    print_success "Cleaned"
}

# Create macOS .app bundle
create_app_bundle() {
    local ARCH=$1
    local BINARY_PATH=$2
    local APP_DIR="$OUTPUT_DIR/$APP_NAME-$ARCH.app"

    echo "Creating app bundle for $ARCH..."

    # Create bundle structure
    mkdir -p "$APP_DIR/Contents/MacOS"
    mkdir -p "$APP_DIR/Contents/Resources"

    # Copy binary
    cp "$BINARY_PATH" "$APP_DIR/Contents/MacOS/ultralog"
    chmod +x "$APP_DIR/Contents/MacOS/ultralog"

    # Create Info.plist
    cat > "$APP_DIR/Contents/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>$APP_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>ultralog</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeExtensions</key>
            <array>
                <string>csv</string>
                <string>log</string>
                <string>txt</string>
                <string>mlg</string>
            </array>
            <key>CFBundleTypeName</key>
            <string>ECU Log File</string>
            <key>CFBundleTypeRole</key>
            <string>Viewer</string>
        </dict>
    </array>
</dict>
</plist>
EOF

    # Create PkgInfo
    echo -n "APPL????" > "$APP_DIR/Contents/PkgInfo"

    # Copy icon if exists, otherwise create a placeholder
    if [ -f "$PROJECT_DIR/assets/icons/mac.icns" ]; then
        cp "$PROJECT_DIR/assets/icons/mac.icns" "$APP_DIR/Contents/Resources/AppIcon.icns"
    elif [ -f "$PROJECT_DIR/assets/icons/mac.png" ]; then
        # Convert PNG to ICNS if needed
        create_icns_from_png "$PROJECT_DIR/assets/icons/mac.png" "$APP_DIR/Contents/Resources/AppIcon.icns"
    fi

    # Ad-hoc sign the app bundle
    codesign --force --deep --sign - "$APP_DIR"

    print_success "Created: $APP_DIR"
}

# Convert PNG to ICNS
create_icns_from_png() {
    local PNG_PATH=$1
    local ICNS_PATH=$2

    local ICONSET_DIR=$(mktemp -d)/AppIcon.iconset
    mkdir -p "$ICONSET_DIR"

    # Generate all required sizes
    sips -z 16 16     "$PNG_PATH" --out "$ICONSET_DIR/icon_16x16.png" >/dev/null 2>&1
    sips -z 32 32     "$PNG_PATH" --out "$ICONSET_DIR/icon_16x16@2x.png" >/dev/null 2>&1
    sips -z 32 32     "$PNG_PATH" --out "$ICONSET_DIR/icon_32x32.png" >/dev/null 2>&1
    sips -z 64 64     "$PNG_PATH" --out "$ICONSET_DIR/icon_32x32@2x.png" >/dev/null 2>&1
    sips -z 128 128   "$PNG_PATH" --out "$ICONSET_DIR/icon_128x128.png" >/dev/null 2>&1
    sips -z 256 256   "$PNG_PATH" --out "$ICONSET_DIR/icon_128x128@2x.png" >/dev/null 2>&1
    sips -z 256 256   "$PNG_PATH" --out "$ICONSET_DIR/icon_256x256.png" >/dev/null 2>&1
    sips -z 512 512   "$PNG_PATH" --out "$ICONSET_DIR/icon_256x256@2x.png" >/dev/null 2>&1
    sips -z 512 512   "$PNG_PATH" --out "$ICONSET_DIR/icon_512x512.png" >/dev/null 2>&1
    sips -z 1024 1024 "$PNG_PATH" --out "$ICONSET_DIR/icon_512x512@2x.png" >/dev/null 2>&1

    # Convert to icns
    iconutil -c icns "$ICONSET_DIR" -o "$ICNS_PATH"

    # Cleanup
    rm -rf "$(dirname "$ICONSET_DIR")"
}

# Create DMG
create_dmg_file() {
    local ARCH=$1
    local APP_DIR="$OUTPUT_DIR/$APP_NAME-$ARCH.app"
    local DMG_PATH="$OUTPUT_DIR/$APP_NAME-$ARCH.dmg"

    echo "Creating DMG for $ARCH..."

    # Remove old DMG if exists
    rm -f "$DMG_PATH"

    if command_exists create-dmg; then
        # Use create-dmg for a nice DMG with background
        create-dmg \
            --volname "$APP_NAME" \
            --volicon "$APP_DIR/Contents/Resources/AppIcon.icns" \
            --window-pos 200 120 \
            --window-size 600 400 \
            --icon-size 100 \
            --icon "$APP_NAME-$ARCH.app" 150 190 \
            --hide-extension "$APP_NAME-$ARCH.app" \
            --app-drop-link 450 190 \
            "$DMG_PATH" \
            "$APP_DIR" \
            2>/dev/null || {
                # Fallback to simple DMG if create-dmg fails
                hdiutil create -volname "$APP_NAME" -srcfolder "$APP_DIR" -ov -format UDZO "$DMG_PATH"
            }
    else
        # Simple DMG creation
        hdiutil create -volname "$APP_NAME" -srcfolder "$APP_DIR" -ov -format UDZO "$DMG_PATH"
    fi

    print_success "Created: $DMG_PATH"
}

# Build macOS (Intel + ARM)
build_macos() {
    print_header "Building macOS (Intel + ARM)"

    mkdir -p "$OUTPUT_DIR"

    # Build Intel
    echo "Building x86_64-apple-darwin (Intel)..."
    cargo build --release --target x86_64-apple-darwin
    print_success "Intel build complete"

    # Build ARM
    echo "Building aarch64-apple-darwin (Apple Silicon)..."
    cargo build --release --target aarch64-apple-darwin
    print_success "ARM build complete"

    # Create app bundles
    create_app_bundle "intel" "$PROJECT_DIR/target/x86_64-apple-darwin/release/ultralog"
    create_app_bundle "arm64" "$PROJECT_DIR/target/aarch64-apple-darwin/release/ultralog"

    # Create DMGs
    create_dmg_file "intel"
    create_dmg_file "arm64"

    # Also create tar.gz for GitHub releases
    echo "Creating tar.gz archives..."
    (cd "$OUTPUT_DIR" && tar -czvf "ultralog-macos-intel.tar.gz" "$APP_NAME-intel.app")
    (cd "$OUTPUT_DIR" && tar -czvf "ultralog-macos-arm64.tar.gz" "$APP_NAME-arm64.app")
    print_success "Created tar.gz archives"
}

# Build Linux
build_linux() {
    print_header "Building Linux (x86_64)"

    if ! command_exists cargo-zigbuild; then
        print_error "cargo-zigbuild not found. Run: ./scripts/build-release.sh --setup"
        exit 1
    fi

    echo "Building x86_64-unknown-linux-gnu..."
    cargo zigbuild --release --target x86_64-unknown-linux-gnu
    print_success "Linux build complete"

    # Package
    mkdir -p "$OUTPUT_DIR"
    cp "$PROJECT_DIR/target/x86_64-unknown-linux-gnu/release/ultralog" "$OUTPUT_DIR/ultralog-linux"
    chmod +x "$OUTPUT_DIR/ultralog-linux"
    (cd "$OUTPUT_DIR" && tar -czvf ultralog-linux.tar.gz ultralog-linux && rm ultralog-linux)
    print_success "Packaged: $OUTPUT_DIR/ultralog-linux.tar.gz"
}

# Build Windows
build_windows() {
    print_header "Building Windows (x86_64)"

    if ! command_exists cargo-xwin; then
        print_error "cargo-xwin not found. Run: ./scripts/build-release.sh --setup"
        exit 1
    fi

    echo "Building x86_64-pc-windows-msvc..."
    cargo xwin build --release --target x86_64-pc-windows-msvc
    print_success "Windows build complete"

    # Package
    mkdir -p "$OUTPUT_DIR"
    cp "$PROJECT_DIR/target/x86_64-pc-windows-msvc/release/ultralog.exe" "$OUTPUT_DIR/ultralog-windows.exe"
    (cd "$OUTPUT_DIR" && zip ultralog-windows.zip ultralog-windows.exe && rm ultralog-windows.exe)
    print_success "Packaged: $OUTPUT_DIR/ultralog-windows.zip"
}

# Install locally (macOS only)
install_local() {
    print_header "Installing UltraLog locally"

    # Detect architecture
    ARCH=$(uname -m)
    if [ "$ARCH" = "arm64" ]; then
        APP_ARCH="arm64"
    else
        APP_ARCH="intel"
    fi

    APP_SRC="$OUTPUT_DIR/$APP_NAME-$APP_ARCH.app"
    APP_DEST="/Applications/$APP_NAME.app"

    # Check if app bundle exists
    if [ ! -d "$APP_SRC" ]; then
        echo "App bundle not found. Building first..."
        build_macos
    fi

    # Remove old installation
    if [ -d "$APP_DEST" ]; then
        echo "Removing old installation..."
        rm -rf "$APP_DEST"
    fi

    # Copy to Applications
    echo "Installing to /Applications..."
    cp -R "$APP_SRC" "$APP_DEST"

    # Remove quarantine attribute
    xattr -cr "$APP_DEST" 2>/dev/null || true

    print_success "Installed: $APP_DEST"
    echo ""
    echo "You can now:"
    echo "  - Open UltraLog from Spotlight (Cmd+Space, type 'UltraLog')"
    echo "  - Open UltraLog from Finder → Applications"
    echo "  - Run from terminal: open -a UltraLog"
}

# Build all platforms
build_all() {
    build_macos
    build_linux
    build_windows

    print_header "Build Complete!"
    echo "Release binaries are in: $OUTPUT_DIR/"
    echo ""
    ls -lh "$OUTPUT_DIR"
}

# Main
case "${1:-all}" in
    --setup)
        setup
        ;;
    --clean)
        clean
        ;;
    --install)
        install_local
        ;;
    macos)
        build_macos
        ;;
    linux)
        build_linux
        ;;
    windows)
        build_windows
        ;;
    all)
        build_all
        ;;
    *)
        echo "Usage: $0 [macos|linux|windows|all|--setup|--clean|--install]"
        exit 1
        ;;
esac
