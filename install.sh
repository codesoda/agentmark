#!/bin/sh
# install.sh — AgentMark installer.
#
# Downloads a pre-built binary from GitHub Releases (or builds from source
# when run from a repo checkout), installs it, registers the native messaging
# host with Chrome, and installs the cross-agent skill. It asks no questions:
# setup (folder, agent, summaries) happens in Chrome or the desktop app.
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash
#   ./install.sh [options]       # from a repo checkout — builds from source
#
# Options:
#   --from-source       Build from source (needs cargo and npm) instead of downloading
#   --extension-id ID   Also allow this (development) extension ID
#   --help, -h          Show this help message
#   --skip-init         Accepted for compatibility; does nothing
#
# Environment overrides:
#   AGENTMARK_EXTENSION_ID — Chrome extension ID (alternative to --extension-id)
#   AGENTMARK_LOCAL_BIN    — Override ~/.local/bin symlink directory
#   AGENTMARK_ALLOW_ROOT   — Set to 1 to install as root (refused otherwise)
#
# The binary always goes to $HOME/.agentmark/bin, the folder the CLI and the
# desktop app use; there is no override for it.

set -eu

# --- Configuration (overridable for forks) ---

REPO_OWNER="${AGENTMARK_REPO_OWNER:-codesoda}"
REPO_NAME="${AGENTMARK_REPO_NAME:-agentmark}"
REPO_REF="${AGENTMARK_REPO_REF:-main}"

# --- Color support ---

if [ -t 1 ] && command -v tput >/dev/null 2>&1 && [ "$(tput colors 2>/dev/null || echo 0)" -ge 8 ]; then
    USE_COLOR=1
else
    USE_COLOR=0
fi

if [ "$USE_COLOR" = 1 ]; then
    C_RESET='\033[0m'
    C_BOLD='\033[1m'
    C_DIM='\033[38;5;249m'
    C_OK='\033[38;5;114m'
    C_WARN='\033[38;5;216m'
    C_ERR='\033[38;5;210m'
    C_HEADER='\033[38;5;141m'
    C_CHECK='\033[38;5;151m'
else
    C_RESET=''
    C_BOLD=''
    C_DIM=''
    C_OK=''
    C_WARN=''
    C_ERR=''
    C_HEADER=''
    C_CHECK=''
fi

# --- Output helpers ---

header() {
    printf '\n%b%b%s%b\n' "$C_BOLD" "$C_HEADER" "$*" "$C_RESET"
    printf '%b%s%b\n' "$C_DIM" "$(echo "$*" | sed 's/./-/g')" "$C_RESET"
}

info() {
    printf '%b%s%b\n' "$C_OK" "$*" "$C_RESET"
}

dim() {
    printf '%b%s%b\n' "$C_DIM" "$*" "$C_RESET"
}

ok() {
    printf '%b✓ %s%b\n' "$C_CHECK" "$*" "$C_RESET"
}

ok_detail() {
    printf '%b✓ %s %b(%s)%b\n' "$C_CHECK" "$1" "$C_DIM" "$2" "$C_RESET"
}

warn() {
    printf '%b! %s%b\n' "$C_WARN" "$*" "$C_RESET" >&2
}

die() {
    printf '%b✗ %s%b\n' "$C_ERR" "$*" "$C_RESET" >&2
    exit 1
}

# --- Usage ---

usage() {
    cat <<'USAGE'
AgentMark Installer

Usage:
  curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash
  ./install.sh [options]

Options:
  --from-source       Build from source (needs cargo and npm) instead of downloading
  --extension-id ID   Also allow this (development) extension ID
  --help, -h          Show this help message
  --skip-init         Accepted for compatibility; does nothing

Environment overrides:
  AGENTMARK_EXTENSION_ID — Chrome extension ID (alternative to --extension-id)
  AGENTMARK_LOCAL_BIN    — Override ~/.local/bin symlink directory
  AGENTMARK_ALLOW_ROOT   — Set to 1 to install as root (refused otherwise)
USAGE
}

# --- Argument parsing ---

FROM_SOURCE=0
EXTENSION_ID="${AGENTMARK_EXTENSION_ID:-}"

