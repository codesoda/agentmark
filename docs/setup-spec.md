# Spec: extension-first and app-first setup

Working spec for branch `feat/extension-first-setup`. Mockups are in `docs/.mockups/` (read its README first). Where a mockup and this spec disagree, this spec wins. `AGENTS.md` at the repo root is binding for all work.

## Goal

Two front doors that meet in the middle:

- **Chrome Web Store first.** The user adds the extension. When they click it and the app is missing, the extension walks them through installing it, then runs setup itself.
- **Desktop app first.** The user downloads a small Tauri desktop app. It installs the CLI, registers with Chrome, runs setup, and sends them to the Web Store for the extension.

Today install is `curl | bash`, then loading the extension unpacked, then `agentmark install-extension --extension-id …`, then `agentmark init` in a terminal.

## 1. Decisions (do not reopen)

- The extension will ship through the Chrome Web Store. A store extension has one fixed ID for everyone. That ID does not exist yet.
- `tabs` becomes an optional permission, off by default, requested from a Settings switch. Install must not show "Read your browsing history".
- Save buttons on LinkedIn, X, Hacker News and Reddit stay on by default (declared content scripts).
- In the platform list the detected platform is always the top row, set apart from the others.
- Setup happens in the extension or the app, never in a terminal. `agentmark init` keeps working.
- Blue only ever means saved.
- Reuse the existing icon shapes. Do not redraw them. The bookmark shape is a licensed design.
- No per-site HTML extractors. Content scripts never click, focus, scroll or navigate the host page.
- Copy must match the code. No invented numbers, testimonials or features. The repo has no licence, so nothing may say "open source".

Defaults chosen for questions the owner left open:

- While the popup shows "Waiting for the app…", it re-checks every 2 seconds, only while open. "Check again" also exists.
- Default bookmark folder: `~/Documents/AgentMark` (not `./bookmarks`).
- Changing the folder later does not move existing bookmarks. The UI says so.
- When the app is missing, the side panel shows the same install component as the popup.
- After setup finishes in the popup, it saves the page that was being saved ("Finish and save this page").

## 2. Architecture and layout

- `packages/cli/` stays where it is. It must stay buildable and usable without the app.
- `packages/app/` is new: a Tauri v2 desktop app. Rust side in `packages/app/src-tauri`, a second member of the root Cargo workspace. Frontend in `packages/app/src` (React + Vite + TypeScript + Tailwind, like the extension).
- The CLI sits outside the app folder. The app uses it two ways:
  1. As a Rust library dependency (the `agentmark` crate already has a lib/bin split) for setup and settings.
  2. As a bundled sidecar binary (`externalBin`), because Chrome needs a real executable for the native messaging host and users need `agentmark` on their PATH.
- **One setup core, three callers.** Setup and settings logic lives in a CLI library module (for example `packages/cli/src/setup/`). The native host exposes it as messages. The Tauri app calls it directly. `agentmark init` calls it too.

### Contract requirements (exact shapes live in `docs/setup-contract.md`)

- **Read setup state.** Whether the app is set up; current settings; the suggested default folder (absolute, plus a `~`-shortened display form); for each agent (`claude`, `codex`, `pi`) whether its CLI is found (reuse the existing agent PATH search); app version; platform; bookmark count.
- **Write settings.** For first-run setup and Settings. Fields: storage folder, default agent, summaries on/off (`enrichment.enabled`), summary instructions (`system_prompt`), extra snapshot sites (`snapshot_sites`). Validates, creates the folder, creates and migrates the index. Returns the new state or field-level errors. Preserves unrelated keys and comments in an existing `config.toml`.
- **Choose a folder.** The CLI opens the system folder dialog and returns the path. OS-specific, behind an injectable seam (CI is Linux). When no dialog is available it returns a clear "not available" so the UI falls back to a typed path.
- **Not set up is a state, not an error.** The extension must tell apart: app missing, app present but not set up, app ready. Requests that need setup return a typed "setup required" result.
- **Liveness.** The native host records a small state file under `~/.agentmark/` on each connection (time, extension ID, extension version), written atomically. The app reads it to know the extension has connected.
- **Extension IDs.** One place defines the allowed extension IDs for the native host manifest: the Web Store ID (a constant, empty until assigned) plus development IDs. `install-extension` with no ID registers the store ID if set. `--extension-id` adds a development ID without removing the store one.

## 3. CLI

