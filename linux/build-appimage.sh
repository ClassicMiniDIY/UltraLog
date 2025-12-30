#!/bin/bash
# Build AppImage for UltraLog
# This script is run in CI to create the AppImage

set -e

VERSION="${1:-$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')}"
ARCH="${2:-x86_64}"

echo "Building UltraLog AppImage v${VERSION} for ${ARCH}"

# Create AppDir structure
APPDIR="UltraLog.AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/share/applications"
mkdir -p "$APPDIR/usr/share/icons/hicolor/256x256/apps"
mkdir -p "$APPDIR/usr/share/icons/hicolor/512x512/apps"

# Copy binary
cp "target/x86_64-unknown-linux-gnu/release/ultralog" "$APPDIR/usr/bin/"
chmod +x "$APPDIR/usr/bin/ultralog"

# Copy desktop file
cp "linux/ultralog.desktop" "$APPDIR/usr/share/applications/"
cp "linux/ultralog.desktop" "$APPDIR/"

# Copy icon (multiple sizes for better display)
cp "assets/icons/linux.png" "$APPDIR/usr/share/icons/hicolor/512x512/apps/ultralog.png"
cp "assets/icons/linux.png" "$APPDIR/ultralog.png"

# Create AppRun script
cat > "$APPDIR/AppRun" << 'EOF'
#!/bin/bash
SELF=$(readlink -f "$0")
HERE=${SELF%/*}
export PATH="${HERE}/usr/bin:${PATH}"
exec "${HERE}/usr/bin/ultralog" "$@"
EOF
chmod +x "$APPDIR/AppRun"

# Download appimagetool if not present
if [ ! -f "appimagetool-x86_64.AppImage" ]; then
    echo "Downloading appimagetool..."
    wget -q "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage"
    chmod +x appimagetool-x86_64.AppImage
fi

# Build AppImage
export ARCH="$ARCH"
./appimagetool-x86_64.AppImage --no-appstream "$APPDIR" "UltraLog-${VERSION}-${ARCH}.AppImage"

echo "Created UltraLog-${VERSION}-${ARCH}.AppImage"
