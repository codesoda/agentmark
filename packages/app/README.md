# AgentMark desktop app

A Tauri v2 app that installs the `agentmark` CLI, registers it with Chrome,
installs the agent skill, and runs setup and settings. Spec: `docs/setup-spec.md`
section 5. Contract: `docs/setup-contract.md`.

## Commands (from this folder)

| Command | What it does |
|---|---|
| `npm run dev` | Frontend on http://localhost:1430. In a plain browser it uses mock commands (`src/dev-mock.ts`; `?scenario=…&view=…`, `&stale` for an old extension connection). |
| `npm run tauri dev` | The real app against your real HOME. It does not copy a CLI into `~/.agentmark/bin` (the "Install the agentmark command" step says so), so a release CLI stays in place. To install `target/debug/agentmark` there, build it first (`cargo build -p agentmark`, after the extension build to embed it) and run `AGENTMARK_DEV_INSTALL_CLI=1 npm run tauri dev`. |
| `npm run typecheck`, `lint`, `test`, `build` | Frontend checks. |
| `npm run sidecar` | Build the extension and the CLI into `src-tauri/binaries/agentmark-<triple>`. |
| `npm run app:build:debug` / `app:build` | Sidecar plus `tauri build` with `src-tauri/tauri.bundle.conf.json`. Output: `../../target/<profile>/bundle/`. |
| `npm run icons` | Regenerate `src-tauri/icons/` from `../extension/icons/icon.svg`. |

A debug build counts as a development run only when it runs from `target/`,
not from a bundled `.app`; the debug bundle installs its sidecar as usual.
To try a build without touching your real setup, start the binary with a
temporary HOME:

```sh
HOME=$(mktemp -d) ../../target/debug/bundle/macos/AgentMark.app/Contents/MacOS/agentmark-app
```

## Switch points for the owner

- **Chrome Web Store listing**: `WEB_STORE_URL` in `src/constants.ts`. Empty shows the unpacked development steps.
- **Store extension ID**: `WEB_STORE_EXTENSION_ID` in `packages/cli/src/setup/extension_ids.rs` (the app uses the CLI's). Set, so the launch step registers Chrome without a pasted ID; the unpacked copy has the same ID through the manifest `key`. Pasting an ID is a collapsed option for a copy with another ID.
- **Updates**: `plugins.updater.pubkey` and `plugins.updater.endpoints` in `src-tauri/tauri.conf.json`, plus the `TAURI_SIGNING_PRIVATE_KEY` (and optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) repository secrets. Generate the key pair with `npx tauri signer generate`. Endpoint: `https://github.com/codesoda/agentmark/releases/latest/download/latest.json`.
- **Signing and notarisation** (release workflow secrets): `APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, then `APPLE_ID`, `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID`. Without them the release has an unsigned app and a warning in the job summary.
- **Bundle identifier**: `identifier` in `src-tauri/tauri.conf.json` (`app.agentmark.desktop`, decided by the owner). Changing it after the first signed release breaks updates.
