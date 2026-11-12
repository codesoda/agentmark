#!/bin/sh
# Build the `agentmark` CLI and place it where Tauri's `externalBin` expects
# it: src-tauri/binaries/agentmark-<target triple>. Used by build-app.sh, CI
# and the release workflow.
#
#   sh scripts/build-sidecar.sh [--debug] [--target TRIPLE]
#
# The CLI embeds the Chrome extension's dist/ at compile time, so the
# extension is built first. Build the sidecar in the same profile and target
# as the app: Tauri copies it back to target/<profile>/agentmark, the CLI's own
# output path, so a matching build keeps that file identical.
set -eu

profile=release
target=""
while [ $# -gt 0 ]; do
  case "$1" in
    --debug) profile=debug ;;
    --target) target="$2"; shift ;;
    --target=*) target="${1#--target=}" ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "build-sidecar: unknown option $1" >&2; exit 2 ;;
  esac
  shift
done

app_dir=$(cd "$(dirname "$0")/.." && pwd)
repo=$(cd "$app_dir/../.." && pwd)

# Without --target, build for the host into target/<profile>/, where a plain
# `tauri build` looks; the file name still needs the host triple.
out_dir="$repo/target/$profile"
if [ -n "$target" ]; then
  out_dir="$repo/target/$target/$profile"
  set -- --target "$target"
else
  target=$(rustc --print host-tuple 2>/dev/null || rustc -Vv | sed -n 's/^host: //p')
  set --
fi

echo "==> Building the Chrome extension (embedded in the CLI)"
(
  cd "$repo/packages/extension"
  [ -d node_modules ] || npm ci
  npm run build
)

echo "==> Building agentmark ($profile, $target)"
[ "$profile" = release ] && set -- "$@" --release
(cd "$repo" && cargo build -p agentmark "$@")

src="$out_dir/agentmark"
dest="$app_dir/src-tauri/binaries/agentmark-$target"
case "$target" in *windows*) src="$src.exe"; dest="$dest.exe" ;; esac
mkdir -p "$(dirname "$dest")"
cp "$src" "$dest"
chmod 755 "$dest"
echo "==> Sidecar ready: $dest ($("$dest" --version 2>/dev/null || echo 'cross-built'))"
