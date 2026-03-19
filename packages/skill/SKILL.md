---
name: agentmark
description: Use when the user mentions bookmarks, saved pages, or wants to save, search, tag, or manage web content via the `agentmark` CLI
---

AgentMark is the user's bookmarking system. When the user says "bookmarks", "saved pages", "what did I save", or anything about their bookmarks, they mean AgentMark. Always use the `agentmark` CLI commands below to find, save, and manage bookmarks.

<required>
- The `agentmark` CLI must be installed and on PATH
- Run `agentmark init` if `~/.agentmark/config.toml` does not exist
</required>

## When to Use This Skill

Activate when the user mentions:
- Saving a URL or web page for later
- Searching saved bookmarks or content
- Managing bookmarks (tagging, organizing, triaging)
- Reading or referencing previously saved articles
- Asking "what did I bookmark", "my bookmarks", "last saved", etc.

## Bootstrap

Verify `agentmark` is on PATH: `command -v agentmark >/dev/null 2>&1`

If not found: `curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash`

## Configuration

Config lives at `~/.agentmark/config.toml`. Key settings: `default_agent` (claude/codex), `storage_path` (default: `~/agentmark`), `system_prompt` (custom enrichment prompt), `enrichment.enabled` (default: true). Index DB at `~/.agentmark/index.db`.

## CLI Commands

`agentmark init` — interactive setup for agent choice and storage path.

`agentmark save URL` — save a URL. Flags: `--tags TAGS` (comma-separated), `--collection NAME`, `--note TEXT`, `--action TEXT`, `--no-enrich`.

`agentmark list` — list bookmarks. Flags: `--collection NAME`, `--tag TAG`, `--state STATE` (inbox/processed/archived), `--limit N` (default: 20).

`agentmark show ID` — show bookmark details. Flag: `--full` for extracted article content.

`agentmark search QUERY` — full-text search. Flags: `--collection NAME`, `--limit N`.

`agentmark tag ID TAGS...` — add tags. Flag: `--remove` to remove instead.

`agentmark collections` — list all collections with counts.

`agentmark open ID` — open bookmark URL in default browser.

`agentmark reprocess ID` — re-extract and re-enrich. Flag: `--all` for all bookmarks.

## Bookmark Fields

Key fields: `user_tags`, `suggested_tags`, `collections`, `note`, `state` (inbox/processed/archived), `capture_source` (cli/chrome_extension), `content_status` (pending/extracted/failed), `summary_status` (pending/done/failed), `content_hash`.

## Bundle Structure

Each bookmark is stored as a content bundle on disk:

```
<storage_path>/<YYYY>/<MM>/<DD>/<slug>-<id>/
```

Contains: `bookmark.md` (metadata + summary), `article.md` (extracted content), `metadata.json`, `source.html`, `events.jsonl`.

To read bookmark content, read `bookmark.md` for metadata/summary or `article.md` for the full article. Use `agentmark show ID` to find the bundle path.

## Extending with system_prompt

Set `system_prompt` in config.toml to customize enrichment:

```toml
system_prompt = """
Focus on technical depth. Always include code examples in summaries.
"""
```
