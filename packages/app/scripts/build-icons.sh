#!/bin/sh
# Render the app icons from the extension's icon source. Do not redraw the
# mark: packages/extension/icons/icon.svg is the one source.
# The macOS icon grid puts the artwork in the middle 824 of 1024 pixels, so
# the SVG is rendered at 824 and centred on a transparent 1024 canvas, then
# `tauri icon` writes every size and format Tauri needs into src-tauri/icons/.
# Needs rsvg-convert (librsvg), ImageMagick and the Tauri CLI. Commit the output.
set -eu

app_dir=$(cd "$(dirname "$0")/.." && pwd)
src="$app_dir/../extension/icons/icon.svg"
out="$app_dir/src-tauri/icons"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

rsvg-convert -w 824 -h 824 "$src" -o "$tmp/art.png"
magick -size 1024x1024 xc:none "$tmp/art.png" -gravity center -composite "$tmp/app-icon.png"
cd "$app_dir"
npx --no-install tauri icon "$tmp/app-icon.png" -o "$out"
# Desktop-only app: drop the mobile and Windows Store sets.
rm -rf "$out/android" "$out/ios" "$out"/Square*Logo.png "$out/StoreLogo.png"