parse_args() {
    while [ $# -gt 0 ]; do
        case "$1" in
            --skip-init)
                # Setup no longer runs in the terminal; kept so old commands work.
                ;;
            --from-source)
                FROM_SOURCE=1
                ;;
            --extension-id)
                if [ $# -lt 2 ]; then
                    die "--extension-id requires a value"
                fi
                EXTENSION_ID="$2"
                shift
                ;;
            --help|-h)
                usage
                exit 0
                ;;
            *)
                die "Unknown option: $1 (use --help)"
                ;;
        esac
        shift
    done
}

# --- Cleanup trap ---

TMP_DIR=""
STAGED_BINARY=""

cleanup() {
    if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
        rm -rf "$TMP_DIR"
    fi
    if [ -n "$STAGED_BINARY" ] && [ -f "$STAGED_BINARY" ]; then
        rm -f "$STAGED_BINARY"
    fi
}

trap cleanup EXIT INT TERM

# --- Global result variables (set by functions, read by main) ---

INSTALLED_BINARY=""
SOURCE_ROOT=""
NEEDS_EXTENSION_ID=0

# --- Platform detection ---

detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin)
            case "$arch" in
                arm64)  echo "aarch64-apple-darwin" ;;
                x86_64) echo "x86_64-apple-darwin" ;;
                *)      echo "" ;;
            esac
            ;;
        Linux)
            case "$arch" in
                x86_64)  echo "x86_64-unknown-linux-gnu" ;;
                aarch64) echo "aarch64-unknown-linux-gnu" ;;
                *)       echo "" ;;
            esac
            ;;
        *)
            echo ""
            ;;
    esac
}

# --- Post-install binary security ---
# Ensures a binary is allowed to execute on the host OS. Without these steps
# the binary may be silently blocked (e.g. by macOS Gatekeeper), causing
# Chrome native messaging to time out.

secure_binary() {
    target_file="$1"
    case "$(uname -s)" in
        Darwin)
            # Strip quarantine / provenance extended attributes
            if command -v xattr >/dev/null 2>&1; then
                xattr -d com.apple.quarantine "$target_file" 2>/dev/null || true
                xattr -d com.apple.provenance "$target_file" 2>/dev/null || true
            fi
            # Ad-hoc code sign so Gatekeeper allows execution. A fixed
            # identifier keeps the signature independent of the temp name.
            if command -v codesign >/dev/null 2>&1; then
                codesign --force --sign - --identifier agentmark "$target_file" 2>/dev/null || true
            fi
            ;;
    esac
}

# --- Install a binary to ~/.agentmark/bin ---
# Copies to a temp file in the same folder, secures it, then renames it over
# the old binary. Chrome may be running the old one: a rename leaves its open
# file alone, while copying or signing in place can kill it.

install_binary() {
    source_binary="$1"
    bin_dir="$HOME/.agentmark/bin"

    if [ -e "$bin_dir" ] && [ ! -d "$bin_dir" ]; then
        die "$bin_dir exists but is not a directory"
    fi

    mkdir -p "$bin_dir"
    STAGED_BINARY="$bin_dir/.agentmark.tmp-$$"
    cp "$source_binary" "$STAGED_BINARY"
    chmod 755 "$STAGED_BINARY"
    secure_binary "$STAGED_BINARY"
    mv -f "$STAGED_BINARY" "$bin_dir/agentmark"
    STAGED_BINARY=""

    INSTALLED_BINARY="$bin_dir/agentmark"
    ok_detail "Binary installed" "$INSTALLED_BINARY"
}

# --- Refuse to run as root ---
# Everything goes into the user's home folder. Run as root (often through
# sudo), the files end up owned by root and the user's Chrome cannot use them.

refuse_root() {
    if [ "$(id -u)" = 0 ] && [ "${AGENTMARK_ALLOW_ROOT:-0}" != 1 ]; then
        die "Do not run the installer as root or with sudo. It installs into your own home folder, so run it again as your normal user. (To install for root anyway, set AGENTMARK_ALLOW_ROOT=1.)"
    fi
}

# --- Download pre-built binary from GitHub Releases ---

