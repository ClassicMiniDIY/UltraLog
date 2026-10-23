#!/usr/bin/env bash
# Build the static TTFs in assets/fonts/ from the variable fonts in google/fonts.
#
# egui renders only the default instance of a variable font, so every weight the type scale
# uses (design/design-system/README.md → Typography) is instanced to its own static file.
# The outputs are committed; run this only to update or add a weight.
#
# Usage: scripts/fonts/build_static_fonts.sh [SOURCE_DIR]
#   SOURCE_DIR holds the downloaded variable fonts; when omitted they are downloaded to a
#   temporary directory. Needs `uv` (fonttools runs through `uv run --with`).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/assets/fonts"
SRC="${1:-$(mktemp -d)}"
BASE="https://github.com/google/fonts/raw/main/ofl"
FONTTOOLS=(uv run --quiet --no-project --with "fonttools>=4.50,<5" fonttools)

fetch() { # <path under ofl/> <local name>
    [ -f "$SRC/$2" ] || curl -fsSL -o "$SRC/$2" "$BASE/$1"
}

fetch 'saira/Saira%5Bwdth%2Cwght%5D.ttf' Saira-VF.ttf
fetch 'saira/Saira-Italic%5Bwdth%2Cwght%5D.ttf' Saira-Italic-VF.ttf
fetch 'outfit/Outfit%5Bwght%5D.ttf' Outfit-VF.ttf
fetch 'jetbrainsmono/JetBrainsMono%5Bwght%5D.ttf' JetBrainsMono-VF.ttf
fetch 'notosansdevanagari/NotoSansDevanagari%5Bwdth%2Cwght%5D.ttf' NotoSansDevanagari-VF.ttf
fetch 'notosansbengali/NotoSansBengali%5Bwdth%2Cwght%5D.ttf' NotoSansBengali-VF.ttf
fetch 'notosansarabic/NotoSansArabic%5Bwdth%2Cwght%5D.ttf' NotoSansArabic-VF.ttf
fetch 'notosanssc/NotoSansSC%5Bwght%5D.ttf' NotoSansSC-VF.ttf
for family in saira outfit jetbrainsmono notosansdevanagari notosansbengali notosansarabic notosanssc; do
    fetch "$family/OFL.txt" "$family-OFL.txt"
done

mkdir -p "$OUT"
instance() { # <source> <output> <axis=value>...
    local src="$1" out="$2"
    shift 2
    "${FONTTOOLS[@]}" varLib.instancer --quiet "$SRC/$src" "$@" -o "$OUT/$out"
}

# Brand faces (weights from the type scale).
instance Saira-VF.ttf Saira-SemiBold.ttf wght=600 wdth=100
instance Saira-Italic-VF.ttf Saira-BoldItalic.ttf wght=700 wdth=100
instance Saira-Italic-VF.ttf Saira-ExtraBoldItalic.ttf wght=800 wdth=100
instance Outfit-VF.ttf Outfit-Regular.ttf wght=400
instance Outfit-VF.ttf Outfit-Medium.ttf wght=500
instance Outfit-VF.ttf Outfit-SemiBold.ttf wght=600
instance JetBrainsMono-VF.ttf JetBrainsMono-Regular.ttf wght=400
instance JetBrainsMono-VF.ttf JetBrainsMono-Medium.ttf wght=500

# Script fallbacks for the hi, bn, ar and ur locales.
instance NotoSansDevanagari-VF.ttf NotoSansDevanagari-Regular.ttf wght=400 wdth=100
instance NotoSansBengali-VF.ttf NotoSansBengali-Regular.ttf wght=400 wdth=100
instance NotoSansArabic-VF.ttf NotoSansArabic-Regular.ttf wght=400 wdth=100

# The language picker names Chinese and Japanese in their own scripts in every locale. Bundling
# full CJK would add ~10 MB, so ship only those glyphs; the ja / zh-CN UI loads a system CJK font
# at runtime (src/ui/fonts.rs).
instance NotoSansSC-VF.ttf NotoSansSC-tmp.ttf wght=400
"${FONTTOOLS[@]}" subset "$OUT/NotoSansSC-tmp.ttf" --text="简体中文日本語" \
    --layout-features='*' --output-file="$OUT/NotoSansSC-LanguageNames.ttf"
rm "$OUT/NotoSansSC-tmp.ttf"

cp "$SRC/saira-OFL.txt" "$OUT/Saira-OFL.txt"
cp "$SRC/outfit-OFL.txt" "$OUT/Outfit-OFL.txt"
cp "$SRC/jetbrainsmono-OFL.txt" "$OUT/JetBrainsMono-OFL.txt"
cp "$SRC/notosansdevanagari-OFL.txt" "$OUT/NotoSansDevanagari-OFL.txt"
cp "$SRC/notosansbengali-OFL.txt" "$OUT/NotoSansBengali-OFL.txt"
cp "$SRC/notosansarabic-OFL.txt" "$OUT/NotoSansArabic-OFL.txt"
cp "$SRC/notosanssc-OFL.txt" "$OUT/NotoSansSC-OFL.txt"

ls -l "$OUT"
