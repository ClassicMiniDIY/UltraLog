#!/usr/bin/env bash
# Build assets/icons/ from the app icon master design/logos/ultralog-app-icon-1024.png
# (needs ImageMagick: `brew install imagemagick`). The paths stay fixed: main.rs embeds the PNGs,
# build.rs embeds windows.ico, and release.yml builds the .icns from mac.png.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="$ROOT/design/logos/ultralog-app-icon-1024.png"
OUT="$ROOT/assets/icons"
mkdir -p "$OUT"

# macOS: Apple's icon grid puts an 824 px tile in a 1024 canvas, so the Dock icon matches the
# size of other apps. The master is full-bleed, so scale it into that tile.
magick "$SRC" -resize 824x824 -background none -gravity center -extent 1024x1024 \
    -strip "$OUT/mac.png"

# Windows and Linux use the tile full-bleed.
magick "$SRC" -strip "$OUT/windows.png"
magick "$SRC" -strip "$OUT/linux.png"
magick "$SRC" -define icon:auto-resize=256,128,64,48,32,24,16 "$OUT/windows.ico"

ls -l "$OUT"
