# PRD: AgentMark v1 — Agent-First Bookmarking for Local AI Workflows

## Introduction

AgentMark is a local-first bookmarking system designed for agent-native users. It captures web content into durable, structured content bundles on the local filesystem — making saved pages immediately useful to humans, Claude Code, Codex, Obsidian, and other local agent workflows.

Traditional bookmarking tools optimize for read-later and discovery. AgentMark treats each saved URL as a reusable, agent-readable filesystem object with automatic enrichment and action dispatch at save time.

**Product thesis:** AgentMark helps agent-native users capture web content into their local knowledge filesystem, enrich it automatically, and trigger downstream agent workflows at save time. "Pocket for the Claude Code / Codex era."

**Core promise:** Save once. Reuse anywhere. Act immediately.

## Goals

- Save web content from browser (Chrome extension) or terminal (CLI) into durable local content bundles
- Auto-enrich at save time (summary, suggested tags) via user-configured agent (Claude or Codex)
- Provide CLI commands for agents and humans to search, manage, and act on bookmarks
- Provide a cross-agent skill (`agentmark`) installable into Claude Code, Codex, and other agent systems
- Use a monorepo layout: `packages/cli/` (Rust) and `packages/extension/` (React + Vite + Tailwind, MV3)
- Store bookmarks as structured folder bundles with SQLite FTS index for fast search

## User Stories

**Definition of Done (applies to all stories):**
- All acceptance criteria met
- Typecheck/lint passes with no warnings
- Tests written and passing
- Code formatted

---

### US-001: Project scaffolding & monorepo setup

**Description:** As a developer, I want a monorepo structure so that the CLI and extension can be developed and built together.

**Acceptance Criteria:**
- [ ] Root `Cargo.toml` workspace with `packages/cli` as a member
- [ ] `packages/cli/` initialized as a Rust binary crate (`agentmark`)
- [ ] `packages/extension/` initialized with Vite + React + Tailwind + TypeScript
- [ ] Chrome Manifest V3 `manifest.json` in extension package
- [ ] `install.sh` script that builds the CLI, installs the binary to `~/.agentmark/bin/agentmark`, symlinks into `~/.local/bin/` for global PATH availability, registers the native messaging host manifest, installs the agent skill, and runs first-time config setup
- [ ] Root-level README with project overview and build instructions
- [ ] `.gitignore` covering Rust targets, `node_modules`, `dist`

**Reference:** The crossbeam POC at `~/projects/obie/crossbeam/` demonstrates this split — `crossbeam-extension/` (React+Vite+Tailwind MV3) and `crossbeam-native/` (Rust native host). Use as architectural reference, not to copy directly. Installer patterns from `~/projects/cadence-cli/install.sh` and `~/projects/vibe-code-audit/install.sh`.

---

### US-002: Config system & first-run setup

**Description:** As a user, I want a first-run setup that configures my default agent, storage location, and system prompt so that AgentMark works out of the box.

**Acceptance Criteria:**
- [ ] `agentmark init` command triggers interactive first-run setup
- [ ] Prompts user to select default enrichment agent: `claude` or `codex`
- [ ] Prompts user for bookmark storage path (defaults to `./bookmarks` relative to current directory)
- [ ] Writes `~/.agentmark/config.toml` with: `default_agent`, `storage_path` (absolute, resolved from user input), `enrichment.enabled`
- [ ] Config supports a `system_prompt` field — a user-defined prefix included in all agent enrichment/action prompts, allowing users to reference their local tools, skills, and capabilities (e.g. native notifications, agent-ui, custom skills)
- [ ] Default config.toml includes comments with examples showing how to declare local tool availability in `system_prompt`
- [ ] Creates directory structure at chosen storage path, creates `~/.agentmark/index.db`
- [ ] If config already exists, warns and asks to overwrite
- [ ] `install.sh` invokes `agentmark init` as final step

---

### US-003: Bookmark data model & SQLite index

**Description:** As a developer, I need a data model for bookmarks and a SQLite index so that bookmarks can be stored durably and searched efficiently.

