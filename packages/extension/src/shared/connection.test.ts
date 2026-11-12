import { describe, it, expect } from "vitest";
import {
  appStateFromResult,
  appStatusFromProbe,
  classifyConnectionError,
  registerCommand,
  POLLING_STATES,
} from "./connection";
import { isConnectionError } from "./runtime";
import { OLD_CLI_NOT_SET_UP_LIST, OLD_CLI_NOT_SET_UP_SAVE } from "../test/fixtures";

describe("classifyConnectionError", () => {
  it.each([
    // Chrome's native messaging errors, as surfaced through port.onDisconnect.
    ["Specified native messaging host not found.", "not_installed"],
    ["specified native messaging host not found", "not_installed"],
    ["Failed to start native messaging host.", "not_installed"],
    ["Access to the specified native messaging host is forbidden.", "forbidden"],
    ["ACCESS TO THE SPECIFIED NATIVE MESSAGING HOST IS FORBIDDEN", "forbidden"],
    ["Native host has exited.", "host_crashed"],
    ["Error when communicating with the native messaging host.", "host_crashed"],
    // NativeMessagingClient's own fallbacks.
    ["Native host disconnected", "host_crashed"],
    ["Native host not connected", "host_crashed"],
    // chrome.runtime.sendMessage failures between popup/side panel and the service worker.
    ["Could not establish connection. Receiving end does not exist.", "worker_unreachable"],
    ["could not establish connection", "worker_unreachable"],
    ["The message port closed before a response was received.", "worker_unreachable"],
  ] as const)("classifies %j as %s", (error, kind) => {
    expect(classifyConnectionError(error)).toBe(kind);
  });

  it.each([
    "Failed to save bookmark",
    "Native host request timed out",
    OLD_CLI_NOT_SET_UP_LIST,
    "Unsupported or missing URL",
    "",
  ])("returns null for unrelated error %j", (error) => {
    expect(classifyConnectionError(error)).toBeNull();
  });

  it("keeps isConnectionError in step with the classifier", () => {
    expect(isConnectionError("Specified native messaging host not found.")).toBe(true);
    expect(isConnectionError("Failed to save bookmark")).toBe(false);
  });
});

describe("connection constants", () => {
  it("fills the extension id into the register command", () => {
    expect(registerCommand("abcdef")).toBe("agentmark install-extension --extension-id abcdef");
    expect(registerCommand(undefined)).toBe(
      "agentmark install-extension --extension-id YOUR_EXTENSION_ID",
    );
  });
});

describe("appStatusFromProbe (contract §4.2)", () => {
  const status = (extra: Record<string, unknown>) =>
    ({ success: true, data: { type: "status_result", ok: true, version: "0.3.0", ...extra } }) as const;

  it("is ready when the app says it is set up", () => {
    expect(appStatusFromProbe(status({ initialized: true }) as never)).toEqual({ state: "ready", version: "0.3.0" });
  });

  it("is setup_required when the app says it is not set up", () => {
    expect(appStatusFromProbe(status({ initialized: false }) as never).state).toBe("setup_required");
  });

  it("treats an old CLI without `initialized` as ready", () => {
    expect(appStatusFromProbe(status({}) as never).state).toBe("ready");
  });

  it.each([
    ["Specified native messaging host not found.", "not_installed"],
    ["Access to the specified native messaging host is forbidden.", "forbidden"],
    ["Could not establish connection. Receiving end does not exist.", "worker_unreachable"],
    ["Native host has exited.", "host_crashed"],
    // Unclassified probe failures (e.g. a timeout) mean the host is broken.
    ["Native host request timed out", "host_crashed"],
  ] as const)("maps the probe error %j to %s", (error, state) => {
    expect(appStatusFromProbe({ success: false, error })).toEqual({ state, detail: error });
  });

  it("is host_crashed when the probe answers with another type", () => {
    const result = appStatusFromProbe({ success: true, data: { type: "list_collections_result", collections: [] } });
    expect(result.state).toBe("host_crashed");
  });

  it("polls only while the app is missing or registered for another extension", () => {
    expect([...POLLING_STATES].sort()).toEqual(["forbidden", "not_installed"]);
  });
});

describe("appStateFromResult (later requests)", () => {
  it("moves to setup_required on the setup_required code", () => {
    expect(appStateFromResult({ success: false, error: "whatever", code: "setup_required" })).toBe("setup_required");
  });

  // Real answers of the 0.2.6 CLI when it is not set up (review finding #5).
  it("moves to setup_required on an old CLI's 'not initialized' error (list, lookup_urls)", () => {
    expect(appStateFromResult({ success: false, error: OLD_CLI_NOT_SET_UP_LIST })).toBe("setup_required");
  });

  it("moves to setup_required on an old CLI's unprefixed save error", () => {
    expect(appStateFromResult({ success: false, error: OLD_CLI_NOT_SET_UP_SAVE })).toBe("setup_required");
    expect(appStatusFromProbe({ success: false, error: OLD_CLI_NOT_SET_UP_SAVE }).state).toBe("setup_required");
  });

  it("does not treat other config errors as setup required", () => {
    expect(appStateFromResult({ success: false, error: "failed to parse config: expected `=`" })).toBeNull();
    expect(appStateFromResult({ success: false, error: "bundle creation failed: config not found in bundle" })).toBeNull();
  });

  it("moves to update_required when a setup message is unknown to the CLI", () => {
    const old = { success: false as const, error: "unknown message type: get_setup_state" };
    expect(appStateFromResult(old, true)).toBe("update_required");
    // The same error from an ordinary request is not about the app's age.
    expect(appStateFromResult(old)).toBeNull();
  });

  it("moves to the classified connection problem", () => {
    expect(appStateFromResult({ success: false, error: "Specified native messaging host not found." })).toBe("not_installed");
  });

  it("leaves the state alone for successes and ordinary failures", () => {
    expect(appStateFromResult({ success: false, error: "Failed to save bookmark" })).toBeNull();
    expect(appStateFromResult({ success: true, data: { type: "list_collections_result", collections: [] } })).toBeNull();
  });
});
