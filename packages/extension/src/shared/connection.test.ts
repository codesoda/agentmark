import { describe, it, expect } from "vitest";
import {
  classifyConnectionError,
  registerCommand,
  INSTALL_COMMAND,
  SETUP_GUIDE_URL,
} from "./connection";
import { isConnectionError } from "./runtime";

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
    "not initialized: config not found",
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
  it("uses the README's install command", () => {
    expect(INSTALL_COMMAND).toBe(
      "curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash",
    );
  });

  it("points at the setup guide", () => {
    expect(SETUP_GUIDE_URL).toBe("https://agentmark.vercel.app/#install");
  });

  it("fills the extension id into the register command", () => {
    expect(registerCommand("abcdef")).toBe("agentmark install-extension --extension-id abcdef");
    expect(registerCommand(undefined)).toBe(
      "agentmark install-extension --extension-id YOUR_EXTENSION_ID",
    );
  });
});
