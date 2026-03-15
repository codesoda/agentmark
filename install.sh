#!/bin/sh
# install.sh — AgentMark installer.
#
# Downloads a pre-built binary from GitHub Releases (or builds from source
# when run from a repo checkout), installs it, extracts the embedded Chrome
# extension, installs the cross-agent skill, and runs first-time setup.
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash
#   ./install.sh [options]       # from a repo checkout — builds from source
#
# Options:
#   --skip-init         Skip interactive first-time setup (agentmark init)
#   --from-source       Force build from source even when a release exists
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
  --skip-init         Skip interactive first-time setup (agentmark init)
  --from-source       Force build from source even when a release exists
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
FROM_SOURCE=0
EXTENSION_ID="${AGENTMARK_EXTENSION_ID:-}"

parse_args() {
    while [ $# -gt 0 ]; do
        case "$1" in
            --skip-init)
                SKIP_INIT=1
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

cleanup() {
    if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
        rm -rf "$TMP_DIR"
    fi
}

trap cleanup EXIT INT TERM

# --- Global result variables (set by functions, read by main) ---

INSTALLED_BINARY=""
SOURCE_ROOT=""

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

    # Install to ~/.agentmark/bin
    agentmark_home="${AGENTMARK_HOME:-$HOME/.agentmark}"
    bin_dir="$agentmark_home/bin"

    if [ -e "$bin_dir" ] && [ ! -d "$bin_dir" ]; then
        die "$bin_dir exists but is not a directory"
    fi

    mkdir -p "$bin_dir"
    cp "$extracted_binary" "$bin_dir/agentmark"
    chmod +x "$bin_dir/agentmark"

    INSTALLED_BINARY="$bin_dir/agentmark"
    ok_detail "Binary installed" "$INSTALLED_BINARY"
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

    header "Building CLI"
    if ! (cd "$SOURCE_ROOT" && cargo build --release -p agentmark); then
        die "cargo build failed"
    fi

    built_binary="$SOURCE_ROOT/target/release/agentmark"
    if [ ! -f "$built_binary" ]; then
        die "Build succeeded but binary not found at $built_binary"
    fi

    ok_detail "CLI built" "$built_binary"

    # Install to ~/.agentmark/bin
    agentmark_home="${AGENTMARK_HOME:-$HOME/.agentmark}"
    bin_dir="$agentmark_home/bin"

    if [ -e "$bin_dir" ] && [ ! -d "$bin_dir" ]; then
        die "$bin_dir exists but is not a directory"
    fi

    mkdir -p "$bin_dir"
    cp "$built_binary" "$bin_dir/agentmark"
    chmod +x "$bin_dir/agentmark"

    INSTALLED_BINARY="$bin_dir/agentmark"
    ok_detail "Binary installed" "$INSTALLED_BINARY"
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

# --- Install extension (delegated to CLI) ---

install_extension_via_cli() {
    header "Installing extension"

    ext_args=""
    if [ -n "$EXTENSION_ID" ]; then
        ext_args="--extension-id $EXTENSION_ID"
    fi

    # shellcheck disable=SC2086
    if "$INSTALLED_BINARY" install-extension $ext_args; then
        return 0
    else
        warn "Extension installation had issues"
        return 1
    fi
}

# --- Skill installation ---

install_skill() {
    header "Installing agent skill"

    # If we have a source tree, use the skill installer from it
    if [ -n "$SOURCE_ROOT" ] && [ -f "$SOURCE_ROOT/packages/skill/install-skill.sh" ]; then
        if ! sh "$SOURCE_ROOT/packages/skill/install-skill.sh"; then
            warn "Skill installation failed — other components still installed"
            return 1
        fi
        ok "Agent skill installed"
        return 0
    fi

    # For release installs, download the skill installer
    if command -v curl >/dev/null 2>&1; then
        skill_url="https://raw.githubusercontent.com/$REPO_OWNER/$REPO_NAME/$REPO_REF/packages/skill/install-skill.sh"
        if curl -sSL "$skill_url" | sh; then
            ok "Agent skill installed"
            return 0
        fi
    fi

    warn "Skill installation skipped — install manually from the repo"
    return 1
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
    ext_installed="$1"
    skill_installed="$2"
    init_ran="$3"

    agentmark_home="${AGENTMARK_HOME:-$HOME/.agentmark}"

    header "Summary"

    ok_detail "CLI binary" "$INSTALLED_BINARY"

    if [ "$ext_installed" = 1 ]; then
        ok_detail "Extension" "$agentmark_home/extension"
    else
        warn "Extension not installed"
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
    printf '%b%b  Done!%b\n\n' "$C_BOLD" "$C_OK" "$C_RESET"
}

# --- Main ---

main() {
    parse_args "$@"

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

    # Symlink
    ensure_local_bin_symlink || true

    # Install extension (embedded in CLI binary) + native host
    ext_installed=0
    if install_extension_via_cli; then
        ext_installed=1
    fi

    # Skill installation
    skill_installed=0
    if install_skill; then
        skill_installed=1
    fi

    # First-time setup
    init_ran=0
    if [ "$SKIP_INIT" = 0 ]; then
        if run_first_time_setup; then
            init_ran=1
        fi
    else
        dim "Skipping init (--skip-init)"
    fi

    # Summary
    print_summary "$ext_installed" "$skill_installed" "$init_ran"
}

main "$@"
