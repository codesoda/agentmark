#!/bin/sh
# Build the desktop app bundle (.app and .dmg) with the CLI sidecar inside.
#
#   sh scripts/build-app.sh [--debug] [--target TRIPLE] [-- extra tauri build args]
#
# The sidecar is added by src-tauri/tauri.bundle.conf.json, so plain
# `cargo build -p agentmark-app` never needs or copies it.
# Output: target/[<triple>/]<profile>/bundle/{macos,dmg}/.
set -eu

app_dir=$(cd "$(dirname "$0")/.." && pwd)
sidecar_args=""
tauri_args=""
while [ $# -gt 0 ]; do
  case "$1" in
    --debug) sidecar_args="$sidecar_args --debug"; tauri_args="$tauri_args --debug" ;;
    --target) sidecar_args="$sidecar_args --target $2"; tauri_args="$tauri_args --target $2"; shift ;;
    --) shift; break ;;
    -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
    *) echo "build-app: unknown option $1" >&2; exit 2 ;;
  esac
  shift
done

# shellcheck disable=SC2086 # word splitting of the collected flags is intended
sh "$app_dir/scripts/build-sidecar.sh" $sidecar_args
cd "$app_dir"
[ -d node_modules ] || npm ci
# shellcheck disable=SC2086
npx --no-install tauri build $tauri_args --config src-tauri/tauri.bundle.conf.json "$@"
