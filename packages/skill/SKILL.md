# AgentMark

AgentMark is a local-first bookmarking system for agent-native workflows. It captures web content into durable, structured content bundles on the local filesystem — making saved pages immediately useful to humans, Claude Code, Codex, and other local agent tools.

## When to Use This Skill

Activate when the user mentions:
- Saving a URL or web page for later
- Searching saved bookmarks or content
- Managing bookmarks (tagging, organizing, triaging)
- Reading or referencing previously saved articles
- Working with their local bookmark collection

## Bootstrap

Before using any command, verify `agentmark` is on `PATH`:

```bash
command -v agentmark >/dev/null 2>&1
```

If not found, install it:

```bash
curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash
```

After installation, run `agentmark init` to set up configuration and storage.

## Configuration

AgentMark stores configuration at `~/.agentmark/config.toml` with these settings:

- `default_agent` — LLM agent for enrichment (`"claude"` or `"codex"`)
- `storage_path` — where bookmark bundles are stored (default: `~/agentmark`)
- `system_prompt` — optional custom prompt appended to enrichment requests; use this to tailor how summaries and tags are generated for your workflow
- `enrichment.enabled` — whether to auto-enrich on save (default: `true`)

The SQLite index lives at `~/.agentmark/index.db`.

## CLI Commands

### `agentmark init`

Initialize configuration and storage. Interactive — prompts for agent choice and storage path.

### `agentmark save <url>`

Save a URL as a bookmark. Fetches the page, extracts article content, creates a content bundle, indexes in SQLite, and optionally enriches via the configured agent.

```bash
# Basic save
agentmark save https://example.com/article

# Save with metadata
agentmark save https://example.com/article \
  --tags "rust,async" \
  --collection "reading-list" \
  --note "Good overview of async patterns" \
  --action "Summarize the key takeaways"

# Save without enrichment
agentmark save https://example.com/article --no-enrich
```

Flags:
- `--tags <tags>` — comma-separated tags to apply
- `--collection <name>` — collection to organize into
- `--note <text>` — note to attach
- `--action <text>` — action intent prompt
- `--no-enrich` — skip auto-enrichment even if enabled in config

### `agentmark list`

List saved bookmarks with optional filters.

```bash
# List recent bookmarks (default: 20)
agentmark list

# Filter by collection
agentmark list --collection "reading-list"

# Filter by tag
agentmark list --tag rust

# Filter by state
agentmark list --state inbox

# Limit results
agentmark list --limit 50
```

Flags:
- `--collection <name>` — filter by collection
- `--tag <tag>` — filter by tag
- `--state <state>` — filter by state: `inbox`, `processed`, or `archived`
- `--limit <n>` — max results (default: 20)

### `agentmark show <id>`

Show details of a specific bookmark.

```bash
# Show bookmark metadata and summary
agentmark show am_01ABC123

# Show full content including extracted article text
agentmark show am_01ABC123 --full
```

Flags:
- `--full` — include extracted article content

### `agentmark search <query>`

Full-text search across bookmark titles, descriptions, URLs, notes, tags, and summaries.

```bash
# Search bookmarks
agentmark search "async rust patterns"

# Search within a collection
agentmark search "error handling" --collection "rust-notes"

# Limit results
agentmark search "typescript" --limit 10
```

Flags:
- `--collection <name>` — restrict search to a collection
- `--limit <n>` — max results (default: 20)

### `agentmark tag <id> <tags...>`

Add or remove tags on a bookmark.

```bash
# Add tags
agentmark tag am_01ABC123 rust async concurrency

# Remove tags
agentmark tag am_01ABC123 --remove outdated draft
```

Flags:
- `--remove <tags...>` — remove the specified tags instead of adding

### `agentmark collections`

List all collections with bookmark counts.

```bash
agentmark collections
```

### `agentmark open <id>`

Open a bookmark's URL in the default browser.

```bash
agentmark open am_01ABC123
```

### `agentmark reprocess <id>`

Re-extract content and re-enrich a bookmark. Useful after content changes or to refresh enrichment with updated config.

```bash
# Reprocess a single bookmark
agentmark reprocess am_01ABC123

# Reprocess all bookmarks
agentmark reprocess --all
```

Flags:
- `--all` — reprocess every bookmark (prompts for confirmation)

## Bookmark Data Model

Each bookmark has these fields:

| Field | Description |
|-------|-------------|
| `id` | Unique identifier (`am_` prefix + ULID) |
| `url` | Original URL |
| `canonical_url` | Normalized URL for deduplication |
| `title` | Page title |
| `description` | Page description / meta description |
| `author` | Article author |
| `site_name` | Site name from OpenGraph |
| `published_at` | Original publication date |
| `saved_at` | When the bookmark was saved |
| `capture_source` | How it was saved: `cli` or `chrome_extension` |
| `user_tags` | Tags added by the user |
| `suggested_tags` | Tags suggested by enrichment |
| `collections` | Collections the bookmark belongs to |
| `note` | User-attached note |
| `action_prompt` | Action intent |
| `state` | Triage state: `inbox`, `processed`, or `archived` |
| `content_status` | Extraction status: `pending`, `extracted`, or `failed` |
| `summary_status` | Enrichment status: `pending`, `done`, or `failed` |
| `content_hash` | SHA-256 hash of extracted content |

## Bundle File Structure

Each bookmark is stored as a content bundle on disk at:

```
<storage_path>/<YYYY>/<MM>/<DD>/<slug>-<id>/
```

For example: `~/agentmark/2026/03/12/async-rust-patterns-am_01ABC123/`

Each bundle contains:

| File | Description |
|------|-------------|
| `bookmark.md` | YAML front-matter with all metadata, plus enriched summary and suggested tags in the body |
| `article.md` | Extracted article content in markdown |
| `metadata.json` | Page metadata (OpenGraph, structured data) |
| `source.html` | Raw HTML of the page |
| `events.jsonl` | Event log (saved, enriched, reprocessed, etc.) |

### Reading Bookmark Content

To read a bookmark's summary and metadata, read `bookmark.md` in its bundle directory. The file has YAML front-matter (between `---` delimiters) followed by body sections.

To read the extracted article content, read `article.md`.

To find a bookmark's bundle path, use `agentmark show <id>` which displays the bundle location, or construct it from `storage_path` + date + slug + id.

## Extending with system_prompt

You can customize enrichment behavior by setting `system_prompt` in `~/.agentmark/config.toml`:

```toml
system_prompt = """
Focus on technical depth. Always include code examples in summaries.
Tag with programming languages and frameworks mentioned.
"""
```

This prompt is appended to enrichment requests, letting you tailor how summaries and tags are generated without modifying the tool itself. The `system_prompt` is a configuration-level setting — it is not stored per-bookmark.