**Acceptance Criteria:**
- [ ] Rust structs for `Bookmark` covering: id (ULID-based `am_` prefixed), url, canonical_url, title, description, author, site_name, published_at, saved_at, capture_source, user_tags, suggested_tags, collections, note, action_prompt, state (`inbox`/`processed`/`archived`), content_status, summary_status, content_hash, schema_version
- [ ] SQLite schema with FTS5 virtual table indexing: title, description, url, note, tags, summary
- [ ] Functions to insert, update, query, and search bookmarks in the index
- [ ] Index is kept in sync with filesystem bundles (index is derived, files are source of truth)
- [ ] **Design consideration:** Evaluate using hidden subdirectories under the bookmarks path (`.inbox/`, `.processing/`, `.archive/`) for state-based file routing instead of (or in addition to) a `state` field in the index. Decide during implementation which approach best supports both filesystem browsing and agent workflows.

---

### US-004: `agentmark save <url>` — CLI save with fetch & extraction

**Description:** As a user or agent, I want to save a URL from the terminal so that I can capture web content without opening a browser.

**Acceptance Criteria:**
- [ ] `agentmark save <url>` fetches the page via HTTP
- [ ] Extracts metadata: title, description, author, site_name, published_at from Open Graph, meta tags, and schema.org markup
- [ ] Extracts readable article content using a Rust-based extraction approach (e.g. `readability-rs` or `scraper` crate with custom logic)
- [ ] Converts extracted content to markdown
- [ ] Optional flags: `--tags <t1,t2>`, `--collection <name>`, `--note "why I saved this"`, `--action "prompt"`
- [ ] Outputs bookmark ID and storage path on success
- [ ] Returns meaningful error on fetch failure (timeout, 404, etc.)

---

### US-005: Bookmark bundle file generation

**Description:** As a user, I want each bookmark saved as a structured folder bundle so that it's human-readable, agent-readable, and reprocessable.

**Acceptance Criteria:**
- [ ] Creates bundle directory at `<storage_path>/<YYYY>/<MM>/<DD>/<slug>-<id>/`
- [ ] Generates `bookmark.md` with YAML front matter (all structured fields) and markdown body sections: Summary, Why I Saved This, Suggested Next Actions, Related Items
- [ ] Generates `article.md` with extracted readable content
- [ ] Generates `metadata.json` with full metadata snapshot
- [ ] Saves `source.html` with raw fetched HTML
- [ ] Creates `events.jsonl` with initial `saved` event entry (timestamp, source, metadata)
- [ ] Slug derived from title, truncated to reasonable length

---

### US-006: URL canonicalization & deduplication

**Description:** As a user, I want duplicate saves detected and content changes tracked so that I don't end up with redundant bundles and my local content stays fresh.

**Acceptance Criteria:**
- [ ] Canonicalize URLs: strip tracking params (`utm_*`, `fbclid`, etc.), normalize scheme, trailing slashes, www prefix
- [ ] On save, check index for existing bookmark with same canonical URL
- [ ] If duplicate found: fetch fresh content and compare content hash of extracted article against stored hash
- [ ] If content has changed: update `article.md`, `source.html`, `metadata.json`, store previous content hash in `events.jsonl` as a `content_updated` event, trigger re-enrichment
- [ ] If content unchanged: only merge new tags/notes, log `resaved` event
- [ ] Content hash (of extracted article) stored in index for content-based dedup
- [ ] CLI outputs clear message indicating "already saved — updated existing bookmark" or "already saved — content updated, re-enriching" when dedup triggers

---

### US-007: Auto-enrichment pipeline (summary + tags)

**Description:** As a user, I want bookmarks automatically enriched at save time so that they're immediately useful without manual effort.

**Acceptance Criteria:**
- [ ] After save + extraction, automatically invoke configured agent (`claude` or `codex` per `config.toml`)
- [ ] Agent prompt includes: user's `system_prompt` from config (if set), extracted article content, user-provided note, existing user tags
- [ ] Agent generates: summary (2-3 sentences), suggested_tags (3-5), inferred collection (if applicable)
- [ ] Writes enrichment results back into `bookmark.md` front matter (`suggested_tags`, `summary_status`) and body sections
- [ ] Logs `enriched` event to `events.jsonl` with agent used and timestamp
- [ ] If enrichment fails (API error, timeout), save still succeeds — bookmark marked `summary_status: failed`, logged to events
- [ ] `enrichment.enabled = false` in config skips this step entirely

---

### US-008: `agentmark list` and `agentmark show`

**Description:** As a user, I want to browse my bookmarks from the terminal so I can quickly see what I've saved and drill into details.

