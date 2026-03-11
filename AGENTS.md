# AgentMark

## Repository Structure

- `packages/cli/` — Rust CLI (`agentmark` binary), built with `cargo build`
- `packages/extension/` — Chrome MV3 extension (React + Vite + Tailwind + TypeScript)

## Rust CLI

- Workspace root `Cargo.toml` with single member `packages/cli`
- Lib/bin split: `src/lib.rs` exposes `cli`, `commands`, `config` modules; `src/main.rs` is a thin wrapper
- Config lives at `~/.agentmark/config.toml`, index DB at `~/.agentmark/index.db`
- Commands are in `src/commands/` module tree (e.g., `src/commands/init.rs`)
- Command handlers return `Result<()>` — `main.rs` converts errors to stderr + non-zero exit
- Run checks: `cargo fmt --check && cargo clippy -- -D warnings && cargo build && cargo test`
- Tests use `tempfile` for temp HOME dirs and `assert_cmd` for binary integration tests

## Chrome Extension

- Package-local npm project (no root `package.json` or workspace)
- All commands run from `packages/extension/`:
  - `npm ci` — install dependencies
  - `npm run typecheck` — TypeScript strict checking via `tsc --noEmit`
  - `npm run build` — Vite multi-entry build producing `dist/`
  - `npm run lint` — ESLint with flat config
- `manifest.json` is static (root of `packages/extension/`), copied into `dist/` by `vite-plugin-static-copy`
- Build outputs three entries: popup HTML, sidepanel HTML, background service worker (`background.js`)
- Internal seams: `src/background/`, `src/popup/`, `src/sidepanel/`, `src/shared/`
- Shared constants and types live in `src/shared/` — popup and sidepanel import from shared, never from each other
- Background service worker is the sole bridge to native messaging (Specs 17-19)

## CI

- GitHub Actions workflow: `.github/workflows/ci.yml`
- Two jobs: `rust` (fmt, clippy, build, test) and `extension` (npm ci, typecheck, build, lint)
- Extension job auto-skips if `packages/extension/package.json` does not exist
