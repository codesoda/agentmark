# Setup contract

Single source of truth for everything that crosses a package boundary in the extension-first / app-first setup work. Read with `docs/setup-spec.md` (the what and why) and `AGENTS.md` (binding conventions). Where this file and the spec disagree, this file wins; each disagreement is listed in §15.

Conventions: wire fields are snake_case; native messages use serde `#[serde(tag = "type", rename_all = "snake_case")]`; "absent" means the key is not in the JSON object; `null` is distinct from absent only where stated. Every literal below is defined once (in backticks, at first use) and used verbatim everywhere else.

---

## 1. Setup core API (Rust library)

Module: `agentmark::setup`, files under `packages/cli/src/setup/`:

| File | Contents |
|---|---|
| `mod.rs` | `SetupState`, `Settings`, `SettingsChanges`, `read_state`, `is_set_up`, `apply_settings`, `default_storage_path`, `display_path`, `SetupEnv` |
| `config_edit.rs` | comment-preserving `config.toml` edit (private) |
| `agents.rs` | `AgentInfo`, `detect_agents` |
| `folder.rs` | `FolderDialog` trait, `SystemFolderDialog`, `FolderChoice` |
| `extension_ids.rs` | `WEB_STORE_EXTENSION_ID`, development IDs, `allowed_extension_ids` |
| `install.rs` | `install_cli`, `link_cli`, `register_native_hosts`, `install_skill` |
| `liveness.rs` | `ConnectionRecord`, `record_connection`, `read_connection` |

Add `pub mod setup;` to `lib.rs`. `skill::install_skill` stays where it is; `setup::install_skill` wraps it (so `skill` can stay `pub(crate)`). `agent::provider::agent_search_path` becomes `pub(crate)` so `setup::agents` reuses it.

### 1.1 Types

```rust
pub struct SetupEnv {
    pub home: PathBuf,
    /// PATH used to find agent CLIs. Production: agent::provider::agent_search_path().
    pub search_path: OsString,
}
impl SetupEnv { pub fn from_process() -> Result<Self, ConfigError>; } // HomeMissing if no HOME

#[derive(Serialize)] pub struct SetupState {
    pub initialized: bool,
    pub config_problem: Option<String>,
    pub version: String,                  // env!("CARGO_PKG_VERSION")
    pub platform: PlatformInfo,
    pub settings: Settings,
    pub default_storage_path: String,
    pub default_storage_path_display: String,
    pub agents: Vec<AgentInfo>,           // always 3, order claude, codex, pi
    pub builtin_snapshot_sites: Vec<String>, // snapshot_rules::BUILTIN_RULES, in order
    pub bookmark_count: Option<u64>,
}
#[derive(Serialize)] pub struct PlatformInfo { pub os: String, pub arch: String } // std::env::consts::{OS, ARCH}

#[derive(Serialize)] pub struct Settings {
    pub storage_path: String,             // absolute
    pub storage_path_display: String,     // display_path(storage_path)
    pub default_agent: String,            // "claude" | "codex" | "pi"
    pub enrichment_enabled: bool,
    pub system_prompt: Option<String>,
    pub snapshot_sites: Vec<String>,      // user sites only
}

#[derive(Deserialize, Default)] pub struct SettingsChanges {
    #[serde(default)] pub storage_path: Option<String>,
    #[serde(default)] pub default_agent: Option<String>,
    #[serde(default)] pub enrichment_enabled: Option<bool>,
    #[serde(default, deserialize_with = "double_option")] pub system_prompt: Option<Option<String>>,
    #[serde(default)] pub snapshot_sites: Option<Vec<String>>,
}

#[derive(Serialize)] pub struct FieldError {
    pub field: SettingsField,        // serialized snake_case
    pub index: Option<usize>,        // only for snapshot_sites entries
    pub code: FieldErrorCode,        // serialized snake_case
    pub message: String,             // human sentence, shown as-is
}
pub enum SettingsField { StoragePath, DefaultAgent, EnrichmentEnabled, SystemPrompt, SnapshotSites }
pub enum FieldErrorCode { Blank, NotAbsolute, TooLong, NotADirectory, CannotCreate, NotWritable, InvalidAgent, TooMany, InvalidSite }

#[derive(Debug, thiserror::Error)] pub enum SettingsError {
    #[error("invalid settings")] Invalid(Vec<FieldError>),
    #[error("failed to write {path}: {source}")] Write { path: PathBuf, source: std::io::Error },
    #[error("failed to open the index: {0}")] Index(#[from] crate::db::DbError),
}

#[derive(Serialize)] pub struct AgentInfo {
    pub id: String,          // "claude" | "codex" | "pi"
    pub name: String,        // display name, §6
    pub found: bool,
    pub path: Option<String>,// absolute path of the executable found
}

pub enum FolderChoice { Chosen(PathBuf), Cancelled, Unavailable { reason: String } }
pub trait FolderDialog { fn pick(&self, start: Option<&Path>) -> FolderChoice; }
pub struct SystemFolderDialog;  // §2.4
```

### 1.2 Functions

| Function | Behaviour | Errors | Side effects | Idempotent | Safe before setup |
|---|---|---|---|---|---|
| `pub fn is_set_up(home: &Path) -> bool` | `Config::load(home).is_ok()` | none | none | yes | yes |
| `pub fn read_state(env: &SetupEnv) -> SetupState` | Never fails. `initialized = is_set_up`. `config_problem`: `None` when initialized or file missing; `Some(err.to_string())` when the file exists but `Config::load` fails. `settings`: current values when initialized; otherwise proposed defaults (§5.1). `bookmark_count`: `Some(count_bookmarks)` when initialized and the index opens, else `None`. | — | When initialized, opens/migrates `index.db` (same as `open_repository` today). Creates nothing when not initialized. | yes | yes |
| `pub fn apply_settings(env: &SetupEnv, changes: &SettingsChanges) -> Result<SetupState, SettingsError>` | Partial update (§5.2). Validates every field and collects all errors before writing anything. Then, in order: create storage folder, create `~/.agentmark/`, write `config.toml` (§5.3), `db::open_and_migrate(index_db_path)`. Returns `read_state` after the write. | `Invalid` (no writes happened), `Write`, `Index` | creates folders, writes config, creates/migrates index | yes (same input twice = same file) | yes — this is how setup happens |
| `pub fn default_storage_path(home: &Path) -> PathBuf` | `home/Documents/AgentMark` on every OS | — | none | yes | yes |
| `pub fn display_path(path: &Path, home: &Path) -> String` | `~` if equal to home, `~/rest` if under home, else the absolute path | — | none | yes | yes |
| `pub fn detect_agents(search_path: &OsStr) -> Vec<AgentInfo>` | §6 | — | none (stat only, never runs the agent) | yes | yes |
| `pub fn choose_folder(dialog: &dyn FolderDialog, start: Option<&Path>) -> FolderChoice` | Calls `dialog.pick`; on `Chosen`, strips a trailing `/` (except `/`) | — | shows a dialog | — | yes |
| `install_cli`, `link_cli`, `register_native_hosts`, `install_skill` | §8, §10 | `InstallError` | §10 | yes | yes |
| `record_connection`, `read_connection` | §9 | never fail (log at `debug`) | §9 | yes | yes |