**Acceptance Criteria:**
- [ ] `agentmark list` shows recent bookmarks in reverse chronological order (default 20)
- [ ] List output: date, title (truncated), tags, state — formatted for terminal readability
- [ ] `agentmark list --collection <name>` filters by collection
- [ ] `agentmark list --tag <tag>` filters by tag (user or suggested)
- [ ] `agentmark list --limit <n>` controls result count
- [ ] `agentmark show <id>` displays full bookmark details: all front matter fields, summary, note, article preview (first ~20 lines)
- [ ] `agentmark show <id> --full` outputs entire article content

---

### US-009: `agentmark search <query>` — full-text search

**Description:** As a user or agent, I want to search across all my bookmarks so I can find relevant saved content by keyword or topic.

**Acceptance Criteria:**
- [ ] `agentmark search <query>` performs FTS5 search across title, description, URL, note, tags, and summary
- [ ] Results ranked by relevance, displayed in same format as `agentmark list`
- [ ] `agentmark search <query> --collection <name>` scopes search to a collection
- [ ] `agentmark search <query> --limit <n>` controls result count
- [ ] Returns meaningful "no results" message when nothing matches
- [ ] Search is fast — should feel instant for up to 10k bookmarks

---

### US-010: `agentmark tag`, `agentmark collections`, `agentmark open`

**Description:** As a user, I want to manage tags and collections and quickly open bookmarks in my browser.

**Acceptance Criteria:**
- [ ] `agentmark tag <id> <tags...>` adds user tags to a bookmark, updates both `bookmark.md` front matter and SQLite index
- [ ] `agentmark tag <id> --remove <tags...>` removes specified tags
- [ ] `agentmark collections` lists all collections with bookmark count per collection
- [ ] `agentmark open <id>` opens the bookmark's URL in the default browser

---

### US-011: `agentmark reprocess <id>`

**Description:** As a user, I want to re-run enrichment on a bookmark so I can get better results after improving my config or when the content has changed.

**Acceptance Criteria:**
- [ ] `agentmark reprocess <id>` re-fetches the page, re-extracts content, and re-runs enrichment pipeline
- [ ] Updates `article.md`, `source.html`, `metadata.json`, and enrichment fields in `bookmark.md`
- [ ] Logs `reprocessed` event to `events.jsonl`
- [ ] `agentmark reprocess --all` reprocesses all bookmarks (with confirmation prompt)
- [ ] Respects current `config.toml` settings (agent, system_prompt) — so reprocessing after config changes uses new settings

---

### US-012: `agentmark native-host` — native messaging host

**Description:** As a developer, I need the CLI to act as a Chrome native messaging host so the extension can communicate with the local system.

