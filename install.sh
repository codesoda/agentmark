#!/bin/sh
# install.sh — AgentMark installer.
#
# Builds the CLI from source, installs the binary, registers the Chrome
# native messaging host, builds/installs the extension, installs the
# cross-agent skill, and runs first-time setup.
#
# Usage:
#   ./install.sh [options]
#   curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash
#
# Options:
#   --skip-init         Skip interactive first-time setup (agentmark init)
#   --skip-extension    Skip extension build/install
#   --extension-id ID   Chrome extension ID for native host registration
#   --help, -h          Show this help message
#
# Environment overrides:
#   AGENTMARK_EXTENSION_ID — Chrome extension ID (alternative to --extension-id)
#   AGENTMARK_HOME         — Override ~/.agentmark install root
#   AGENTMARK_LOCAL_BIN    — Override ~/.local/bin symlink directory

set -eu

# --- Configuration ---

REPO_OWNER="codesoda"
REPO_NAME="agentmark"
REPO_REF="main"
NATIVE_HOST_NAME="com.agentmark.native"

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
  ./install.sh [options]
  curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash

Options:
  --skip-init         Skip interactive first-time setup (agentmark init)
  --skip-extension    Skip extension build/install
  --extension-id ID   Chrome extension ID for native host registration
  --help, -h          Show this help message

Environment overrides:
  AGENTMARK_EXTENSION_ID — Chrome extension ID (alternative to --extension-id)
  AGENTMARK_HOME         — Override ~/.agentmark install root
  AGENTMARK_LOCAL_BIN    — Override ~/.local/bin symlink directory
USAGE
}

# --- Argument parsing ---

SKIP_INIT=0
SKIP_EXTENSION=0
EXTENSION_ID="${AGENTMARK_EXTENSION_ID:-}"

parse_args() {
    while [ $# -gt 0 ]; do
        case "$1" in
            --skip-init)
                SKIP_INIT=1
                ;;
            --skip-extension)
                SKIP_EXTENSION=1
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

cleanup() {
    if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
        rm -rf "$TMP_DIR"
    fi
}

trap cleanup EXIT INT TERM

# --- Global result variables (set by functions, read by main) ---

SOURCE_ROOT=""
BUILT_BINARY=""
INSTALLED_BINARY=""
EXT_INSTALL_DIR=""

# --- Source resolution ---

resolve_source_root() {
    # If invoked from a repo checkout, use it directly
    script_dir="$(cd "$(dirname "$0")" && pwd)"
    if [ -f "$script_dir/Cargo.toml" ] && [ -d "$script_dir/packages/cli" ]; then
        SOURCE_ROOT="$script_dir"
        return 0
    fi

    # Bootstrap mode: download source archive
    if ! command -v curl >/dev/null 2>&1; then
        die "curl is required for bootstrap install (no local source tree found)"
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

# --- Prerequisite checks ---

ensure_prereqs() {
    if ! command -v cargo >/dev/null 2>&1; then
        die "cargo is required (install Rust: https://rustup.rs)"
    fi
    ok "cargo found"

    if [ "$SKIP_EXTENSION" = 0 ]; then
        if ! command -v node >/dev/null 2>&1; then
            die "node is required for extension build (install Node.js: https://nodejs.org)"
        fi
        if ! command -v npm >/dev/null 2>&1; then
            die "npm is required for extension build"
        fi
        ok "node/npm found"
    fi
}

# --- Build CLI ---

build_cli() {
    header "Building CLI"

    if ! (cd "$SOURCE_ROOT" && cargo build --release -p agentmark); then
        die "cargo build failed"
    fi

    BUILT_BINARY="$SOURCE_ROOT/target/release/agentmark"
    if [ ! -f "$BUILT_BINARY" ]; then
        die "Build succeeded but binary not found at $BUILT_BINARY"
    fi

    ok_detail "CLI built" "$BUILT_BINARY"
}

# --- Install binary ---

install_binary() {
    agentmark_home="${AGENTMARK_HOME:-$HOME/.agentmark}"
    bin_dir="$agentmark_home/bin"

    header "Installing binary"

    # Ensure bin dir exists and is a directory
    if [ -e "$bin_dir" ] && [ ! -d "$bin_dir" ]; then
        die "$bin_dir exists but is not a directory"
    fi

    mkdir -p "$bin_dir"

    cp "$BUILT_BINARY" "$bin_dir/agentmark"
    chmod +x "$bin_dir/agentmark"

    INSTALLED_BINARY="$bin_dir/agentmark"
    ok_detail "Binary installed" "$INSTALLED_BINARY"
}

# --- Symlink to ~/.local/bin ---

ensure_local_bin_symlink() {
    local_bin="${AGENTMARK_LOCAL_BIN:-$HOME/.local/bin}"
    symlink_path="$local_bin/agentmark"

    # Create ~/.local/bin if it doesn't exist
    if [ -e "$local_bin" ] && [ ! -d "$local_bin" ]; then
        warn "$local_bin exists but is not a directory — skipping symlink"
        return 1
    fi

    mkdir -p "$local_bin"

    # Handle existing target
    if [ -L "$symlink_path" ]; then
        rm "$symlink_path"
    elif [ -e "$symlink_path" ]; then
        warn "$symlink_path exists and is not a symlink — skipping (remove it manually to fix)"
        return 1
    fi

    ln -s "$INSTALLED_BINARY" "$symlink_path"
    ok_detail "Symlinked" "$symlink_path -> $INSTALLED_BINARY"

    # Check if local bin is on PATH
    case ":${PATH}:" in
        *":${local_bin}:"*)
            ;;
        *)
            warn "$local_bin is not on your PATH — add it to your shell profile"
            ;;
    esac

    return 0
}