- Implement the setup module and the native messages, following existing message patterns (snake_case, serde tag, the allow-list in `from_value`, exhaustive dispatch).
- `agentmark init` uses the setup core; default folder `~/Documents/AgentMark`; add a non-interactive form (flags or JSON on stdin).
- Native host manifest registration: fixed store ID as above. Chrome on macOS and Linux as today; also Chromium, Brave, Edge and Arc where installed, if the paths are confirmed. The registered host path must survive the app being moved or updated (decide in the contract: app bundle sidecar vs a stable copy under `~/.agentmark/bin/`).
- `install.sh`: with the store ID set, the one-line install needs no ID and no follow-up command, asks no questions when piped, and ends by telling the user to go back to Chrome. Fix `--from-source` (it currently builds a CLI with no extension embedded). Keep the unpacked-extension copy as a labelled development path; do not remove the embed without checking `build.rs`, the release workflow and install tests.
- Small fixes: `agentmark delete` must not print "Deleted" when the row vanished between read and delete. `cargo clippy --all-targets -- -D warnings` fails on existing test code (`needless_borrows_for_generic_args` in `tests/save_tests.rs`, `tests/skill_tests.rs`); fix it.
- Tests: unit tests for the setup core; native-host integration tests for each state (missing config, set up, invalid settings, folder not writable, comments preserved); install-script tests in the existing fake-cargo style; an end-to-end integration test that drives the real binary over the native protocol against a temp HOME: status before setup → read setup state → write settings → save a page → lookup shows it saved → liveness file exists.

## 4. Extension

Mockups: `docs/.mockups/setup-flow.png` (screens A–D) and `journey.png`.

**A. App not found** (popup, and the same component in the side panel). Heading "Finish setting up AgentMark" with the grey idle icon. One sentence: the extension saves pages through the AgentMark app on this computer. A list of platform rows. The detected platform (`chrome.runtime.getPlatformInfo`: OS and arch) is the first row, highlighted, sub-labelled "Your computer". Under a small "Other platforms" label: the rest. Rows: Mac Apple Silicon, Mac Intel, Linux, Windows; each shows its install method in a sub-line. Footer: "Already installed? Check again".

**B. Install method for the chosen platform.** "‹ All platforms" back link. Heading "Install on <platform>". The method comes from one data table in shared code describing what really exists per platform:
- A desktop app download exists: primary "Download AgentMark for Mac" button, terminal command as a secondary option. (The download URL is a constant, empty for now; when empty this option is not shown.)
- A pre-built CLI exists (Mac Apple Silicon today): step 1 "Open Terminal and paste this" with the command in a copyable block; step 2 "Come back here. This updates by itself."
- Neither: honest build-from-source steps and what they need (Rust, Node.js).
- Windows: "not supported yet".
Below: "Waiting for the app…" polling every 2 seconds while the popup is open, plus "Check again". While the store ID constant is empty the command includes `--extension-id <this extension's real ID>`; once set, the short command.

**C. App found, not set up.** Heading "Set up AgentMark"; "The app is installed (version). Two choices and you're done."
- "Where should bookmarks live?" Path field with the suggested folder and a "Choose…" button (folder request). Typed fallback when the dialog is unavailable.
- "Which agent writes summaries?" Radio rows for Claude Code, Codex, pi, each with a "Found" / "Not installed" badge. Preselect the first found. If none is found, summaries default to off and the UI says why.
- Switch: "Write a summary and tags after each save", with "Sends the page text to your agent's provider."
- Primary button "Finish and save this page". Footer "You can change all of this later in Settings." Field errors from the CLI show next to the field.

**D. Settings.**
- A "Settings" item in the toolbar icon's context menu, next to the side panel item. It opens the side panel on the Settings view.
- Side panel header gets a Bookmarks / Settings switch. No router; follow the existing view-state pattern.
- Sections: App (connected version, bookmark count); Storage (folder, "Choose…", note that changing it does not move existing bookmarks); Summaries (switch, agent rows with badges, optional instructions); "Capture the page from the browser on" (built-in sites as fixed chips, user sites editable); Toolbar icon (the optional permission switch). One "Save changes" button with saved and error states.

**E. Optional `tabs` permission.**
- Move `tabs` to `optional_permissions`. Update manifest tests.
- Settings switch: "Show when the current page is already saved. On every site. Chrome will ask for permission." On → `chrome.permissions.request` from the click handler; off → `remove`.
- `toolbar-icon.ts` works in three modes: no permission (reacts to saves and to pages that report themselves), granted, revoked while running. Register tab listeners only when allowed; react to `chrome.permissions.onAdded/onRemoved`.
- Without the permission the icon still turns blue on saved LinkedIn, X, Hacker News and Reddit pages: the content script reports its own page URL to the service worker. Content scripts still may not import from `src/shared/`.
- Without the permission, clicking the icon still tells the truth: the popup's save result marks the page saved and the icon turns blue then.

**F. Reuse.** Extend `src/shared/connection.ts` and `InstallPrompt.tsx` (kinds `not_installed`, `forbidden`, `worker_unreachable`, `host_crashed`); do not build a parallel system. `forbidden` keeps its own short fix. The install command and site URL are constants in one place.

**G. Tests.** Extend `chrome-mock.ts` minimally. Cover: platform detection and ordering per OS/arch; each install method; polling starts, stops on close, moves to setup when the app appears; setup happy path, field errors, no agent found, dialog unavailable; Settings load/save/error; permission grant, deny, revoke; toolbar icon in all three modes; the three connection states in popup and side panel.

