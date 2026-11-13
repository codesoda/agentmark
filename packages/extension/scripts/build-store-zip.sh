#!/bin/sh
# Build the Chrome Web Store upload zip from packages/extension/dist.
#
# Usage: scripts/build-store-zip.sh [--no-build]
#   (or: npm run build:store-zip [-- --no-build])
#
# Steps:
#   1. Fail if the versions in manifest.json, package.json and
#      packages/cli/Cargo.toml disagree.
#   2. Run `npm run build` (skip with --no-build; then dist/ must exist and be
#      newer than every source file, or the script fails).
#   3. Copy dist/ to a staging folder and remove what must not ship:
#      the manifest "key" field, source maps, and dotfiles.
#   4. Check the staged manifest version and that every file it names exists.
#   5. Write target/chrome-web-store/agentmark-extension-<version>.zip
#      (target/ is git-ignored) and print its path and size.
#
# POSIX sh. Needs node, npm and zip.
set -eu

die() {
  printf 'build-store-zip: %s\n' "$*" >&2
  exit 1
}

build=1
for arg in "$@"; do
  case "$arg" in
    --no-build) build=0 ;;
    -h|--help)
      sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) die "unknown argument: $arg" ;;
  esac
done

ext_dir=$(cd "$(dirname "$0")/.." && pwd)
repo_root=$(cd "$ext_dir/../.." && pwd)
cargo_toml="$repo_root/packages/cli/Cargo.toml"

command -v node >/dev/null 2>&1 || die "node is required"
command -v zip >/dev/null 2>&1 || die "zip is required"
[ -f "$cargo_toml" ] || die "missing $cargo_toml"

# --- 1. Versions must agree ---

json_version() {
  node -e 'process.stdout.write(String(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).version ?? ""))' "$1"
}

manifest_version=$(json_version "$ext_dir/manifest.json")
package_version=$(json_version "$ext_dir/package.json")
# First `version = "..."` inside the [package] table.
cargo_version=$(awk '
  /^\[/ { in_pkg = ($0 == "[package]") }
  in_pkg && /^version[ \t]*=/ {
    sub(/^version[ \t]*=[ \t]*"/, ""); sub(/".*$/, ""); print; exit
  }
' "$cargo_toml")

[ -n "$manifest_version" ] || die "no version in manifest.json"
if [ "$manifest_version" != "$package_version" ] || [ "$manifest_version" != "$cargo_version" ]; then
  die "version mismatch: manifest.json=$manifest_version package.json=$package_version packages/cli/Cargo.toml=$cargo_version"
fi
version=$manifest_version

# --- 2. Build, or check dist/ is current ---

dist="$ext_dir/dist"
if [ "$build" -eq 1 ]; then
  command -v npm >/dev/null 2>&1 || die "npm is required"
  printf 'build-store-zip: running npm run build\n' >&2
  (cd "$ext_dir" && npm run build >&2) || die "npm run build failed"
else
  [ -f "$dist/manifest.json" ] || die "dist/ is missing; run npm run build (or drop --no-build)"
  newer=$(find "$ext_dir/src" "$ext_dir/public" "$ext_dir/manifest.json" "$ext_dir/package.json" \
    -type f -newer "$dist/manifest.json" 2>/dev/null | head -n 1)
  [ -z "$newer" ] || die "dist/ is stale ($newer is newer); run npm run build (or drop --no-build)"
fi
[ -f "$dist/manifest.json" ] || die "build produced no dist/manifest.json"

# --- 3. Stage and strip ---

stage=$(mktemp -d "${TMPDIR:-/tmp}/agentmark-store.XXXXXX")
trap 'rm -rf "$stage"' EXIT INT TERM
cp -R "$dist/." "$stage/"

find "$stage" -name '.*' ! -path "$stage" -exec rm -rf {} + 2>/dev/null || true
find "$stage" -type f -name '*.map' -exec rm -f {} +

node - "$stage/manifest.json" <<'EOF'
const fs = require("fs");
const file = process.argv[2];
const manifest = JSON.parse(fs.readFileSync(file, "utf8"));
if ("key" in manifest) {
  delete manifest.key;
  process.stderr.write("build-store-zip: removed \"key\" from manifest.json\n");
}
fs.writeFileSync(file, JSON.stringify(manifest, null, 2) + "\n");
EOF

# --- 4. Check the staged package ---

staged_version=$(json_version "$stage/manifest.json")
[ "$staged_version" = "$version" ] || die "dist/manifest.json has version $staged_version, expected $version; rebuild"

node - "$stage" <<'EOF' || die "manifest names files that are not in dist/"
const fs = require("fs");
const path = require("path");
const root = process.argv[2];
const m = JSON.parse(fs.readFileSync(path.join(root, "manifest.json"), "utf8"));
const files = [];
const icons = (obj) => Object.values(obj || {}).forEach((f) => files.push(f));
icons(m.icons);
icons(m.action && m.action.default_icon);
if (m.action && m.action.default_popup) files.push(m.action.default_popup);
if (m.side_panel && m.side_panel.default_path) files.push(m.side_panel.default_path);
if (m.background && m.background.service_worker) files.push(m.background.service_worker);
for (const cs of m.content_scripts || []) {
  (cs.js || []).forEach((f) => files.push(f));
  (cs.css || []).forEach((f) => files.push(f));
}
const missing = files.filter((f) => !fs.existsSync(path.join(root, f)));
if (missing.length) {
  process.stderr.write("missing: " + missing.join(", ") + "\n");
  process.exit(1);
}
EOF

# --- 5. Zip ---

out_dir="$repo_root/target/chrome-web-store"
mkdir -p "$out_dir"
zip_path="$out_dir/agentmark-extension-$version.zip"
rm -f "$zip_path"
(cd "$stage" && zip -q -r -X "$zip_path" .) || die "zip failed"

bytes=$(wc -c < "$zip_path" | tr -d ' ')
files=$(cd "$stage" && find . -type f | wc -l | tr -d ' ')
printf '%s\n' "$zip_path"
printf 'version %s, %s files, %s bytes\n' "$version" "$files" "$bytes"