# --- Build extension ---

build_extension() {
    agentmark_home="${AGENTMARK_HOME:-$HOME/.agentmark}"
    EXT_INSTALL_DIR="$agentmark_home/extension"

    header "Building extension"

    ext_src="$SOURCE_ROOT/packages/extension"
    if [ ! -f "$ext_src/package.json" ]; then
        die "Extension source not found at $ext_src"
    fi

    if ! (cd "$ext_src" && npm ci); then
        die "npm ci failed in $ext_src"
    fi

    if ! (cd "$ext_src" && npm run build); then
        die "npm run build failed in $ext_src"
    fi

    if [ ! -d "$ext_src/dist" ]; then
        die "Extension build succeeded but dist/ not found"
    fi

    # Copy to durable install location
    if [ -e "$EXT_INSTALL_DIR" ] && [ ! -d "$EXT_INSTALL_DIR" ]; then
        die "$EXT_INSTALL_DIR exists but is not a directory"
    fi

    rm -rf "$EXT_INSTALL_DIR"
    mkdir -p "$EXT_INSTALL_DIR"
    cp -R "$ext_src/dist/." "$EXT_INSTALL_DIR/"

    ok_detail "Extension installed" "$EXT_INSTALL_DIR"
}

# --- Native host manifest ---

write_native_host_manifest() {
    header "Registering native messaging host"

    # Determine host directory based on OS
    case "$(uname -s)" in
        Darwin)
            host_dir="$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts"
            ;;
        Linux)
            host_dir="$HOME/.config/google-chrome/NativeMessagingHosts"
            ;;
        *)
            warn "Unsupported OS for native host registration — skipping"
            return 1
            ;;
    esac

    manifest_path="$host_dir/$NATIVE_HOST_NAME.json"

    if [ -e "$host_dir" ] && [ ! -d "$host_dir" ]; then
        die "$host_dir exists but is not a directory"
    fi

    mkdir -p "$host_dir"

    if [ -z "$EXTENSION_ID" ]; then
        warn "No extension ID provided — native host manifest not written"
        dim "  After loading the extension in Chrome, find its ID at chrome://extensions"
        dim "  Then rerun: curl -sSL https://raw.githubusercontent.com/$REPO_OWNER/$REPO_NAME/$REPO_REF/install.sh | bash -s -- --extension-id YOUR_EXTENSION_ID --skip-init --skip-extension"
        return 1
    fi

    # Write manifest deterministically
    cat > "$manifest_path" <<MANIFEST_EOF
{
  "name": "$NATIVE_HOST_NAME",
  "description": "AgentMark native messaging host",
  "path": "$INSTALLED_BINARY",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$EXTENSION_ID/"]
}
MANIFEST_EOF

    ok_detail "Native host registered" "$manifest_path"
    return 0
}

