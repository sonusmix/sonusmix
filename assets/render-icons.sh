#!/usr/bin/sh

INPUT_SVG="sonusmix.svg"
APP_ID="org.sonusmix.Sonusmix"
ICON_SIZES="512 256 128 64 48 32 24 16"

rm -r icons
mkdir -p icons/hicolor/scalable
cp "$INPUT_SVG" "icons/hicolor/scalable/$APP_ID.svg"
for SIZE in $ICON_SIZES; do
    ICON_DIR="icons/hicolor/${SIZE}x${SIZE}/apps"
    mkdir -p "$ICON_DIR"
    resvg -w "$SIZE" -h "$SIZE" "$INPUT_SVG" "$ICON_DIR/$APP_ID.png"
done
