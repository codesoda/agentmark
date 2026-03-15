# AgentMark

Agent-first bookmarking for local AI workflows. Save web pages into structured, durable content bundles on your local filesystem — immediately useful to you, Claude Code, Codex, and other local agent tools.

## Install

```sh
curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash
```

This downloads a pre-built binary, installs the Chrome extension, and sets up the agent skill. After install, run the interactive setup:

```sh
agentmark init
```

## What it does

When you save a URL, AgentMark:

1. Fetches the page and extracts article content
2. Creates a local **bundle** with the article markdown, metadata, and raw HTML
3. Indexes it in a local SQLite database with full-text search
4. Optionally enriches it via your local AI agent (Claude or Codex) — generating a summary, suggested tags, and structured metadata

Everything stays on your machine. No cloud accounts, no sync, no API keys for basic usage.

## CLI Usage

### Save a bookmark

```sh
agentmark save https://example.com/article
agentmark save https://example.com/article --tags rust,async --collection dev-reading
agentmark save https://example.com/article --note "Good intro to error handling" --action "summarize key patterns"
```

### Browse and search

```sh
agentmark list
agentmark list --collection dev-reading --state inbox
agentmark list --tag rust --limit 50

agentmark search "error handling patterns"
agentmark search "async runtime" --collection dev-reading

agentmark show am_01ABC123          # summary view
agentmark show am_01ABC123 --full   # includes extracted article text
```

### Organize

```sh
agentmark tag am_01ABC123 rust async        # add tags
agentmark tag am_01ABC123 --remove draft    # remove tags
agentmark collections                        # list all collections
agentmark open am_01ABC123                   # open in browser
```

### Reprocess

```sh
agentmark reprocess am_01ABC123    # re-extract and re-enrich one bookmark
agentmark reprocess --all          # reprocess everything
```

## Chrome Extension

The extension provides two interfaces for saving and managing bookmarks directly from Chrome.

### Quick Save (Popup)

Press **Cmd+Shift+S** (Mac) or **Ctrl+Shift+S** to save the current page. The popup confirms the save and closes automatically. Click "More options" before saving to add tags, a collection, a note, or an action prompt.

If you select text on the page before saving, it's captured alongside the bookmark.

### Side Panel

Press **Cmd+Shift+B** (Mac) or **Ctrl+Shift+B** to open the side panel. From here you can:

- Browse saved bookmarks filtered by state (Inbox, Processed, Archived)
- Click into a bookmark to see its details, summary, and suggested tags
- Edit tags, notes, and collections inline
- Accept or reject AI-suggested tags
- Move bookmarks through states: Inbox → Processed → Archived

### Extension Setup

After installing, load the extension in Chrome:

1. Go to `chrome://extensions`
2. Enable **Developer mode**
3. Click **Load unpacked** and select `~/.agentmark/extension`
4. Copy the extension ID shown on the card
5. Register the native messaging host:

```sh
agentmark install-extension --extension-id YOUR_EXTENSION_ID
```

## Agent Integration

AgentMark includes a skill definition that lets AI agents (Claude Code, Codex) save, search, and manage bookmarks on your behalf. The skill is installed automatically to `~/.agents/skills/agentmark/` and symlinked into detected agent roots.

If an agent tries to use AgentMark and it's not installed, the skill includes bootstrap instructions so the agent can install it via `curl | bash` without user intervention.

### What agents can do

- Save URLs you mention in conversation
- Search your bookmarks for relevant context
- Read extracted article content from saved pages
- Tag and organize bookmarks based on conversation context
- Triage your inbox by moving bookmarks through states

## Configuration

Config lives at `~/.agentmark/config.toml`:

```toml
default_agent = "claude"
storage_path = "/path/to/bookmarks"

# Optional: customize how the agent enriches bookmarks
# system_prompt = """
# Focus on technical content. Tag with programming languages mentioned.
# """

[enrichment]
enabled = true
```

- `default_agent` — `"claude"` or `"codex"` (used for auto-enrichment)
- `storage_path` — where bookmark bundles are stored
- `system_prompt` — optional custom instructions for enrichment
- `enrichment.enabled` — auto-enrich on save (default: `true`)

## Bundle Structure

Each bookmark is stored as a directory under your storage path:

```
bookmarks/2026/03/16/my-article-am_01ABC123/
  bookmark.md       # YAML front-matter + AI summary + suggested tags
  article.md        # Extracted article in markdown
  metadata.json     # OpenGraph and structured metadata
  source.html       # Raw HTML
  events.jsonl      # Event log (saved, enriched, reprocessed, etc.)
```

These bundles are plain files — grep them, read them in your editor, or point your agents at them directly.