# --- Skill installation ---

install_skill() {
    header "Installing agent skill"

    skill_installer="$SOURCE_ROOT/packages/skill/install-skill.sh"
    if [ ! -f "$skill_installer" ]; then
        warn "Skill installer not found at $skill_installer — skipping"
        return 1
    fi

    if ! sh "$skill_installer"; then
        warn "Skill installation failed — other components still installed"
        return 1
    fi

    ok "Agent skill installed"
    return 0
}

# --- First-time setup ---

run_first_time_setup() {
    header "First-time setup"

    if ! "$INSTALLED_BINARY" init; then
        warn "agentmark init did not complete — you can run it later"
        return 1
    fi

    ok "Setup complete"
    return 0
}

# --- Summary ---

print_summary() {
    host_registered="$1"
    skill_installed="$2"
    init_ran="$3"

    header "Summary"

    ok_detail "CLI binary" "$INSTALLED_BINARY"

    if [ -n "$EXT_INSTALL_DIR" ]; then
        ok_detail "Extension" "$EXT_INSTALL_DIR"
    else
        dim "  Extension: skipped"
    fi

    if [ "$host_registered" = 1 ]; then
        ok "Native messaging host registered"
    else
        warn "Native messaging host not registered (rerun with --extension-id)"
    fi

    if [ "$skill_installed" = 1 ]; then
        ok "Agent skill installed"
    else
        warn "Agent skill not installed"
    fi

    if [ "$init_ran" = 1 ]; then
        ok "Configuration initialized"
    elif [ "$SKIP_INIT" = 1 ]; then
        dim "  Init: skipped (run 'agentmark init' when ready)"
    else
        warn "Init did not complete (run 'agentmark init' when ready)"
    fi

    printf '\n'

    if [ -n "$EXT_INSTALL_DIR" ] && [ "$host_registered" != 1 ]; then
        info "Next steps:"
        dim "  1. Open Chrome and go to chrome://extensions"
        dim "  2. Enable Developer mode and click 'Load unpacked'"
        dim "  3. Select: $EXT_INSTALL_DIR"
        dim "  4. Copy the extension ID shown on the card"
        dim "  5. Rerun: curl -sSL https://raw.githubusercontent.com/$REPO_OWNER/$REPO_NAME/$REPO_REF/install.sh | bash -s -- --extension-id YOUR_ID --skip-init --skip-extension"
        printf '\n'
    elif [ -n "$EXT_INSTALL_DIR" ]; then
        info "Extension ready:"
        dim "  Load unpacked from: $EXT_INSTALL_DIR"
        printf '\n'
    fi

    printf '%b%b  Done!%b\n\n' "$C_BOLD" "$C_OK" "$C_RESET"
}

# --- Main ---

main() {
    parse_args "$@"

    printf '\n%b%bAgentMark Installer%b\n' "$C_BOLD" "$C_HEADER" "$C_RESET"
    dim "━━━━━━━━━━━━━━━━━━━"
    printf '\n'

    # Step 1: Resolve source
    resolve_source_root
    ok_detail "Source tree" "$SOURCE_ROOT"

    # Step 2: Check prerequisites
    header "Checking prerequisites"
    ensure_prereqs

    # Step 3: Build CLI
    build_cli

    # Step 4: Install binary
    install_binary

    # Step 5: Symlink
    ensure_local_bin_symlink || true

    # Step 6: Build extension
    if [ "$SKIP_EXTENSION" = 0 ]; then
        build_extension
    else
        dim "Skipping extension build (--skip-extension)"
    fi

    # Step 7: Native host manifest
    host_registered=0
    if write_native_host_manifest; then
        host_registered=1
    fi

    # Step 8: Skill installation
    skill_installed=0
    if install_skill; then
        skill_installed=1
    fi

    # Step 9: First-time setup
    init_ran=0
    if [ "$SKIP_INIT" = 0 ]; then
        if run_first_time_setup; then
            init_ran=1
        fi
    else
        dim "Skipping init (--skip-init)"
    fi

    # Step 10: Summary
    print_summary "$host_registered" "$skill_installed" "$init_ran"
}

main "$@"
