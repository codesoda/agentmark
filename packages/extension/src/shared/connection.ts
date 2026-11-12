/**
 * Classifies errors from the extension ↔ AgentMark CLI path so the popup and
 * side panel can show the right fix: install the app, register this
 * extension, or reload the extension. Pure; no Chrome API access.
 */

/** The one-line installer from the README and `install.sh`. */
export const INSTALL_COMMAND =
  "curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash";

/** Public setup guide, opened from the install prompt. */
export const SETUP_GUIDE_URL = "https://agentmark.vercel.app/#install";

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
