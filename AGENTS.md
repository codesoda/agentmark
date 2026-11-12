# AgentMark

## Versioning

- Every change that is built and installed gets a version bump, at least the patch number, so the running build is identifiable (`agentmark --version`, `chrome://extensions`)
- The CLI embeds the extension, so they share one version. Bump all of these together: `packages/cli/Cargo.toml`, `packages/extension/manifest.json`, `packages/extension/package.json`, and the two root entries in `packages/extension/package-lock.json` (`Cargo.lock` follows on the next build). Tests fail if the CLI, manifest and package versions disagree

## Repository Structure

- `packages/cli/` — Rust CLI (`agentmark` binary), built with `cargo build`
- `packages/extension/` — Chrome MV3 extension (React + Vite + Tailwind + TypeScript)
- `packages/skill/` — Cross-agent skill definition and installer

## Rust CLI

- Workspace root `Cargo.toml` with single member `packages/cli`
- Lib/bin split: `src/lib.rs` exposes `agent`, `bundle`, `canonical`, `cli`, `commands`, `config`, `db`, `display`, `enrich`, `extension`, `extract`, `fetch`, `models` modules; `src/main.rs` is a thin wrapper
- Extension embedding: `src/extension.rs` uses `include_dir!` to embed the pre-built Chrome extension `dist/` at compile time. `build.rs` ensures the `dist/` directory exists (creating an empty placeholder if needed) so compilation succeeds even without a prior extension build. The `install-extension` command extracts embedded files and registers the native messaging host
- Agent layer lives in `src/agent/` (`mod.rs`, `provider.rs`, `prompt.rs`, `claude.rs`, `codex.rs`, `pi.rs`) — LLM provider abstraction for enrichment (`enrich`) and page-to-markdown rendering (`render_article`) via local CLI subprocesses. `RealRunner` appends well-known user bin dirs to `PATH` because Chrome starts the native host with a bare system `PATH`. Uses an injected `ProcessRunner` trait for testable subprocess invocation. Providers are selected by `create_provider()` factory based on `config.default_agent`. Tests use mock runners rather than real CLIs or PATH mutation
- Canonical layer lives in `src/canonical.rs` — pure URL normalization (strip tracking params, normalize host/scheme/slashes, sort query params), depends only on `url` crate
- Domain model types live in `src/models/` (`bookmark.rs`, `event.rs`) — pure data + serde, no I/O or config coupling
- Database layer lives in `src/db/` (`mod.rs`, `schema.rs`, `repository.rs`) — SQLite + FTS5 index, depends on `models` and (for the v2 backfill only) the pure `canonical` module
- `bookmark_urls` (schema v2) maps every canonical URL a bookmark has been seen under to its id. `insert`/`update` record the bookmark's own `canonical_url`, `save.rs` adds the requested URL's canonical, and a trigger deletes rows with the bookmark. The native `lookup_urls` message reads it (`saved_urls`) to tell the extension which URLs are saved. Dedup (`get_by_canonical_url`) does not consult it
- Display layer lives in `src/display.rs` — shared terminal display helpers for list/show/search output (truncation, tag merging, width detection, color capability, list row formatting, show detail formatting). Pure functions returning strings; commands print
- Bundle layer lives in `src/bundle/` (`mod.rs`, `bookmark_md.rs`, `writer.rs`) — filesystem bundle creation + in-place updates + bookmark.md rendering + event append + read helpers (`read_article_md`, `read_body_sections`), depends on `models` + `fetch::PageMetadata`. `Bundle::find` locates existing bundles by `saved_at` + `id` suffix; update helpers preserve body sections during front-matter changes
- Extract layer lives in `src/extract/` (`mod.rs`, `readability.rs`, `to_markdown.rs`, `simplify.rs`) — article extraction + markdown conversion + content hashing, plus `simplify_html` (compact page skeleton for model input), depends only on readability/scraper/sha2
- Fetch layer lives in `src/fetch/` (`mod.rs`, `metadata.rs`) — HTTP fetch + metadata extraction, depends only on reqwest/scraper/url
- Config lives at `~/.agentmark/config.toml`, index DB at `~/.agentmark/index.db`
- DB layer accepts explicit paths/connections; `config.rs` remains the only HOME-aware module
- Commands are in `src/commands/` module tree (`init.rs`, `save.rs`, `list.rs`, `show.rs`, `search.rs`, `tag.rs`, `collections.rs`, `open.rs`, `reprocess.rs`, `delete.rs`, `install_extension.rs`)
- Shared detail/update helper lives in `src/commands/bookmark_detail.rs` — canonical DB+bundle detail loading and typed update application used by both `show.rs` (CLI) and `native_host.rs` (extension). Do not duplicate detail assembly logic in command handlers
- Browser snapshots: the extension sends the live tab's HTML as `page_html`. For URLs matching `src/snapshot_rules.rs` (built-in LinkedIn profile/post/company rules, X/Twitter site-wide, Hacker News discussions, plus `snapshot_sites` in config) the snapshot is the content and the CLI's fetch is kept as a supplement in the bundle's `fetched.html` and as the preferred metadata source. For all other URLs `save.rs` uses the snapshot only when the CLI's own fetch fails (login walls, bot blocking). Do not add per-site HTML extractors — site-specific pages are rendered to `article.md` by the agent
- Site-specific *sources* live in the CLI, not the extension: the extension captures every page the same way, and `save.rs` decides by URL how to get the content. `src/youtube.rs` captures video pages through the optional `yt-dlp` CLI (metadata, chapters, captions rendered as the article) and falls back to a normal page save when it is missing or fails; it reuses the agent layer's `ProcessRunner` seam, so tests use a fake runner
- Native-host saves set `defer_enrichment` and never wait on a model: the host starts the hidden `agentmark post-process <id> [--render-article]` command (`commands/post_process.rs`) as a detached process, which renders `article.md` from `source.html` for snapshot saves and then runs enrichment. `content_hash` stays the hash of the captured page so re-saving the same page is `Unchanged`
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
- Build outputs: popup HTML, sidepanel HTML, background service worker (`background.js`), and content scripts under `content/`
- Internal seams: `src/background/`, `src/popup/`, `src/sidepanel/`, `src/shared/`, `src/content/`
- Content scripts (`src/content/`) add a per-item save button on LinkedIn, X and Hacker News. `save-button.ts` is the site-agnostic core (button, states, snapshot, hand-off); each site is a `SiteAdapter` (`linkedin-posts.ts`, `x-posts.ts`, `hn-posts.ts`) chosen by hostname in `posts.entry.ts`. A click sends `open_save_popup`; the service worker stashes the save and opens the toolbar popup, which takes it via `take_pending_save` and saves it like a page (falling back to a direct save if the popup cannot open), then reports the result to the tab. After each injection pass, `saved-lookup.ts` sends one batched `lookup_urls` message (≤200 URLs each) for unchecked buttons and caches answers and successful saves per page; only buttons still idle are repainted (plain saved look), and every lookup failure is silent. `linkedin-urn.ts` (MAIN world, LinkedIn only) reads the post URN from LinkedIn's React data, the only place a timeline post's ID exists. Each `*.entry.ts` must bundle to one self-contained classic script: do not import `src/shared/` modules from content scripts or share a module between the two entries (Rollup would emit a shared chunk), and keep the constants duplicated between the two bundles in sync (a test checks them). Content scripts must never click or navigate the page
- Icons: SVG sources live in `packages/extension/icons/` (outside `public/`, so they do not ship); `icons/build.sh` (needs `rsvg-convert`) renders them into `public/icons/`. `icon-{16,32,48,128}.png` (from `icon.svg`) is the listing and notification icon (`manifest.icons`, `iconUrl`); `toolbar-<look>-{16,32,48}.png` are the per-tab toolbar looks: idle (grey, plus), saving (hollow light blue), saved-fresh (brand blue, tick), saved (brand blue, plain), error (red, "!"). Blue only ever means saved. All reuse the shapes in `save-button.ts`; do not redraw them. The in-page badge draws the same mark inline in `src/content/save-button.ts`, one look per state: solid with a plus (idle), hollow light blue (saving), solid brand blue with a tick for `SAVED_TICK_MS` then plain (saved), red with "!" (error). The bookmark shape is a licensed Noun Project icon (favorite #8112368)
- Shared constants and types live in `src/shared/` — popup and sidepanel import from shared, never from each other
- Background service worker (`src/background/service-worker.ts`) is the sole bridge to native messaging — thin MV3 wiring that delegates to `src/shared/native-messaging.ts` for port lifecycle
- Toolbar icon (`src/background/toolbar-icon.ts`) paints each tab idle or saved from a `lookup_urls` request on every navigation/tab switch, and shows a save's progress only when the tab's current URL is exactly the saved URL (an in-page post save leaves it alone). Answers are dropped if the tab has moved on (`tabs.get`), and refreshes never override a save's look. Its in-memory state is only an optimisation; it must work after a cold worker start. Needs the `tabs` permission
- Native messaging client (`src/shared/native-messaging.ts`) owns `connectNative()`, FIFO response matching, reconnect policy, and connection status — no other module should touch the native port directly
- Wire-contract types (`src/shared/types.ts`) mirror the Rust `native::messages` schema exactly (snake_case fields, same discriminator tags) — do not introduce camelCase alternatives
- Popup owns toolbar action click (`_execute_action` command) — sidepanel open is additive via action-context menu item and named `open_side_panel` command. Do not use `openPanelOnActionClick` or it will regress the popup
- Side panel (`src/sidepanel/`) uses list/detail view state without a router — `SidePanel.tsx` is the stateful container, `BookmarkList.tsx`, `BookmarkCard.tsx`, `BookmarkDetail.tsx`, `TagManager.tsx`, and `EditableField.tsx` are presentational
- Native list requests use a narrow `BookmarkSummary` DTO (DB-only, no bundle reads) — keep list payloads bounded and filter server-side
- Native detail requests use a `BookmarkDetail` DTO (DB + bundle summary, no article body) — keep detail payloads bounded
- Shared form controls (`src/shared/TagInput.tsx`, `src/shared/CollectionSelect.tsx`) are used by both popup and sidepanel — popup re-exports from shared. Do not copy form controls into sidepanel or popup; edit the shared originals
- CLI-unreachable onboarding is shared: `src/shared/connection.ts` (`classifyConnectionError` maps Chrome's error strings to `not_installed` / `forbidden` / `worker_unreachable` / `host_crashed`; `INSTALL_COMMAND` and `SETUP_GUIDE_URL` live here) and `src/shared/InstallPrompt.tsx` (install command + register command + reload for `not_installed`, register + reload for `forbidden`, reload only for `worker_unreachable`, reload then register for `host_crashed`; Try again re-runs the caller's request). The popup shows it in place of "Save Failed" for all but `host_crashed` and retries the same save target; the side panel shows it in place of the list
- Test setup (`src/test/chrome-mock.ts`) provides lightweight Chrome API mocks for vitest — use `resetChromeMock()` in `beforeEach` and `createMockPort()` for port lifecycle tests

## Cross-Agent Skill

- `packages/skill/SKILL.md` — skill prompt (source of truth for agent loaders)
- `packages/skill/install-skill.sh` — installs canonical copy to `~/.agents/skills/agentmark/` and symlinks into detected agent roots (`~/.claude/skills/`, `~/.codex/skills/`)
- Environment overrides for testing: `AGENTMARK_SHARED_SKILLS_DIR`, `CLAUDE_SKILLS_DIR`, `CODEX_SKILLS_DIR`
## Installer

- `install.sh` — Root installer script (POSIX shell). Supports both repo-local and bootstrap (`curl | bash`) execution
- Builds CLI via `cargo build --release`, installs binary to `~/.agentmark/bin/agentmark`, symlinks to `~/.local/bin/agentmark`
- Extension installation and native host registration are delegated to `agentmark install-extension` — the extension is embedded in the CLI binary at compile time, so no Node.js is needed at install time
- Delegates skill installation to `packages/skill/install-skill.sh` — do not duplicate agent-root detection
- Flags: `--skip-init`, `--extension-id ID`
- Env overrides: `AGENTMARK_EXTENSION_ID`, `AGENTMARK_HOME`, `AGENTMARK_LOCAL_BIN`
- Uses global variables for inter-function communication (not stdout capture) since display functions write to stdout
- Tests in `packages/cli/tests/install_script_tests.rs` use temp repo fixtures with fake cargo shims

## CI

- GitHub Actions workflow: `.github/workflows/ci.yml`
- Two jobs: `rust` (extension build, fmt, clippy, build, test) and `extension` (npm ci, typecheck, build, lint, test)
- A `web` job runs the website's checks (pnpm install, typecheck, lint, build)
- The `rust` job builds the extension first so `include_dir!` embeds real files into the CLI binary
- CI jobs run on `ubuntu-latest`, so macOS-specific CLI behaviors need an injectable or overrideable seam for tests instead of assuming local macOS binaries exist in CI
- Extension job auto-skips if `packages/extension/package.json` does not exist

## Website

- `web/` — marketing site, one static page at `/` plus `llms.txt`, robots, sitemap, icons and OpenGraph image. Deploys to Vercel with root directory `web` (canonical `https://agentmark.vercel.app`)
- Next.js 16 App Router, React 19, TypeScript strict, Tailwind CSS 4, pnpm. No UI kits or animation libraries; icons are inline SVG
- All commands run from `web/`: `pnpm install`, `pnpm dev`, `pnpm typecheck`, `pnpm lint`, `pnpm build`
- Self-contained: nothing in `web/` imports from outside it, and nothing outside imports from it. Every route must stay statically renderable (no API routes, no server-only runtime features)
- Demo slots live in `web/src/components/demos/*-demo.tsx`: default-exported client components with no props, rendered by the page as-is. Replace a demo by editing its file, not the page. Shared frames, the mark, hooks and sample content are in `demos/kit/`; rules and sizes in `demos/README.md`
- Copy must match the repo: check every claim against the README, `install.sh`, `release.yml`, the CLI and the extension. No invented numbers, quotes, users, logos, pricing or features. Update the site (and `public/llms.txt`) when a user-facing claim changes, such as release platforms
- Themes follow `prefers-color-scheme` with no toggle. Both themes are hand-written CSS variables in `web/src/app/globals.css`; use the Tailwind token utilities, never raw colours (the mark is the exception)
- The mark is copied from `packages/extension/icons/` into `web/public/mark/` and `demos/kit/agentmark-mark.tsx`. Do not redraw it; if the icons change, copy them again
- The extension links to `https://agentmark.vercel.app/#install` (`packages/extension/src/shared/connection.ts`), so keep the `#install` anchor