**Acceptance Criteria:**
- [ ] `agentmark native-host` runs in stdin/stdout native messaging mode (Chrome's length-prefixed JSON protocol)
- [ ] Accepts messages: `save` (with url, title, tags, note, selected_text, action), `status` (health check)
- [ ] Returns responses: `save_result` (id, path, status), `error` (message)
- [ ] `install.sh` registers the native messaging host manifest (JSON) in the Chrome-expected location (`~/Library/Application Support/Google/Chrome/NativeMessagingHosts/` on macOS)
- [ ] Host manifest points to `~/.agentmark/bin/agentmark` binary with `native-host` subcommand
- [ ] Handles malformed messages gracefully — logs error, returns error response, does not crash

**Reference:** The crossbeam POC native host at `~/projects/obie/crossbeam/crossbeam-native/` implements this exact pattern — Chrome native messaging in Rust via stdin/stdout. Use as architectural reference for the message framing and host registration.

---

### US-013: Chrome extension — quick save popup

**Description:** As a user, I want to save the current page with one click so that capture is as fast as possible.

**Acceptance Criteria:**
- [ ] Clicking the toolbar icon saves the current tab immediately (URL, title, favicon URL)
- [ ] Brief popup confirmation shows: "Saved!" with title, then auto-dismisses after 2 seconds
- [ ] If native host is not connected, popup shows error with link to install instructions
- [ ] Keyboard shortcut triggers same quick save (configurable, default `Ctrl+Shift+S` / `Cmd+Shift+S`)
- [ ] Context menu item "Save to AgentMark" available on right-click

**Reference:** The crossbeam extension POC at `~/projects/obie/crossbeam/crossbeam-extension/` implements a similar popup save flow with React + Vite + Tailwind in MV3. Use as architectural reference for extension structure, build setup, and native messaging wiring.

---

### US-014: Chrome extension — save with intent (tags, note, action)

**Description:** As a user, I want to add tags, a note, and an action prompt when saving so I can capture context and trigger agent workflows.

**Acceptance Criteria:**
- [ ] Long-press or click-and-hold on toolbar icon opens expanded save form (alternatively, a "More" button in the quick save popup)
- [ ] Form fields: tags (comma-separated input), collection (dropdown of existing + freetext), note ("why I saved this"), action prompt (freetext), selected text (pre-filled if text was selected on page)
- [ ] All fields optional — user can fill any combination
- [ ] Submit sends full payload to native host, same save pipeline as CLI
- [ ] Form remembers last-used collection and tags for convenience (stored in extension local storage)

---

### US-015: Chrome extension — side panel for triage

**Description:** As a user, I want a persistent side panel showing recent saves so I can review, edit tags, accept suggested tags, and manage my inbox without leaving the browser.

**Acceptance Criteria:**
- [ ] Side panel opens via toolbar icon context action or keyboard shortcut
- [ ] Shows list of recent bookmarks (pulled from native host) with title, tags, state, save date
- [ ] Clicking a bookmark shows details: summary, user tags, suggested tags, note, collections
- [ ] User can accept/reject suggested tags inline (accepted tags move to user_tags)
- [ ] User can edit note and collection from the side panel
- [ ] Side panel persists across tab navigation (Chrome Side Panel API)
- [ ] Filter by state: inbox / processed / archived

**Reference:** The crossbeam extension POC at `~/projects/obie/crossbeam/crossbeam-extension/` uses React + Tailwind component patterns that can inform the side panel UI structure.

---

### US-016: Cross-agent `agentmark` skill

**Description:** As an agent-native user, I want an `agentmark` skill installable across multiple agent systems so that any local agent can save, search, and act on bookmarks.

**Acceptance Criteria:**
- [ ] Skill installable via `npx skills add agentmark`, `install.sh`, or manual placement
- [ ] `install.sh` installs skill to `~/.agents/skills/agentmark/` and symlinks into detected local agent skill directories (Claude Code, Codex, others)
- [ ] Skill prompt includes instructions for installing the CLI on the fly (`curl | sh` from git repo) if `agentmark` binary is not found on PATH
- [ ] Skill exposes core CLI commands to the agent: `save`, `search`, `list`, `show`, `tag`, `related`, `reprocess`
- [ ] Skill prompt describes AgentMark's capabilities, the bookmark data model, and available commands
- [ ] Agent can save a URL mid-conversation, search bookmarks for context, and reference bookmark content
- [ ] Skill respects user's `system_prompt` from config when composing agent prompts

**Reference:** Installer patterns from `~/projects/cadence-cli/install.sh` and `~/projects/vibe-code-audit/install.sh` for agent detection and symlink approach.

---

## Functional Requirements

- **FR-1:** The system must save a URL as a structured folder bundle on the local filesystem, containing `bookmark.md`, `article.md`, `metadata.json`, `source.html`, and `events.jsonl`.
- **FR-2:** The system must extract readable article content from fetched HTML using a Rust-based extraction approach.
- **FR-3:** The system must maintain a SQLite database with FTS5 index as a derived, searchable index over filesystem bundles.
- **FR-4:** The system must canonicalize URLs (strip tracking params, normalize scheme/www/trailing slashes) and detect duplicates.
- **FR-5:** On duplicate save, the system must compare content hashes and update local content if the source has changed.
- **FR-6:** The system must auto-enrich bookmarks at save time using the configured agent (Claude or Codex), including summary generation and tag suggestion.
- **FR-7:** All agent prompts must include the user's `system_prompt` from `config.toml` if configured.
- **FR-8:** The CLI must support commands: `init`, `save`, `list`, `show`, `search`, `tag`, `collections`, `open`, `reprocess`, `native-host`.
- **FR-9:** The CLI must operate as a Chrome native messaging host via `agentmark native-host`, using Chrome's length-prefixed JSON stdin/stdout protocol.
- **FR-10:** The Chrome extension must support quick save (one-click), save with intent (form), and side panel triage.
- **FR-11:** The Chrome extension must communicate with the CLI exclusively via Chrome native messaging.
- **FR-12:** The `agentmark` skill must be installable into multiple agent systems and capable of bootstrapping the CLI if not already installed.
- **FR-13:** The installer must place the binary at `~/.agentmark/bin/agentmark`, symlink to `~/.local/bin/`, register the native messaging host, install the agent skill, and run `agentmark init`.
- **FR-14:** User tags and suggested tags must be stored separately in the bookmark data model.
- **FR-15:** All bookmark lifecycle events must be logged to `events.jsonl` within the bundle directory.

## Non-Goals (Out of Scope)

- Social bookmarking or cross-user recommendations
- Cloud sync or remote storage
- Team collaboration features
- Full reader app or reading mode
- Mobile app
- Firefox or other browser extensions (Chrome only for v1)
- Import from external services (Pocket, Raindrop, etc.) — stretch goal, not required for v1
- Embedding-based semantic search (SQLite FTS is sufficient for v1)
- Site-specific adapters (generic extraction only for v1)
- Automatic resurfacing / digest notifications

## Design Considerations

- **Monorepo structure:** `packages/cli/` (Rust) and `packages/extension/` (React + Vite + Tailwind CSS, MV3)
- **Bookmark bundle structure:**
  ```
  <storage_path>/<YYYY>/<MM>/<DD>/<slug>-<id>/
    bookmark.md        # Human+agent readable, YAML front matter
    article.md         # Extracted readable content
    metadata.json      # Full metadata snapshot
    source.html        # Raw fetched HTML
    events.jsonl       # Processing event log
  ```
- **bookmark.md front matter schema:**
  ```yaml
  id: am_01HXYZ
  url: "https://example.com/post"
  canonical_url: "https://example.com/post"
  title: "The Future of X"
  description: "..."
  site_name: "Example"
  author: "Jane Doe"
  published_at: "2026-03-01"
  saved_at: "2026-03-11T09:14:00+10:00"
  capture_source: "chrome_extension"  # or "cli"
  user_tags: ["agents", "productivity"]
  suggested_tags: ["knowledge-management"]
  collections: ["product-discovery"]
  note: "Might be useful for our research workflow"
  action_prompt: "compare this to my existing capture system"
  state: "inbox"
  content_status: "extracted"
  summary_status: "done"
  content_hash: "sha256:abc123..."
  schema_version: 1
  ```
- **State-based file routing:** Consider hidden subdirectories (`.inbox/`, `.processing/`, `.archive/`) under the bookmarks path for state-based routing as an alternative to purely index-based state tracking. Evaluate during implementation.

## Technical Considerations

- **Rust crates (CLI):** `clap` (CLI args), `serde` + `serde_json` + `serde_yaml` (serialization), `rusqlite` (SQLite + FTS5), `reqwest` (HTTP), `scraper` or `readability` (HTML extraction), `ulid` (ID generation), `toml` (config)
- **Extension stack:** React 18+, Vite, Tailwind CSS, TypeScript, Chrome Manifest V3, Side Panel API
- **Native messaging:** Chrome's length-prefixed JSON protocol (4-byte little-endian length prefix, then JSON payload). The crossbeam POC at `~/projects/obie/crossbeam/crossbeam-native/` implements this in Rust.
- **Config location:** `~/.agentmark/config.toml` (global config), `~/.agentmark/index.db` (SQLite index), `~/.agentmark/bin/agentmark` (binary)
- **Storage location:** User-selected during `agentmark init`, defaults to `./bookmarks` relative to where installer runs. Stored as absolute path in config.
- **Agent integration:** The `system_prompt` config field provides extensibility — users can declare their local tools (notifications, agent-ui, custom skills) so that enrichment/action prompts are aware of the broader agent ecosystem.

## Success Metrics

- Saves per active user per week (target: 10+)
- Percentage of saves successfully enriched at save time (target: >90%)
- Percentage of saves later reopened, searched, or acted on (target: >30%)
- Average time from click/command to durable save (target: <5 seconds excluding enrichment)
- Duplicate detection accuracy (target: >95% of true duplicates caught)
- User acceptance rate of suggested tags (measures enrichment quality)

## Open Questions

None — all resolved during PRD walkthrough.

## Appendix: POC References

The following existing projects serve as architectural references (not to copy directly):

| Reference | Location | Relevance |
|-----------|----------|-----------|
| Crossbeam Chrome Extension | `~/projects/obie/crossbeam/crossbeam-extension/` | MV3 extension structure, React+Vite+Tailwind build, popup save flow, native messaging wiring |
| Crossbeam Native Host | `~/projects/obie/crossbeam/crossbeam-native/` | Rust native messaging host, Chrome stdin/stdout protocol, host registration |
| Cadence CLI installer | `~/projects/cadence-cli/install.sh` | Installer script pattern, agent detection, skill symlink approach |
| Vibe Code Audit installer | `~/projects/vibe-code-audit/install.sh` | Additional installer pattern reference |