## 5. Desktop app (`packages/app`, Tauri v2)

An installer, a setup screen and a settings window. It does not stay running.

**On every launch**, idempotently, each step reporting success or a specific failure with retry:
1. Place or refresh the CLI at the stable path from the contract and link `agentmark` into `~/.local/bin` (logic lives in the CLI library where possible).
2. Register the native messaging host for installed browsers.
3. Install or refresh the agent skill.

**Screens** (neutral surfaces, one indigo accent `#4f46e5`, the AgentMark mark, dark and light by system):
- Welcome / install progress: the three steps with ticks.
- Setup: the same two questions and switch as extension screen C, with a native folder dialog.
- Add the Chrome extension: a button opening the Web Store listing (URL constant, empty for now; when empty show the unpacked-extension steps, labelled as the development path). Then "Waiting for Chrome…" that becomes "Connected" from the liveness file.
- Settings: same fields as the extension's Settings, minus the browser permission switch.
- About: version, links.

**Engineering:**
- Single instance, small fixed window, no tray, no background process.
- Shares the repo's single version. Extend version-sync tests and `AGENTS.md` to include the app's `Cargo.toml`, `tauri.conf.json`, `package.json`.
- Updater configured against GitHub Releases with key and endpoints empty by default; with no key the app builds and runs with updates off.
- Release workflow signs and notarises when the secrets exist, and produces an unsigned build with a clear warning in the job summary when they do not. No credentials committed.
- Release builds the app for `aarch64-apple-darwin` (and `x86_64-apple-darwin` if clean) and attaches the `.dmg` and updater artifacts beside the existing CLI archive, which stays.
- CI `app` job: frontend typecheck/lint/tests, Rust fmt/clippy/tests for `src-tauri`, debug build on a macOS runner.
- Tests: Rust unit tests for install steps against temp HOME dirs (injectable paths); frontend component tests per screen and state.

## 6. Chrome Web Store listing draft (`docs/chrome-web-store/`)

Drafts the owner can paste in. Every claim checked against the code.

- `listing.md`: name, short description within the store limit, full description, category, language.
- `single-purpose.md`.
- `permissions.md`: one justification per permission and host in the final manifest. Remove any permission the code no longer needs instead of justifying it.
- `data-use.md`: answers for the data disclosure form. Be honest that when summaries are on, the user's own agent CLI sends page text to that agent's provider; AgentMark has no server.
- `privacy-policy.md`: policy text, published by the website at `/privacy`.
- `reviewer-notes.md`: how a reviewer tests an extension that needs a native app.
- `assets.md` plus files in `assets/`: screenshots at the store's size and a small promo tile, made from real UI. No third-party logos or real names.
- A script that builds the store zip from `packages/extension/dist`, stripping what the store rejects and failing on a version mismatch.
- `checklist.md`: the steps only the owner can do, in order.

## 7. Website (`web/`)

- The live site is `https://agentmark.app`. The old `agentmark.vercel.app` address belongs to someone else, yet the extension's setup link, `README.md`, and the site's metadata, sitemap and `llms.txt` point at it. One site-URL constant per package, set to the working address.
- Install section rebuilt around the two doors: "Add to Chrome" and "Download for Mac", each saying what happens next. While a URL constant is empty, show the current real path (terminal command, unpacked extension), never a dead link.
- Add `/privacy`.
- Update hero, privacy section (browsing history is now an optional switch), `llms.txt` and demo wording. Add one demo in the demo-slot convention: first run, from "app not found" through setup to saved.
- Fix: side panel demo's last button is cut off on wide screens; bundle viewer's file strip scrolls sideways on phones with no hint.
- `web/` stays self-contained and static; dark and light by system. Verify with screenshots at 1440px and 390px in both schemes.

## 8. Docs

- `README.md`: new install paths, setup without a terminal, the optional permission, the app.
- `AGENTS.md`: structure, setup module and contract, app section, toolbar icon permission modes, versioning files, CI jobs. Correct the stale Installer section.
- `packages/skill/SKILL.md`: only if agent-facing behaviour changed.

## 9. Owner-only (do not attempt; leave one-constant or one-secret switch points)

Web Store account, upload and review; Apple enrolment, certificates, signing secrets; licence, domain, short install URL; repo secrets, release tags, merging, deploying the website.

## 10. Verification

- `packages/cli`: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo build && cargo test`
- `packages/extension`: `npm ci && npm run typecheck && npm run lint && npm run test && npm run build`
- `packages/app`: typecheck, lint, tests, build
- `web`: `pnpm install --frozen-lockfile && pnpm typecheck && pnpm lint && pnpm build`
- Built content scripts contain no `import`/`export` and no shared chunk.
- Version 0.3.0 in every file the versioning rule names, plus the app's.
