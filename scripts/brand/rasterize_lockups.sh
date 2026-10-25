#!/usr/bin/env bash
# Rasterize the export lockups from design/logos/ (needs rsvg-convert: `brew install librsvg`).
# Exports embed these PNGs; there is no SVG renderer in the app.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/assets/brand"
mkdir -p "$OUT"
for name in ultralog-lockup ultralog-lockup-paper; do
    rsvg-convert --height 96 --keep-aspect-ratio \
        "$ROOT/design/logos/$name.svg" -o "$OUT/$name.png"
done
ls -l "$OUT"