download_release() {
    header "Downloading pre-built binary"

    if ! command -v curl >/dev/null 2>&1; then
        die "curl is required for downloading releases"
    fi

    target="$(detect_target)"
    if [ -z "$target" ]; then
        die "No pre-built binary available for $(uname -s) $(uname -m). Use --from-source to build locally."
    fi

    # Get the latest release tag
    tag="$(curl -sSL "https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/releases/latest" \
        | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"

    if [ -z "$tag" ]; then
        die "Could not determine latest release. Use --from-source to build locally."
    fi

    info "Latest release: $tag"

    TMP_DIR="$(mktemp -d)"
    archive_name="agentmark-${tag}-${target}.tar.gz"
    download_url="https://github.com/$REPO_OWNER/$REPO_NAME/releases/download/${tag}/${archive_name}"

    dim "Downloading $archive_name..."
    if ! curl -sSL --fail -o "$TMP_DIR/$archive_name" "$download_url"; then
        die "Failed to download $download_url — no binary for this platform in release $tag. Use --from-source to build locally."
    fi

    tar xzf "$TMP_DIR/$archive_name" -C "$TMP_DIR"

    extracted_binary="$TMP_DIR/agentmark-${tag}-${target}/agentmark"
    if [ ! -f "$extracted_binary" ]; then
        die "Archive did not contain expected binary"
    fi

    ok_detail "Downloaded" "$tag ($target)"

    install_binary "$extracted_binary"
}

# --- Source build path (repo checkout or --from-source) ---

resolve_source_root() {
    # If invoked from a repo checkout, use it directly
    script_dir="$(cd "$(dirname "$0")" && pwd)"
    if [ -f "$script_dir/Cargo.toml" ] && [ -d "$script_dir/packages/cli" ]; then
        SOURCE_ROOT="$script_dir"
        return 0
    fi

    # Download source archive
    if ! command -v curl >/dev/null 2>&1; then
        die "curl is required for source install"
    fi

    info "Downloading source from GitHub..."
    TMP_DIR="$(mktemp -d)"
    archive_url="https://github.com/$REPO_OWNER/$REPO_NAME/archive/refs/heads/$REPO_REF.tar.gz"

    if ! curl -sSL "$archive_url" | tar xz -C "$TMP_DIR" 2>/dev/null; then
        die "Failed to download source from $archive_url"
    fi

    extracted="$TMP_DIR/$REPO_NAME-$REPO_REF"
    if [ ! -f "$extracted/Cargo.toml" ]; then
        die "Downloaded archive does not contain expected source tree"
    fi

    SOURCE_ROOT="$extracted"
}

build_from_source() {
    resolve_source_root
    ok_detail "Source tree" "$SOURCE_ROOT"

    header "Checking prerequisites"
    if ! command -v cargo >/dev/null 2>&1; then
        die "cargo is required for source builds (install Rust: https://rustup.rs)"
    fi
    ok "cargo found"
    if ! command -v npm >/dev/null 2>&1; then
        die "npm is required for source builds (install Node.js: https://nodejs.org)"
    fi
    ok "npm found"

    # The CLI embeds packages/extension/dist at compile time, so the
    # extension must be built first or the binary ships without it.
    header "Building extension"
    if ! (cd "$SOURCE_ROOT/packages/extension" && npm ci && npm run build); then
        die "extension build failed"
    fi
    ok "Extension built"

    header "Building CLI"
    if ! (cd "$SOURCE_ROOT" && cargo build --release -p agentmark); then
        die "cargo build failed"
    fi

    built_binary="$SOURCE_ROOT/target/release/agentmark"
    if [ ! -f "$built_binary" ]; then
        die "Build succeeded but binary not found at $built_binary"
    fi

    ok_detail "CLI built" "$built_binary"

    install_binary "$built_binary"
}

# --- Symlink to ~/.local/bin ---

ensure_local_bin_symlink() {
    local_bin="${AGENTMARK_LOCAL_BIN:-$HOME/.local/bin}"
    symlink_path="$local_bin/agentmark"

    if [ -e "$local_bin" ] && [ ! -d "$local_bin" ]; then
        warn "$local_bin exists but is not a directory — skipping symlink"
        return 1
    fi

    mkdir -p "$local_bin"

    if [ -L "$symlink_path" ]; then
        rm "$symlink_path"
    elif [ -e "$symlink_path" ]; then
        warn "$symlink_path exists and is not a symlink — skipping (remove it manually to fix)"
        return 1
    fi

    ln -s "$INSTALLED_BINARY" "$symlink_path"
    ok_detail "Symlinked" "$symlink_path -> $INSTALLED_BINARY"

    case ":${PATH}:" in
        *":${local_bin}:"*)
            ;;
        *)
            warn "$local_bin is not on your PATH — add it to your shell profile"
            ;;
    esac

    return 0
}

# --- Register with Chrome (delegated to CLI) ---

