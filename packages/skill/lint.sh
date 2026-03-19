#!/bin/sh
set -eu

# Lint SKILL.md files using nori-lint.
# Works locally and in CI.
#
# Set ANTHROPIC_API_KEY to enable LLM rules.
# Without it, only static rules run.

# Source .env.local if present (local API key)
if [ -f ".env.local" ]; then
  set -a
  . ./.env.local
  set +a
fi

LINT_DIR="${1:-packages/skill}"
DISABLED_RULES="unclosed_tags,first_person,unexplained_url,cli_command_index,process_not_integration"

TMP_CONFIG="$(mktemp "${TMPDIR:-/tmp}/nori-lint-config.XXXXXX")"
trap 'rm -f "$TMP_CONFIG"' EXIT INT TERM

DISABLED_JSON="$(echo "$DISABLED_RULES" | sed 's/,/","/g')"

if [ -n "${ANTHROPIC_API_KEY:-}" ]; then
  printf '{"anthropic_api_key":"%s","rules":{"disabled":["%s"]}}\n' \
    "$ANTHROPIC_API_KEY" "$DISABLED_JSON" > "$TMP_CONFIG"
else
  printf '{"rules":{"disabled":["%s"]}}\n' "$DISABLED_JSON" > "$TMP_CONFIG"
fi

if command -v nori-lint >/dev/null 2>&1; then
  nori-lint lint "$LINT_DIR" --config "$TMP_CONFIG"
else
  npx nori-lint lint "$LINT_DIR" --config "$TMP_CONFIG"
fi
