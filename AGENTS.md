# AgentMark

## Repository Structure

- `packages/cli/` — Rust CLI (`agentmark` binary), built with `cargo build`
- `packages/extension/` — Chrome MV3 extension (React + Vite + Tailwind + TypeScript)

## Rust CLI

- Workspace root `Cargo.toml` with single member `packages/cli`
- Lib/bin split: `src/lib.rs` exposes `agent`, `bundle`, `canonical`, `cli`, `commands`, `config`, `db`, `display`, `enrich`, `extract`, `fetch`, `models` modules; `src/main.rs` is a thin wrapper
- Agent layer lives in `src/agent/` (`mod.rs`, `provider.rs`, `prompt.rs`, `claude.rs`, `codex.rs`) — LLM provider abstraction for enrichment via local CLI subprocesses. Uses an injected `ProcessRunner` trait for testable subprocess invocation. Providers are selected by `create_provider()` factory based on `config.default_agent`. Tests use mock runners rather than real CLIs or PATH mutation
- Canonical layer lives in `src/canonical.rs` — pure URL normalization (strip tracking params, normalize host/scheme/slashes, sort query params), depends only on `url` crate
- Domain model types live in `src/models/` (`bookmark.rs`, `event.rs`) — pure data + serde, no I/O or config coupling
- Database layer lives in `src/db/` (`mod.rs`, `schema.rs`, `repository.rs`) — SQLite + FTS5 index, depends on `models` only
- Display layer lives in `src/display.rs` — shared terminal display helpers for list/show/search output (truncation, tag merging, width detection, color capability, list row formatting, show detail formatting). Pure functions returning strings; commands print
- Bundle layer lives in `src/bundle/` (`mod.rs`, `bookmark_md.rs`, `writer.rs`) — filesystem bundle creation + in-place updates + bookmark.md rendering + event append + read helpers (`read_article_md`, `read_body_sections`), depends on `models` + `fetch::PageMetadata`. `Bundle::find` locates existing bundles by `saved_at` + `id` suffix; update helpers preserve body sections during front-matter changes
- Extract layer lives in `src/extract/` (`mod.rs`, `readability.rs`, `to_markdown.rs`) — article extraction + markdown conversion + content hashing, depends only on readability/scraper/sha2
- Fetch layer lives in `src/fetch/` (`mod.rs`, `metadata.rs`) — HTTP fetch + metadata extraction, depends only on reqwest/scraper/url
- Config lives at `~/.agentmark/config.toml`, index DB at `~/.agentmark/index.db`
- DB layer accepts explicit paths/connections; `config.rs` remains the only HOME-aware module
- Commands are in `src/commands/` module tree (`init.rs`, `save.rs`, `list.rs`, `show.rs`, `search.rs`, `tag.rs`, `collections.rs`, `open.rs`, `reprocess.rs`)
- Command handlers return `Result<()>` — `main.rs` converts errors to stderr + non-zero exit
- Save command (`commands/save.rs`) is the integration boundary for canonical → fetch → extract → bundle → DB; uses typed `SaveError`/`SaveOutcome`/`DedupResult` with two-stage canonical dedup (pre-fetch + post-fetch), three-way branching (new/unchanged/changed), merge semantics for user-owned fields, and partial-save semantics (bundle preserved if DB update fails)
- Run checks: `cargo fmt --check && cargo clippy -- -D warnings && cargo build && cargo test`
- Tests use `tempfile` for temp HOME dirs and `assert_cmd` for binary integration tests

## Chrome Extension

- Package-local npm project (no root `package.json` or workspace)
- All commands run from `packages/extension/`:
  - `npm ci` — install dependencies
  - `npm run typecheck` — TypeScript strict checking via `tsc --noEmit`
  - `npm run build` — Vite multi-entry build producing `dist/`
  - `npm run lint` — ESLint with flat config
  - `npm run test` — Vitest unit tests (Node environment with Chrome API mocks)
- `manifest.json` is static (root of `packages/extension/`), copied into `dist/` by `vite-plugin-static-copy`
- Build outputs three entries: popup HTML, sidepanel HTML, background service worker (`background.js`)
- Internal seams: `src/background/`, `src/popup/`, `src/sidepanel/`, `src/shared/`
- Shared constants and types live in `src/shared/` — popup and sidepanel import from shared, never from each other
- Background service worker (`src/background/service-worker.ts`) is the sole bridge to native messaging — thin MV3 wiring that delegates to `src/shared/native-messaging.ts` for port lifecycle
- Native messaging client (`src/shared/native-messaging.ts`) owns `connectNative()`, FIFO response matching, reconnect policy, and connection status — no other module should touch the native port directly
- Wire-contract types (`src/shared/types.ts`) mirror the Rust `native::messages` schema exactly (snake_case fields, same discriminator tags) — do not introduce camelCase alternatives
- Test setup (`src/test/chrome-mock.ts`) provides lightweight Chrome API mocks for vitest — use `resetChromeMock()` in `beforeEach` and `createMockPort()` for port lifecycle tests

## CI

- GitHub Actions workflow: `.github/workflows/ci.yml`
- Two jobs: `rust` (fmt, clippy, build, test) and `extension` (npm ci, typecheck, build, lint, test)
- CI jobs run on `ubuntu-latest`, so macOS-specific CLI behaviors need an injectable or overrideable seam for tests instead of assuming local macOS binaries exist in CI
- Extension job auto-skips if `packages/extension/package.json` does not exist