# The CLI prints this when it knows no extension ID: only a build with an
# empty WEB_STORE_EXTENSION_ID (for example a fork with no store item) and no
# --extension-id. Builds of this repo always allow the store ID. Keep in sync
# with NO_EXTENSION_IDS_NOTE in packages/cli/src/commands/install_extension.rs.
NO_IDS_MARKER="No extension IDs are known yet"

install_extension_via_cli() {
    header "Registering with Chrome"

    if [ -n "$EXTENSION_ID" ]; then
        set -- install-extension --extension-id "$EXTENSION_ID"
    else
        set -- install-extension
    fi

    if ! ext_output="$("$INSTALLED_BINARY" "$@")"; then
        if [ -n "$ext_output" ]; then
            printf '%s\n' "$ext_output"
        fi
        warn "Registering the native messaging host failed"
        return 1
    fi

    case "$ext_output" in
        *"$NO_IDS_MARKER"*)
            NEEDS_EXTENSION_ID=1
            ;;
        *)
            if [ -n "$ext_output" ]; then
                dim "$ext_output"
            fi
            ok "Native messaging host registered"
            ;;
    esac
    return 0
}

# --- Development path: unpacked extension ---

print_development_steps() {
    ext_dir="$HOME/.agentmark/extension"

    header "Development: load the extension unpacked"
    dim "  1. Open chrome://extensions"
    dim "  2. Enable Developer mode (top-right toggle)"
    dim "  3. Click 'Load unpacked' and select:"
    dim "     $ext_dir"
    dim "  4. Copy the extension ID shown on the card"

    # Helpers for a person at a terminal on macOS only (never in pipes or tests).
    if [ "$(uname -s)" = "Darwin" ] && [ -t 1 ]; then
        if command -v pbcopy >/dev/null 2>&1; then
            printf '%s' "$ext_dir" | pbcopy
            ok "Extension path copied to clipboard"
        fi
        if command -v open >/dev/null 2>&1; then
            open "chrome://extensions" 2>/dev/null || true
        fi
    fi

    printf '\n'
    info "Then run: agentmark install-extension --extension-id YOUR_EXTENSION_ID"
}

# --- Skill installation (delegated to CLI) ---

install_skill() {
    header "Installing agent skill"

    if "$INSTALLED_BINARY" add-skill; then
        ok "Agent skill installed"
        return 0
    else
        warn "Skill installation failed"
        return 1
    fi
}

# --- Main ---

main() {
    parse_args "$@"
    refuse_root

    printf '\n%b%bAgentMark Installer%b\n' "$C_BOLD" "$C_HEADER" "$C_RESET"
    dim "━━━━━━━━━━━━━━━━━━━"
    printf '\n'

    # Determine install mode:
    # - From a repo checkout → build from source
    # - --from-source flag → build from source
    # - Otherwise → download pre-built release
    script_dir="$(cd "$(dirname "$0")" && pwd)"
    if [ "$FROM_SOURCE" = 1 ] || { [ -f "$script_dir/Cargo.toml" ] && [ -d "$script_dir/packages/cli" ]; }; then
        build_from_source
    else
        download_release
    fi

    ensure_local_bin_symlink || true

    ext_ok=1
    if ! install_extension_via_cli; then
        ext_ok=0
    fi

    # A missing skill only affects agents, not the extension: warn and go on.
    install_skill || true

    printf '\n'
    if [ "$ext_ok" = 0 ]; then
        die "AgentMark is installed at $INSTALLED_BINARY, but Chrome registration failed. Retry with: agentmark install-extension"
    fi

    if [ "$NEEDS_EXTENSION_ID" = 1 ]; then
        print_development_steps
        return 0
    fi

    printf '%b%bDone. Go back to Chrome: AgentMark finishes setting up there.%b\n\n' "$C_BOLD" "$C_OK" "$C_RESET"
    print_unpacked_hint
}

# The development copy carries the manifest key, so it loads with the store ID
# that is already registered: no ID to copy. Shown while the store listing is
# not live; people who already have the extension can ignore it.
print_unpacked_hint() {
    ext_dir="$HOME/.agentmark/extension"
    if [ ! -f "$ext_dir/manifest.json" ]; then
        return 0
    fi
    dim "No AgentMark extension in Chrome yet? Open chrome://extensions, turn on"
    dim "Developer mode, click 'Load unpacked' and select:"
    dim "  $ext_dir"
    printf '\n'
}

main "$@"
