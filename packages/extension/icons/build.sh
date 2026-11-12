#!/bin/sh
# Render the SVG icon sources into the PNGs the extension ships in public/icons/.
# The SVGs stay out of public/ so they are not copied into dist/.
# Needs rsvg-convert (librsvg). Run after editing any SVG here; commit the PNGs.
set -eu

src_dir=$(cd "$(dirname "$0")" && pwd)
out_dir="$src_dir/../public/icons"
mkdir -p "$out_dir"

if ! command -v rsvg-convert >/dev/null 2>&1; then
  echo "rsvg-convert not found (install librsvg)" >&2
  exit 1
fi

# Extension-listing and notification icon.
for size in 16 32 48 128; do
  rsvg-convert -w "$size" -h "$size" "$src_dir/icon.svg" -o "$out_dir/icon-$size.png"
done

# Per-tab toolbar looks.
for look in idle saving saved-fresh saved error; do
  for size in 16 32 48; do
    rsvg-convert -w "$size" -h "$size" "$src_dir/toolbar-$look.svg" -o "$out_dir/toolbar-$look-$size.png"
  done
done
