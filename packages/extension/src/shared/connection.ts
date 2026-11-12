/**
 * Classifies errors from the extension ↔ AgentMark CLI path so the popup and
 * side panel can show the right fix: install the app, register this
 * extension, set it up, update it, or reload the extension. Also derives the
 * app state (docs/setup-contract.md §4). Pure; no Chrome API access.
 */

import type { RuntimeResponse } from "./types";

/** Shown in the register command when the extension ID is unavailable. */
export const EXTENSION_ID_PLACEHOLDER = "YOUR_EXTENSION_ID";

/**
 * Why the extension could not reach the AgentMark CLI.
 * - `not_installed`: no native host manifest for this name (or its binary is missing).
 * - `forbidden`: the host is registered, but for a different extension ID.
 * - `worker_unreachable`: the popup/side panel could not reach the service worker.
 * - `host_crashed`: the host started and then exited or disconnected.
 */
export type ConnectionProblem =
  | "not_installed"
  | "forbidden"
  | "worker_unreachable"
  | "host_crashed";

// Checked in order; the first kind with a matching (lower-case) fragment wins.
const PATTERNS: ReadonlyArray<[ConnectionProblem, readonly string[]]> = [
  [
    "not_installed",
    [
      // Chrome: "Specified native messaging host not found."
      "native messaging host not found",
      // Chrome: the manifest exists but its binary path does not run.
      "failed to start native messaging host",
    ],
  ],
  // Chrome: "Access to the specified native messaging host is forbidden."
  ["forbidden", ["native messaging host is forbidden"]],
  [
    "worker_unreachable",
    [
      // Chrome: "Could not establish connection. Receiving end does not exist."
      "could not establish connection",
      "receiving end does not exist",
      // Chrome: "The message port closed before a response was received."
      "message port closed",
    ],
  ],
  [
    "host_crashed",
    [
      // Chrome: "Native host has exited."
      "native host has exited",
      // Chrome: "Error when communicating with the native messaging host."
      "error when communicating with the native messaging host",
      // NativeMessagingClient: "Native host disconnected", "Native host not connected".
      "disconnected",
      "not connected",
    ],
  ],
];

/** Map an error message to a connection problem, or null if it is some other failure. */
export function classifyConnectionError(error: string): ConnectionProblem | null {
  const lower = error.toLowerCase();
  for (const [kind, fragments] of PATTERNS) {
    if (fragments.some((fragment) => lower.includes(fragment))) return kind;
  }
  return null;
}

/** The command that registers the native host for this extension ID. */
export function registerCommand(extensionId: string | undefined): string {
  return `agentmark install-extension --extension-id ${extensionId || EXTENSION_ID_PLACEHOLDER}`;
}

/**
 * Where the app stands, as the popup and side panel see it (contract §4.1).
 * The four `ConnectionProblem` kinds are a subset.
 */
export type AppState =
  | "checking"
  | ConnectionProblem
  | "update_required"
  | "setup_required"
  | "ready";

/** States in which an open surface keeps probing every `POLL_INTERVAL_MS` (contract §4.3). */
export const POLLING_STATES: ReadonlySet<AppState> = new Set<AppState>(["not_installed", "forbidden"]);

/** Gap between the end of one status probe and the start of the next. */
export const POLL_INTERVAL_MS = 2000;

/** Prefix of the CLI's "not set up" error, also sent by CLIs that predate `code`. */
const NOT_INITIALIZED_PREFIX = "not initialized";
/**
 * What a CLI older than 0.3.0 answers a `save` with when it is not set up:
 * `config not found at <path> — run \`agentmark init\` first`, with no prefix.
 */
const OLD_CONFIG_MISSING = /^config not found at .*run `agentmark init` first\s*$/is;
/** Prefix of the error an older CLI returns for a message it does not know. */
const UNKNOWN_MESSAGE_PREFIX = "unknown message type";

export interface AppStatus {
  state: AppState;
  /** The app's version, from the last successful status probe. */
  version?: string;
  /** The raw error behind a problem state, shown collapsed for troubleshooting. */
  detail?: string;
}

/** Whether a failed request means "set up first": the code (0.3.0+), the "not initialized" prefix, or an old CLI's bare `save` error. */
export function isSetupRequiredError(error: string, code?: string): boolean {
  return (
    code === "setup_required" ||
    error.toLowerCase().startsWith(NOT_INITIALIZED_PREFIX) ||
    OLD_CONFIG_MISSING.test(error)
  );
}

/** Whether a failed request means the CLI predates the message. */
export function isUnknownMessageError(error: string): boolean {
  return error.toLowerCase().startsWith(UNKNOWN_MESSAGE_PREFIX);
}

/** The state a `status` probe's result puts the app in (contract §4.2). */
export function appStatusFromProbe(response: RuntimeResponse): AppStatus {
  if (!response.success) {
    if (isSetupRequiredError(response.error, response.code)) {
      return { state: "setup_required", detail: response.error };
    }
    return { state: classifyConnectionError(response.error) ?? "host_crashed", detail: response.error };
  }
  if (response.data.type !== "status_result") {
    return { state: "host_crashed", detail: `Unexpected response type: ${response.data.type}` };
  }
  return {
    // A CLI older than 0.3.0 does not say; it is ready or will answer "not initialized" later.
    state: response.data.initialized === false ? "setup_required" : "ready",
    version: response.data.version,
  };
}

/**
 * The state any later request's result moves the app to, or null when the
 * result says nothing about the app (a success, or an ordinary failure).
 * `setupMessage`: the request was `get_setup_state` or `save_settings`, so an
 * "unknown message type" answer means the CLI is too old.
 */
export function appStateFromResult(response: RuntimeResponse, setupMessage = false): AppState | null {
  if (response.success) return null;
  if (isSetupRequiredError(response.error, response.code)) return "setup_required";
  if (setupMessage && isUnknownMessageError(response.error)) return "update_required";
  return classifyConnectionError(response.error);
}