Callers: native host (§2), `agentmark init` (§11), Tauri app (direct calls). Nobody else may write `config.toml`.

---

## 2. Native messages

Host process: `agentmark` started by Chrome with argv[1] = `chrome-extension://<id>/`. `main.rs` passes that origin to `run_native_host(origin: Option<String>)` (it is currently dropped). Allow-list in `IncomingMessage::from_value` gains `get_setup_state`, `save_settings`, `choose_folder`. Dispatch stays exhaustive.

### 2.1 `status` (changed)

Request (new optional field; the extension's service worker always fills it from `chrome.runtime.getManifest().version`):
```json
{"type":"status","extension_version":"0.3.0"}
```
`extension_version`: optional string, ≤ 64 chars (longer → truncated, never rejected). Old CLIs ignore it (serde ignores extra keys on internally tagged unit variants; the builder must add a test that `{"type":"status","extension_version":"x"}` parses as `Status` and keep `Status` accepting extra keys).

Response (new field `initialized`):
```json
{"type":"status_result","ok":true,"version":"0.3.0","initialized":false}
```
`ok` stays `true` always. `initialized` = `setup::is_set_up(home)`. Must stay cheap (5 s timeout): no DB access, no agent detection.

### 2.2 `get_setup_state` (new)

```json
{"type":"get_setup_state"}
```
Success:
```json
{"type":"setup_state_result","state":{
  "initialized": true,
  "config_problem": null,
  "version": "0.3.0",
  "platform": {"os":"macos","arch":"aarch64"},
  "settings": {
    "storage_path": "/Users/me/Documents/AgentMark",
    "storage_path_display": "~/Documents/AgentMark",
    "default_agent": "claude",
    "enrichment_enabled": true,
    "system_prompt": null,
    "snapshot_sites": ["example.com/app/"]
  },
  "default_storage_path": "/Users/me/Documents/AgentMark",
  "default_storage_path_display": "~/Documents/AgentMark",
  "agents": [
    {"id":"claude","name":"Claude Code","found":true,"path":"/Users/me/.local/bin/claude"},
    {"id":"codex","name":"Codex","found":false,"path":null},
    {"id":"pi","name":"pi","found":false,"path":null}
  ],
  "builtin_snapshot_sites": ["linkedin.com/in/","linkedin.com/posts/","linkedin.com/feed/update/","linkedin.com/company/","x.com","twitter.com","news.ycombinator.com/item","reddit.com"],
  "bookmark_count": 42
}}
```
All keys always present. Nullable: `config_problem`, `settings.system_prompt`, `agents[].path`, `bookmark_count`. `platform.os`/`arch` are Rust `std::env::consts` values (informational; the extension's platform list uses §7, not these). Failure: only `error` (e.g. `HOME` unset).

### 2.3 `save_settings` (new)

```json
{"type":"save_settings","settings":{
  "storage_path":"~/Documents/AgentMark",
  "default_agent":"claude",
  "enrichment_enabled":true,
  "system_prompt":null,
  "snapshot_sites":["example.com/app/"]
}}
```
`settings` is required (an object); each key in it is optional. Partial-update rule in §5.2. A wrong JSON type (e.g. `"enrichment_enabled":"yes"`) fails parsing → `error` with `invalid message fields: …`.

Success:
```json
{"type":"save_settings_result","state":{ …same object as setup_state_result.state… }}
```
Validation failure (nothing written):
```json
{"type":"settings_invalid","errors":[
  {"field":"storage_path","index":null,"code":"not_writable","message":"AgentMark cannot write to /Volumes/X."},
  {"field":"snapshot_sites","index":2,"code":"invalid_site","message":"\"exa mple.com\" is not a host/path rule."}
]}
```
`errors` has ≥ 1 entry; at most one entry per field, except `snapshot_sites` which has at most one per index plus at most one with `index: null` (`too_many`). Write/index failure: `{"type":"error","message":"failed to write …"}`.

### 2.4 `choose_folder` (new)

```json
{"type":"choose_folder","start_path":"/Users/me/Documents"}
```
`start_path`: optional string; `~` expanded; ignored if not an existing directory.

Response (always this type; none of the three outcomes is an `error`):
```json
{"type":"choose_folder_result","outcome":"chosen","path":"/Users/me/Bookmarks","path_display":"~/Bookmarks","reason":null}
{"type":"choose_folder_result","outcome":"cancelled","path":null,"path_display":null,"reason":null}
{"type":"choose_folder_result","outcome":"unavailable","path":null,"path_display":null,"reason":"No folder dialog is available on this system."}
```
`SystemFolderDialog`:

| OS | Program (found via the agent search PATH) | chosen | cancelled | unavailable |
|---|---|---|---|---|
| macOS | `osascript -e 'activate' -e 'POSIX path of (choose folder with prompt "Choose where AgentMark keeps bookmarks" default location (POSIX file "<start>"))'` (omit `default location …` when no start) | exit 0, stdout trimmed | exit ≠ 0 and stderr contains `-128` | any other failure, `osascript` missing |
| Linux | `zenity --file-selection --directory --title="Choose where AgentMark keeps bookmarks" [--filename=<start>/]`, else `kdialog --getexistingdirectory <start>` | exit 0, stdout trimmed | exit 1 | neither program, or both `DISPLAY` and `WAYLAND_DISPLAY` unset |
| other | — | — | — | always |

The dialog process is killed after 300 s → `unavailable` with reason `Timed out waiting for the folder dialog.`

**Timeout and port handling.** DECISION: the extension never sends `choose_folder` on the shared `connectNative` port. It uses `chrome.runtime.sendNativeMessage` (one-shot: Chrome starts a fresh host process for one message), wrapped in `NativeMessagingClient.sendOneShot(request, timeoutMs)` in `native-messaging.ts` with a 310 000 ms timeout. Reason: the host is single-threaded and the port matches responses FIFO, so a minutes-long dialog would block every other request and a 30 s timeout would misalign all later responses.

**Popup closes when the dialog takes focus.** Chrome closes the popup on blur, so the popup cannot receive the reply. DECISION: the service worker, on `outcome: "chosen"`, writes the path into the setup draft (§4.4) before replying. The popup restores from the draft when reopened. The side panel stays open and uses the reply directly.

### 2.5 "Setup required" (existing requests)

DECISION: a structured field on `error`, not a new response type. Reason: old extensions throw on unknown response types (`parseNativeResponse`) but show `error.message`, so this degrades to a readable message.

`OutgoingMessage::Error` becomes `Error { message: String, #[serde(skip_serializing_if = "Option::is_none")] code: Option<ErrorCode> }` with `pub enum ErrorCode { SetupRequired }` (snake_case). `OutgoingMessage::error(msg)` keeps `code: None`; add `OutgoingMessage::setup_required()`.

For `save`, `list`, `show`, `update`, `list_collections`, `lookup_urls`: `dispatch` checks `setup::is_set_up(home)` first and, if false, returns exactly:
```json
{"type":"error","message":"not initialized: AgentMark is not set up yet. Finish setup in the extension or the AgentMark app.","code":"setup_required"}
```
The message keeps the `not initialized` prefix (existing tests and the old-CLI fallback rely on it). `status`, `get_setup_state`, `save_settings`, `choose_folder` work before setup.

### 2.6 Compatibility

| Extension | CLI | Result |
|---|---|---|
| old | new | `status` ignores missing `extension_version`; extra `initialized` and `code` keys ignored by old parsers; old extension never sends new types. Works. |
| new | old | `status_result` has no `initialized` → treated as ready (§4.2). Not-set-up old CLI answers `not initialized: …` with no `code` → extension matches the prefix → `setup_required`. Then `get_setup_state` answers `unknown message type: get_setup_state` → extension shows `update_required`. `choose_folder` one-shot to old CLI → same `unknown message type` error → treat as `unavailable`. |

---

## 3. Runtime messages

### 3.1 Types in `src/shared/types.ts`

Native wire additions mirror §2 exactly: `StatusRequest.extension_version?: string`; `StatusResultResponse.initialized?: boolean` (optional on parse for old CLIs); `ErrorResponse.code?: "setup_required"` (any other string → dropped on parse); new `GetSetupStateRequest`, `SaveSettingsRequest`, `ChooseFolderRequest`, `SetupStateResultResponse`, `SaveSettingsResultResponse`, `SettingsInvalidResponse`, `ChooseFolderResultResponse`; add their `type` strings to `RESPONSE_TYPES` with full parsers.

`RuntimeErrorResponse` gains `code?: "setup_required"`. `handleNativeRequest` copies `response.code` into it. `settings_invalid` and `choose_folder_result` are non-`error` types, so they arrive as `{success: true, data}`.

### 3.2 Popup / side panel → service worker

| Message (literal) | Fields | Service worker does | Response |
|---|---|---|---|
| `{"type":"status"}` | — (unchanged) | adds `extension_version`, sends on port | `RuntimeResponse` |
| `{"type":"get_setup_state"}` | — | sends on port | `RuntimeResponse` with `setup_state_result` |
| `{"type":"save_settings","settings":{…}}` | `settings`: object (§2.3) | forwards `settings` unchanged (CLI validates) | `RuntimeResponse` with `save_settings_result` or `settings_invalid` |
| `{"type":"choose_folder","start_path":"…"}` | `start_path?: string` | `sendOneShot` (§2.4); on `chosen` updates the draft (§4.4); on `unknown message type` returns a synthesized `{"type":"choose_folder_result","outcome":"unavailable","path":null,"path_display":null,"reason":"Update AgentMark to use the folder dialog."}` | `RuntimeResponse` with `choose_folder_result` |

Runtime helpers in `runtime.ts`: `sendGetSetupState()`, `sendSaveSettings(settings)`, `sendChooseFolder(startPath?)`.

### 3.3 Content script → service worker

```json
{"type":"report_page","url":"https://x.com/someone/status/123"}
```
Sent by `posts.entry.ts` (all three site adapters) once per distinct `location.href`: at the first injection pass and whenever a later pass sees `location.href` changed. Content scripts define the literal `"report_page"` locally (no `src/shared/` import); the existing constant-sync test must cover it.

Service worker validation (silently ignore on any failure, respond `{"ok":false}`): `sender.tab?.id` is a number; `sender.frameId === 0`; `typeof url === "string"`; `isSupportedUrl(url)`; `url === sender.url`. On success: `toolbarIcon.reportPage(tabId, url)` and respond `{"ok":true}`. The content script ignores the response.

### 3.4 Service worker → side panel (open Settings)

Action context menu item: id `agentmark-settings`, title `Settings`, `contexts: ["action"]`, created next to `agentmark-sidepanel`. On click: `chrome.storage.session.set({agentmark_sidepanel_view: "settings"})` (not awaited) then `chrome.sidePanel.open({windowId})` (must stay within the gesture). The side panel reads `agentmark_sidepanel_view` on load and on `chrome.storage.onChanged`, switches view, then removes the key. Values: `"settings"` only (`"bookmarks"` is the default and is never written).

### 3.5 Install-time ping

On `chrome.runtime.onInstalled` and `onStartup`, the service worker sends one `status` (errors ignored) so the host writes the liveness file (§9) and the app's "Waiting for Chrome…" can turn to "Connected" without the user opening the popup.

---

## 4. Extension state model

### 4.1 States

`type AppState = "checking" | "not_installed" | "forbidden" | "worker_unreachable" | "host_crashed" | "update_required" | "setup_required" | "ready"`. Lives in `src/shared/connection.ts` next to `ConnectionProblem` (the four problem kinds are a subset). `InstallPrompt.tsx` keeps rendering the four problem kinds; `not_installed` renders the new platform list (screens A/B); `forbidden` keeps its register-command fix.

### 4.2 Derivation (from the `status` probe)

| Probe result | State |
|---|---|
| no answer yet | `checking` |
| `success:false`, `classifyConnectionError(error)` = kind | that kind |
| `success:false`, unclassified (e.g. `Native host request timed out`) | `host_crashed` |
| `status_result` with `initialized === false` | `setup_required` |
| `status_result` with `initialized === true` or absent | `ready` |
| `success:true` with any other type | `host_crashed` |

Any later request result moves state:
- `success:false` with `code === "setup_required"`, or `error` starting with `not initialized` → `setup_required`.
- `get_setup_state` / `save_settings` failing with an error starting with `unknown message type` → `update_required`. UI: "This AgentMark app is too old for this extension. Update it:" plus the install command of the detected platform (§7).
- connection errors → the classified kind.

### 4.3 Polling

- Popup: while mounted and state ∈ {`not_installed`, `forbidden`}, send `status` 2000 ms after the previous probe settles (no overlap). Stops on any other state or unmount. "Check again" runs one probe immediately and resets the timer.
- Side panel: same rule, additionally paused while `document.visibilityState !== "visible"`.
- DECISION: polls on screen A as well as B. Reason: the user may install from the website without choosing a row.
- On `not_installed`/`forbidden` → `setup_required`: popup calls `get_setup_state` and shows screen C. → `ready`: normal save view.

### 4.4 Setup draft

`chrome.storage.session` key `agentmark_setup_draft`: `{"storage_path": string|null, "default_agent": string|null, "enrichment_enabled": boolean|null, "pending_save": {"url": string, "title"?: string, "page_html"?: string}|null, "updated_at": number}` (`updated_at` = epoch ms). The popup writes it on every change in screen C and reads it on mount (ignore if older than 600 000 ms); `null` fields fall back to `settings` from `get_setup_state`. The service worker writes `storage_path` (and `updated_at`) on a chosen folder. `pending_save` is the in-page item (for example a post) whose save setup interrupted, so the reopened popup saves that item rather than the whole tab; the popup clears it once used. Removed after `save_settings_result`.

### 4.5 Screen C defaults

From `setup_state_result.state.settings` (CLI computes them, §5.1). "Not installed" badge = `found:false`. When no agent is found, the switch starts off and the UI says no agent was found. The UI may still select an agent that is not found (allowed, §5.2). Submit sends all five fields except `system_prompt` and `snapshot_sites` (absent = unchanged). Success → popup runs the normal save of the active tab ("Finish and save this page").

---

## 5. Settings semantics

### 5.1 Fields

| Wire field | `config.toml` location | Type | Default (not set up) | Validation (code) |
|---|---|---|---|---|
| `storage_path` | top-level `storage_path` | string | `default_storage_path(home)` | trim; empty → `blank`; `~` or `~/…` expanded against home; else must be absolute → `not_absolute`; > 4096 bytes → `too_long`; exists and not a dir → `not_a_directory`; `create_dir_all` fails → `cannot_create`; write+delete of `.agentmark-write-test-<pid>` fails → `not_writable` |
| `default_agent` | top-level `default_agent` | string | first of `claude`,`codex`,`pi` with `found`, else `claude` | `config::validate_agent` (trim, lowercase) → `invalid_agent`. Not-found agents are allowed. |
| `enrichment_enabled` | `[enrichment] enabled` | bool | `true` if any agent found, else `false` | type only |
| `system_prompt` | top-level `system_prompt` | string or null | `null` | trimmed; empty → treated as `null`; > 20 000 chars → `too_long` |
| `snapshot_sites` | top-level `snapshot_sites` | string[] | `[]` | > 100 entries → `too_many` (index null). Per entry: trim, strip `http://`/`https://`, lowercase the host part; empty or contains whitespace or host has chars outside `[a-z0-9.-]` or host empty → `invalid_site`; > 200 chars → `too_long`. Duplicates and entries equal to a built-in rule are dropped silently. |

Validation order inside `storage_path`: the filesystem checks (create, writable) run only if the string checks pass and no other field failed. Relative paths: DECISION — rejected by the core (`not_absolute`); there is no meaningful working directory for the native host or the app. `agentmark init` resolves relative input against its cwd before calling the core, so the CLI keeps today's behaviour.

`log_level` and any other keys are never touched.

### 5.2 Partial update

Absent key = keep current value (or the §5.1 default when not set up / config unreadable). A file that parses as TOML but does not load as a config (a missing table, a wrong type) is not thrown away: each of its keys that is valid on its own (`storage_path` if absolute, `default_agent`, `enrichment.enabled`, `system_prompt`, `snapshot_sites`) is the current value, and only the rest fall back to §5.1. `system_prompt: null` clears it. `snapshot_sites` replaces the whole user list. Built-in sites (`builtin_snapshot_sites`) are not editable and never stored.

### 5.3 Writing `config.toml`

DECISION: targeted edit with `toml_edit` (add `toml_edit = "0.22"` to `packages/cli/Cargo.toml`; already in `Cargo.lock` as 0.22.27 via `toml`). Reason: keeps comments, key order and unknown keys.

- File exists and parses as TOML: edit `storage_path`, `default_agent`, `system_prompt` (remove key when null), `snapshot_sites` (remove key when empty), `enrichment.enabled` (create `[enrichment]` table if missing). Fields not in the change set but required by `Config` and missing from the file are written with their current/default values, so the result always loads. If the file does not load as a config before the edit, a copy is first kept as `config.toml.invalid-<UTC YYYYMMDDTHHMMSSZ>`. If the edit still does not load (for example a `log_level` of the wrong type, which the edit never touches), a new file is written from the template and the copy is kept.
- File exists but is not valid TOML: rename it to `config.toml.invalid-<UTC YYYYMMDDTHHMMSSZ>` and write a new file. `config_problem` before the save tells the UI this will happen.
- So a `config.toml.invalid-<ts>` copy is kept in both cases: unparseable TOML, and parseable TOML that does not load.
- File missing: start from `render_config_toml` (the existing commented template) and apply the values.
- Atomic: write `config.toml.tmp-<pid>` in the same dir, `fsync`, rename over `config.toml`. Then `Config::load` must succeed (test asserts this).
- Stored `storage_path` is the absolute expanded path, never `~`.

### 5.4 Effects of changing a setting

- `storage_path`: new saves go to the new folder. Existing bundles are not moved. The index (`~/.agentmark/index.db`, fixed location) is kept, so old bookmarks still list and search. Re-saving an old bookmark's URL writes a fresh bundle in the new folder, keeping its id, date and user-owned fields from the index. `show` and `update` (CLI and extension) fail with a specific error: "the files for bookmark <id> are not in the storage folder <folder>. They may still be in the folder AgentMark used before; move them into this folder to open or edit the bookmark". Reprocess still fails with "bundle not found". Settings note text: "Changing the folder does not move bookmarks you already saved."
- `enrichment_enabled = false`: no post-processing at all. This also turns off model rendering of `article.md` for snapshot pages (LinkedIn, X, HN, Reddit, `snapshot_sites`); they keep the extracted article. UI copy must not claim the switch only affects summaries.
- `default_agent`, `system_prompt`: used by the next post-process run.
- `snapshot_sites`: used by the next save.

---

## 6. Agent detection

| `id` | Executable probed | `name` (UI display; UIs take it from `agents[].name`, never hardcode) |
|---|---|---|
| `claude` | `claude` | `Claude Code` |
| `codex` | `codex` | `Codex` |
| `pi` | `pi` | `pi` |

Search path: `agent::provider::agent_search_path()` (inherited `PATH` then `EXTRA_BIN_DIRS`), the same one `RealRunner` uses. First directory containing a regular file (symlinks followed) with any execute bit wins; `path` is that `dir/name` (not canonicalized). The Tauri app, launched from Finder with a bare PATH, gets the same result because of the appended directories.

---

## 7. Platforms and constants

### 7.1 Constants

| Name | Value today | Extension | CLI | App | Web |
|---|---|---|---|---|---|
| `WEB_STORE_EXTENSION_ID` | `"oepdfndmpcpimbkaaocgolenncdkljeo"` (store item, draft) | `src/shared/constants.ts` | `src/setup/extension_ids.rs` (`pub const …: &str`) | uses CLI's via the library | — |
| `WEB_STORE_URL` | `""` | `src/shared/constants.ts` | — | `src/constants.ts` | `src/lib/site.ts` |
| `MAC_APP_DOWNLOAD_URL` (Apple Silicon) | `""` | `src/shared/constants.ts` | — | — | `src/lib/site.ts` |
| `MAC_INTEL_APP_DOWNLOAD_URL` | `""` | `src/shared/constants.ts` | — | — | `src/lib/site.ts` |
| `SITE_URL` | `"https://agentmark.app"` | `src/shared/constants.ts` | — | `src/constants.ts` | `src/lib/site.ts` (fix value) |
| `INSTALL_SCRIPT_URL` | `"https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh"` | `src/shared/constants.ts` | — | — | `src/lib/site.ts` |

Paths are relative to each package root (`packages/extension`, `packages/cli`, `packages/app`, `web`). In the extension, `SETUP_GUIDE_URL` becomes `` `${SITE_URL}/#install` `` and `INSTALL_COMMAND` is replaced by `installCommand()` below. On the web, `INSTALL_CMD` is replaced by the same command builder. A test in each of CLI and extension asserts `WEB_STORE_EXTENSION_ID` is 32 chars in `a`–`p` and that both copies are equal (each reads the other file as text), and tests in both assert that the extension manifest's `key` gives this ID (SHA-256 of the DER key, first 16 bytes, hex digits `0`–`f` as `a`–`p`). The manifest `key` makes an unpacked copy (including `~/.agentmark/extension`) load with the store ID; `npm run build:store-zip` removes `key` from the store zip. Code paths for an empty store ID stay testable through `register_native_hosts_with_store_id` / `allowed_extension_ids_with_store_id`.

### 7.2 Commands

`cliCommand(extensionId)`, `fromSourceCommand(extensionId)` in `src/shared/platforms.ts` (web: `src/lib/platforms.ts`):

- pre-built: `curl -sSL <INSTALL_SCRIPT_URL> | bash`
- from source: `curl -sSL <INSTALL_SCRIPT_URL> | bash -s -- --from-source`
- `--extension-id <id>` is appended (after `bash -s --` for the pre-built form, i.e. `… | bash -s -- --extension-id <id>`) whenever `chrome.runtime.id !== WEB_STORE_EXTENSION_ID` (this includes the constant being empty). On the website (no extension ID) the suffix is omitted.

### 7.3 Platform table (canonical; extension and web each hold a copy in `platforms.ts`)

| `key` | Display name | Detect from `getPlatformInfo()` | Method |
|---|---|---|---|
| `mac_arm64` | Mac Apple Silicon | `os === "mac" && arch === "arm64"` | `app_download` if `MAC_APP_DOWNLOAD_URL` ≠ `""` (secondary: pre-built command), else `cli_command` |
| `mac_x64` | Mac Intel | `os === "mac" && arch !== "arm64"` | `app_download` if `MAC_INTEL_APP_DOWNLOAD_URL` ≠ `""` (secondary: from-source command), else `build_from_source` |
| `linux` | Linux | `os === "linux"` | `build_from_source` |
| `windows` | Windows | `os === "win"` | `unsupported` |

Methods (sub-line text in the row):

| Method | Sub-line | Screen B content |
|---|---|---|
| `app_download` | `Download the app` | primary button "Download AgentMark for Mac" → URL; secondary: the command |
| `cli_command` | `One Terminal command` | step 1 "Open Terminal and paste this" + pre-built command; step 2 "Come back here. This updates by itself." |
| `build_from_source` | `Build from source (needs Rust and Node.js)` | prerequisites Rust (`https://rustup.rs`) and Node.js; the from-source command; step 2 as above |
| `unsupported` | `Not supported yet` | "AgentMark does not run on Windows yet." |

Ordering: the detected row first, sub-labelled "Your computer"; the rest under "Other platforms" in table order. If nothing matches (e.g. `cros`), DECISION: no highlighted row, all four under "Other platforms". Changing a method when a new release target appears is an edit to this table in both copies.

---

## 8. Extension IDs and native host manifests

- Host name: `com.agentmark.native` (unchanged; `NATIVE_HOST_NAME` in extension `constants.ts`, `NATIVE_HOST_NAME` moves from `install_extension.rs` to `setup/install.rs` as `pub const`).
- Manifest file name: `com.agentmark.native.json`.
- Manifest:
```json
{
  "name": "com.agentmark.native",
  "description": "AgentMark native messaging host",
  "path": "/Users/me/.agentmark/bin/agentmark",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://<store id>/", "chrome-extension://<dev id>/"]
}
```

### 8.1 Allowed origins

`allowed_extension_ids(home)` = `WEB_STORE_EXTENSION_ID` (if non-empty) followed by development IDs in insertion order, deduplicated. Development IDs persist in `~/.agentmark/native-host.json`:
```json
{"development_extension_ids":["abcdefghijklmnopabcdefghijklmnop"]}
```
- `install-extension --extension-id X`: validate (32 lowercase letters), append X if new, re-register. Never removes the store ID.
- `install-extension` with no ID: re-register with the current list.
- Migration (DECISION, keeps existing unpacked installs working): if `native-host.json` does not exist, read every existing `com.agentmark.native.json` this code would write, take its `allowed_origins` IDs other than the store ID, and save them as development IDs before registering.
- If the list is empty (only possible in a build with an empty store ID): write no manifest; `register_native_hosts` returns `InstallError::NoExtensionIds`; `install-extension` prints the load-unpacked steps and the `--extension-id` command and exits 0.

### 8.2 Target directories (user level)

| Browser | macOS | Linux | Written when |
|---|---|---|---|
| Chrome | `~/Library/Application Support/Google/Chrome/NativeMessagingHosts/` | `~/.config/google-chrome/NativeMessagingHosts/` | always (dir created) |
| Chromium | `~/Library/Application Support/Chromium/NativeMessagingHosts/` | `~/.config/chromium/NativeMessagingHosts/` | parent user-data dir exists |
| Edge | `~/Library/Application Support/Microsoft Edge/NativeMessagingHosts/` | `~/.config/microsoft-edge/NativeMessagingHosts/` | parent exists |
| Brave | `~/Library/Application Support/BraveSoftware/Brave-Browser/NativeMessagingHosts/` | `~/.config/BraveSoftware/Brave-Browser/NativeMessagingHosts/` | parent exists |
| Arc | `~/Library/Application Support/Arc/User Data/NativeMessagingHosts/` | — | parent exists |

"Parent" = the path without the final `NativeMessagingHosts/`. OWNER: Brave and Arc paths come from community sources; confirm on a real machine. Safe default meanwhile: write them only when the parent exists (harmless otherwise). Writes are atomic (temp + rename). `register_native_hosts(home, binary) -> Result<Vec<HostRegistration>, InstallError>` returns one `HostRegistration { browser: &'static str, manifest_path: PathBuf, outcome: Written | SkippedNotInstalled | Failed(String) }` per row for this OS; it errors only on `NoExtensionIds` or if Chrome's write fails.

### 8.3 Registered `path`

DECISION: always `~/.agentmark/bin/agentmark` (a real file, not a symlink into the app). Reason: it survives the app being moved, renamed, updated or deleted, and it is the path `install.sh` already uses, so both installers converge on one manifest.

| Event | Behaviour |
|---|---|
| App launched | `install_cli` copies `<App>.app/Contents/MacOS/agentmark` (the sidecar) to `~/.agentmark/bin/agentmark` per the rule below, then `link_cli`, `register_native_hosts`, `install_skill`. |
| App moved | Nothing breaks; the copy is independent. |
| App updated | Next launch refreshes the copy. Until then the old CLI keeps working. |
| App deleted | Extension keeps working with the existing copy. |
| `install.sh` and app both used | Same path. `install_cli` never downgrades: it replaces when the target is missing, when the target's `--version` is lower than the sidecar's, or when versions are equal and SHA-256 differs; when the target is newer it keeps it and reports `KeptNewer`. `install.sh` always overwrites (the user asked for it). |

`install_cli(home, source: &Path) -> Result<CliInstallOutcome, InstallError>` where `CliInstallOutcome = Installed | Updated { from: String } | AlreadyCurrent | KeptNewer { installed: String }`. Copy to `bin/agentmark.tmp-<pid>`, chmod 0755, rename. Running the native host from the old file during rename is safe (rename does not touch open inodes).

`link_cli(home, local_bin: &Path) -> Result<LinkOutcome, InstallError>`: `local_bin/agentmark` → symlink to `~/.agentmark/bin/agentmark`. `LinkOutcome = Linked | AlreadyLinked | BlockedByFile`. Replaces an existing symlink; never replaces a regular file. `local_bin` default `~/.local/bin`; `AGENTMARK_LOCAL_BIN` overrides (same as `install.sh`).

`install-extension` registers `~/.agentmark/bin/agentmark` if it exists, else `std::env::current_exe()` canonicalized.

---

## 9. Liveness file

- Path: `~/.agentmark/extension-connection.json`.
- Shape:
```json
{"connected_at":"2026-11-12T09:21:36Z","extension_id":"abcdefghijklmnopabcdefghijklmnop","extension_version":"0.3.0","host_version":"0.3.0"}
```
`connected_at`: RFC 3339 UTC. `extension_id`: from argv origin, `null` if absent. `extension_version`: `null` until a `status` with `extension_version` arrives.
- Written by `setup::record_connection(home, &ConnectionRecord)`: once at native-host start (before the loop), and again when a `status` carries an `extension_version` different from the one this process last wrote. Only when `~/.agentmark/` already exists (never create it).
- Atomic: write `extension-connection.json.tmp-<pid>`, rename. Every error is logged at `debug` and swallowed; never delays or changes a response.
- App: `setup::read_connection(home) -> Option<ConnectionRecord>`; polls every 1000 ms only while the "Add the Chrome extension" screen is visible. "Connected" only when it returns `Some` with `connected_at` no older than the app's start (2 s slack for whole-second timestamps); the file stays after Chrome quits, so an older record shows "Last connected <time>" instead. Shows `extension_version` if present.

---

## 10. Filesystem map

| Path | Created by | Read by |
|---|---|---|
| `~/.agentmark/config.toml` | `setup::apply_settings` only | all commands, host, app |
| `~/.agentmark/config.toml.invalid-<ts>` | `apply_settings` (§5.3) | user |
| `~/.agentmark/index.db` (+ `-wal`, `-shm`) | `apply_settings`, `open_and_migrate` | CLI, host, app (count) |
| `~/.agentmark/logs/` | `logging::init` | user |
| `~/.agentmark/bin/agentmark` | `install.sh`, `setup::install_cli` | Chrome (manifest `path`), symlink |
| `~/.agentmark/native-host.json` | `install-extension`, `register_native_hosts` | same |
| `~/.agentmark/extension-connection.json` | native host | app |
| `~/.agentmark/extension/` | `install-extension` (development copy; only when the build embeds the extension) | developer (load unpacked) |
| `~/.agentmark/skill/SKILL.md` | `install_skill` | agents via symlinks |
| `~/.agents/skills/agentmark`, `~/.claude/skills/agentmark` | `install_skill` | agents |
| `~/.local/bin/agentmark` (symlink) | `install.sh`, `link_cli` | shell |
| `<browser>/NativeMessagingHosts/com.agentmark.native.json` | `register_native_hosts` | browser |
| storage folder (default `~/Documents/AgentMark`) | `apply_settings` | save, show, etc. |
| `*.tmp-<pid>` beside any atomically written file | the writer; removed by rename | — |

The app writes nothing of its own outside this list (Tauri's own WebView data dir aside).

---

## 11. `install.sh` and `agentmark init`

### 11.1 `install.sh`

- Flags: `--from-source`, `--extension-id ID`, `--help`; `--skip-init` accepted and ignored (prints nothing). Env: unchanged.
- Never runs `agentmark init` and never prompts (piped or not). DECISION: setup happens in Chrome or the app (spec decision), so the terminal asks nothing.
- Steps: install binary to `~/.agentmark/bin/agentmark` (download or build) → symlink → `agentmark install-extension [--extension-id ID]` → `agentmark add-skill`.
- `--from-source` (and repo-checkout builds): require `cargo` and `npm` (die with the missing tool's name); run `npm ci && npm run build` in `packages/extension` before `cargo build --release -p agentmark`, so the extension is embedded.
- The unpacked-extension steps print only when `install-extension` reports no extension IDs (only a build with an empty store ID), under the heading "Development: load the extension unpacked". The `pbcopy`/`open chrome://extensions` helpers stay in that branch only. After a successful registration, if `~/.agentmark/extension/manifest.json` exists, a short hint follows the final message: no extension in Chrome yet? load that folder unpacked (it carries the store key, so no ID is needed).
- Exit codes: `0` success (including "no extension ID known yet"); `1` binary not installed, or `install-extension` failed. Skill failure is a warning.
- Final message (success, IDs registered): `Done. Go back to Chrome: AgentMark finishes setting up there.` Without IDs: the development steps, then `Then run: agentmark install-extension --extension-id YOUR_EXTENSION_ID`.
- `install-extension` no longer errors when nothing is embedded: it skips extraction with a note and still registers.

### 11.2 `agentmark init`

- Uses `setup::read_state` + `setup::apply_settings`; no direct config writes.
- Interactive (no flags): if set up, ask `Update existing settings? [y/N]` (N → exit 0, "Nothing changed."). Prompts: `Default agent [claude/codex/pi] (<state default>): `, `Bookmark folder (<default display>): `, `Write a summary and tags after each save? [Y/n] ` (default from state). Relative folder input resolved against cwd first.
- Non-interactive: any of `--storage-path PATH`, `--agent NAME`, `--summaries on|off`, `--yes`. Missing values = state defaults (current or §5.1). No prompts. Existing settings are updated without asking.
- Output on success: `Initialized AgentMark:` then `  config:  …`, `  storage: …`, `  agent:   …`, `  summaries: on|off`.
- Validation errors: one line each to stderr `error: <field>: <message>`, exit `1`. Other failures: exit `1`. Clap usage errors: exit `2` (clap default).

---

## 12. Versioning

All carry one version (0.3.0 for this branch); tests fail on mismatch:

1. `packages/cli/Cargo.toml`
2. `packages/extension/manifest.json`
3. `packages/extension/package.json`
4. `packages/extension/package-lock.json` (two root entries)
5. `packages/app/src-tauri/Cargo.toml`
6. `packages/app/src-tauri/tauri.conf.json` (`version`)
7. `packages/app/package.json`
8. `packages/app/package-lock.json` (two root entries)

`Cargo.lock` follows on build. Workspace: root `Cargo.toml` gets `members = ["packages/cli", "packages/app/src-tauri"]` and DECISION `default-members = ["packages/cli"]` so root `cargo build/clippy/test` on Linux CI does not need WebKit; the app job uses `-p agentmark-app`.

App identity: crate `agentmark-app`; `productName` `AgentMark`; `mainBinaryName` `agentmark-app` (must differ from the sidecar `agentmark`, both live in `Contents/MacOS/`); `identifier` `app.agentmark.desktop` (owner-confirmed; changing it after the first signed release breaks updates). Sidecar: `bundle.externalBin: ["binaries/agentmark"]`, source file `packages/app/src-tauri/binaries/agentmark-<target triple>` built from `-p agentmark` after the extension build; runtime path `current_exe().parent()/agentmark`. The app does not execute the sidecar (no shell plugin needed); it only copies it.

---

## 13. Risks / do not assume

- `chrome.runtime.lastError` strings in `connection.ts` are from docs; confirm in a live build (§4.2 depends on them).
- `osascript` dialog may open behind Chrome even with `activate`; confirm on macOS. Fallback is the typed path.
- Without `tabs`, `sender.url` for content-script messages is assumed present (content-script matches act as host access); confirm in a dev build before relying on `report_page`.
- The FIFO port misaligns responses after any 30 s timeout (pre-existing). Do not add long-running requests to the port.
- Two writers (app and host) can both write `config.toml`; last write wins, never torn. Do not cache config across requests.
- Release `gh release create --draft` means `releases/latest` does not see a release until the owner publishes it; the pre-built command fails until then.
- Rosetta-translated Chrome on Apple Silicon reports `x86-64` and is shown as Mac Intel.
- Folder change leaves old bundles unreachable by `show` (§5.4); dedup of a re-saved old URL may fail to find its bundle. Out of scope; do not "fix" by moving files.
- Brave/Arc manifest paths unconfirmed (§8.2).
- Quarantine/Gatekeeper on the copied CLI from an unsigned app build is untested.

---

## 14. Builder checklists

### CLI (`packages/cli`, `install.sh`, root `Cargo.toml`)
- [ ] `setup` module with every type and function in §1, §6, §8, §9; `toml_edit` dependency.
- [ ] `main.rs` passes origin to the host; liveness writes (§9).
- [ ] Messages §2.1–2.5: allow-list, exhaustive dispatch, `ErrorCode`, `setup_required` gate, `status` fields, one-shot-safe `choose_folder` with `FolderDialog` seam.
- [ ] `install-extension`: IDs file, migration, multi-browser registration, stable path, no-embed tolerance (§8, §11.1).
- [ ] `init` rewrite (§11.2), default folder `~/Documents/AgentMark`.
- [ ] `install.sh` changes (§11.1) + fake-cargo/npm tests.
- [ ] `delete` must not print "Deleted" when the row vanished; clippy `--all-targets` fixes in `tests/save_tests.rs`, `tests/skill_tests.rs`.
- [ ] `default-members` (§12); release/CI commands updated to `cargo clippy --all-targets -- -D warnings`.
- [ ] Tests from spec §3 incl. the end-to-end native-protocol test (status → get_setup_state → save_settings → save → lookup_urls → liveness file).

### Extension (`packages/extension`)
- [ ] Types and parsers §3.1; `sendOneShot`; runtime messages §3.2–3.5; `RuntimeErrorResponse.code`.
- [ ] Constants §7.1 in `constants.ts`; `platforms.ts` table and commands §7.2–7.3; remove `INSTALL_COMMAND`; fix `SETUP_GUIDE_URL`.
- [ ] `AppState` derivation and polling §4; setup draft §4.4; screens A–D per spec.
- [ ] Settings view: Bookmarks/Settings switch, `agentmark-settings` menu item, `agentmark_sidepanel_view`.
- [ ] `tabs` → `optional_permissions`; toolbar icon modes: listeners registered at module scope, gated by a `tabsAllowed` flag from `permissions.contains` + `onAdded`/`onRemoved`; `reportPage` path; revoke clears caches.
- [ ] Content scripts send `report_page` (local literal, sync test).
- [ ] `chrome-mock.ts`: `sendNativeMessage`, `permissions.*`, `storage.session`, `runtime.getPlatformInfo`.
- [ ] Mock native responses exactly as the JSON in §2.

### App (`packages/app`)
- [ ] Workspace member, identity and sidecar per §12; version files §12.
- [ ] Launch steps calling `setup::install_cli` (source = sidecar), `link_cli`, `register_native_hosts(home, ~/.agentmark/bin/agentmark)`, `install_skill`; each step reports its outcome/error with retry.
- [ ] Setup and Settings screens call `read_state` / `apply_settings`; show `FieldError`s by `field`; folder via `tauri-plugin-dialog`, not the CLI seam.
- [ ] "Add the Chrome extension": `WEB_STORE_URL` button or development steps when empty; liveness polling §9.
- [ ] Constants in `src/constants.ts` (§7.1); single instance; updater with empty key/endpoints = off.

---

## 15. Decisions, owner items, spec deviations

**DECISION list:** `choose_folder` via one-shot `sendNativeMessage` (§2.4); chosen folder handed to the popup through the setup draft (§2.4, §4.4); setup required as `error.code` (§2.5); old `status_result` without `initialized` = ready, `unknown message type` = `update_required` (§2.6, §4.2); `status.extension_version` carries the extension version for liveness (§2.1); install ping on install/startup (§3.5); polling also on screen A (§4.3); relative storage paths rejected by the core (§5.1); `toml_edit` targeted edit, invalid TOML moved aside (§5.3); `--extension-id` in the command whenever the runtime ID is not the store ID (§7.2); unmatched platform → no highlighted row (§7.3); separate `MAC_INTEL_APP_DOWNLOAD_URL` (§7.1); dev IDs in `native-host.json` with migration from existing manifests (§8.1); registered path is the stable copy `~/.agentmark/bin/agentmark`, never downgraded by the app (§8.3); `install.sh` never prompts or runs init (§11.1); `default-members` (§12); toolbar tab listeners always registered, gated by a flag (§14).

**OWNER items (safe default in brackets):** Brave/Arc manifest paths [write only when the browser's user-data dir exists]; app `identifier` [`app.agentmark.desktop`]; all empty URL constants in §7.1 [empty, UI hides the option] (the store ID is set).

**Spec deviations:**
- Spec E: "Register tab listeners only when allowed". MV3 only wakes the worker for listeners registered synchronously at top level, so listeners are always registered and gated by a permission flag.
- Spec B: "While the store ID constant is empty the command includes `--extension-id`". Here it is included whenever the running ID differs from the store ID, which also covers a developer's unpacked copy after the store ID is set.
- Spec C/§3 "Choose a folder": the popup closes when the system dialog takes focus, so the result reaches it through `agentmark_setup_draft` rather than the reply.
- Spec §3 install.sh "asks no questions when piped": it now asks none at all, matching the decision that setup never happens in a terminal.
