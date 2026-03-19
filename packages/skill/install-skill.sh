#!/bin/sh
# install-skill.sh — Install the AgentMark skill into local agent systems.
#
# Copies the skill to a canonical shared location (~/.agents/skills/agentmark/)
# and symlinks it into detected agent skill roots (Claude Code, Codex, etc.).
#
# Environment overrides for testing:
#   AGENTMARK_SHARED_SKILLS_DIR — override ~/.agents/skills
#   CLAUDE_SKILLS_DIR           — override ~/.claude/skills
#   CODEX_SKILLS_DIR            — override ~/.codex/skills

set -eu

SKILL_NAME="agentmark"

# --- Resolve source directory (where this script and SKILL.md live) ---

resolve_source_dir() {
    # Get the directory containing this script
    script_dir="$(cd "$(dirname "$0")" && pwd)"
    if [ ! -f "$script_dir/SKILL.md" ]; then
        echo "error: SKILL.md not found in $script_dir" >&2
        exit 1
    fi
    echo "$script_dir"
}

# --- Install canonical shared copy ---

install_canonical_root() {
    shared_skills_dir="${AGENTMARK_SHARED_SKILLS_DIR:-$HOME/.agents/skills}"
    canonical_dir="$shared_skills_dir/$SKILL_NAME"

    # Ensure parent is a directory
    if [ -e "$shared_skills_dir" ] && [ ! -d "$shared_skills_dir" ]; then
        echo "error: $shared_skills_dir exists but is not a directory" >&2
        exit 1
    fi

    mkdir -p "$canonical_dir"

    # Copy skill files into canonical location
    source_dir="$1"
    cp "$source_dir/SKILL.md" "$canonical_dir/SKILL.md"

    echo "$canonical_dir"
}

# --- Detect and link agent roots ---

install_link_for_root() {
    agent_name="$1"
    skills_root="$2"
    canonical_dir="$3"
    target="$skills_root/$SKILL_NAME"

    # Check if skills root parent is usable
    if [ -e "$skills_root" ] && [ ! -d "$skills_root" ]; then
        echo "  $agent_name: SKIPPED ($skills_root is not a directory)"
        return 1
    fi

    if [ ! -d "$skills_root" ]; then
        echo "  $agent_name: SKIPPED ($skills_root does not exist)"
        return 1
    fi

    # Handle existing target
    if [ -L "$target" ]; then
        # Existing symlink — replace it
        rm "$target"
    elif [ -d "$target" ]; then
        # Existing directory — replace it
        rm -rf "$target"
    elif [ -e "$target" ]; then
        # Existing file — error
        echo "  $agent_name: SKIPPED ($target exists and is not a directory or symlink)"
        return 1
    fi

    ln -s "$canonical_dir" "$target"
    echo "  $agent_name: LINKED $target -> $canonical_dir"
    return 0
}

# --- Main ---

main() {
    source_dir="$(resolve_source_dir)"
    echo "AgentMark Skill Installer"
    echo "========================="
    echo ""

    # Step 1: Install canonical copy
    echo "Installing canonical skill..."
    canonical_dir="$(install_canonical_root "$source_dir")"
    echo "  Installed to: $canonical_dir"
    echo ""

    # Step 2: Detect and link agent roots
    claude_skills="${CLAUDE_SKILLS_DIR:-$HOME/.claude/skills}"
    codex_skills="${CODEX_SKILLS_DIR:-$HOME/.codex/skills}"

    echo "Linking to agent skill roots:"

    linked=0
    skipped=0

    if install_link_for_root "Claude Code" "$claude_skills" "$canonical_dir"; then
        linked=$((linked + 1))
    else
        skipped=$((skipped + 1))
    fi

    if install_link_for_root "Codex" "$codex_skills" "$canonical_dir"; then
        linked=$((linked + 1))
    else
        skipped=$((skipped + 1))
    fi

    echo ""

    # Step 3: Summary
    echo "Summary:"
    echo "  Canonical install: $canonical_dir"
    echo "  Agent roots linked: $linked"
    echo "  Agent roots skipped: $skipped"

    if [ "$linked" -eq 0 ]; then
        echo ""
        echo "No agent skill roots were detected."
        echo "You can manually copy or symlink $canonical_dir into your agent's skill directory."
    fi
}

main
