/**
 * Native responses for tests, copied from the JSON examples in
 * docs/setup-contract.md §2 so tests exercise the real wire shapes.
 */

import type { RuntimeResponse, SetupState } from "../shared/types";

/** `setup_state_result.state` from contract §2.2, verbatim. */
export const CONTRACT_SETUP_STATE = {
  initialized: true,
  config_problem: null,
  version: "0.3.0",
  platform: { os: "macos", arch: "aarch64" },
  settings: {
    storage_path: "/Users/me/Documents/AgentMark",
    storage_path_display: "~/Documents/AgentMark",
    default_agent: "claude",
    enrichment_enabled: true,
    system_prompt: null,
    snapshot_sites: ["example.com/app/"],
  },
  default_storage_path: "/Users/me/Documents/AgentMark",
  default_storage_path_display: "~/Documents/AgentMark",
  agents: [
    { id: "claude", name: "Claude Code", found: true, path: "/Users/me/.local/bin/claude" },
    { id: "codex", name: "Codex", found: false, path: null },
    { id: "pi", name: "pi", found: false, path: null },
  ],
  builtin_snapshot_sites: [
    "linkedin.com/in/",
    "linkedin.com/posts/",
    "linkedin.com/feed/update/",
    "linkedin.com/company/",
    "x.com",
    "twitter.com",
    "news.ycombinator.com/item",
    "reddit.com",
  ],
  bookmark_count: 42,
} as const;

type DeepPartialState = Partial<Omit<SetupState, "settings">> & { settings?: Partial<SetupState["settings"]> };

/** A setup state: the contract example with overrides (settings merged). */
export function setupState(overrides: DeepPartialState = {}): SetupState {
  const base = JSON.parse(JSON.stringify(CONTRACT_SETUP_STATE)) as SetupState;
  return { ...base, ...overrides, settings: { ...base.settings, ...overrides.settings } };
}

/** The state a fresh, not-set-up computer reports (CLI defaults from contract §5.1). */
export function notSetUpState(overrides: DeepPartialState = {}): SetupState {
  return setupState({
    initialized: false,
    bookmark_count: null,
    ...overrides,
    settings: { snapshot_sites: [], ...overrides.settings },
  });
}

export const statusReady: RuntimeResponse = {
  success: true,
  data: { type: "status_result", ok: true, version: "0.3.0", initialized: true },
};
export const statusNotSetUp: RuntimeResponse = {
  success: true,
  data: { type: "status_result", ok: true, version: "0.3.0", initialized: false },
};
/** A CLI from before 0.3.0: no `initialized`. */
export const statusOldCli: RuntimeResponse = {
  success: true,
  data: { type: "status_result", ok: true, version: "0.2.6" },
};
export const hostNotFound: RuntimeResponse = { success: false, error: "Specified native messaging host not found." };
export const hostForbidden: RuntimeResponse = {
  success: false,
  error: "Access to the specified native messaging host is forbidden.",
};
/**
 * What the 0.2.6 CLI really answers when it is not set up (review finding #5):
 * `save` has no prefix; `list`, `lookup_urls` and the rest add "not initialized: ".
 */
export const OLD_CLI_NOT_SET_UP_SAVE =
  "config not found at /Users/me/.agentmark/config.toml \u2014 run `agentmark init` first";
export const OLD_CLI_NOT_SET_UP_LIST = `not initialized: ${OLD_CLI_NOT_SET_UP_SAVE}`;

/** Contract §2.5, verbatim. */
export const setupRequired: RuntimeResponse = {
  success: false,
  error: "not initialized: AgentMark is not set up yet. Finish setup in the extension or the AgentMark app.",
  code: "setup_required",
};

export function setupStateResult(state: SetupState): RuntimeResponse {
  return { success: true, data: { type: "setup_state_result", state } };
}

export function saveSettingsResult(state: SetupState): RuntimeResponse {
  return { success: true, data: { type: "save_settings_result", state } };
}

/** Contract §2.3 example, verbatim. */
export const settingsInvalid: RuntimeResponse = {
  success: true,
  data: {
    type: "settings_invalid",
    errors: [
      { field: "storage_path", index: null, code: "not_writable", message: "AgentMark cannot write to /Volumes/X." },
      { field: "snapshot_sites", index: 2, code: "invalid_site", message: "\"exa mple.com\" is not a host/path rule." },
    ],
  },
};

/** Contract §2.4, verbatim. */
export const folderChosen: RuntimeResponse = {
  success: true,
  data: { type: "choose_folder_result", outcome: "chosen", path: "/Users/me/Bookmarks", path_display: "~/Bookmarks", reason: null },
};
export const folderCancelled: RuntimeResponse = {
  success: true,
  data: { type: "choose_folder_result", outcome: "cancelled", path: null, path_display: null, reason: null },
};
export const folderUnavailable: RuntimeResponse = {
  success: true,
  data: {
    type: "choose_folder_result",
    outcome: "unavailable",
    path: null,
    path_display: null,
    reason: "No folder dialog is available on this system.",
  },
};
