import { describe, it, expect, vi, beforeEach } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";

vi.mock("../shared/native-messaging", () => {
  const mockClient = {
    sendRequest: vi.fn(),
    sendOneShot: vi.fn(),
    getStatus: vi.fn().mockReturnValue("connected"),
    disconnect: vi.fn(),
  };
  return {
    getNativeClient: vi.fn(() => mockClient),
    resetNativeClient: vi.fn(),
    NativeMessagingClient: vi.fn(),
  };
});

import {
  CHOOSE_FOLDER_TIMEOUT_MS,
  ensureContextMenu,
  handleContextMenuClick,
  handleNativeRequest,
  handleRuntimeMessage,
  SETTINGS_MENU_ID,
  SIDEPANEL_MENU_ID,
  toolbarIcon,
} from "./service-worker";
import { getNativeClient } from "../shared/native-messaging";
import { SETUP_DRAFT_KEY, SIDEPANEL_VIEW_KEY } from "../shared/constants";
import type { RuntimeMessage, RuntimeResponse } from "../shared/types";
import { CONTRACT_SETUP_STATE } from "../test/fixtures";

// Captured at import, before beforeEach replaces the mock.
const registered = {
  onInstalled: vi.mocked(chrome.runtime.onInstalled.addListener).mock.calls[0]?.[0] as () => void,
  onStartup: vi.mocked(chrome.runtime.onStartup.addListener).mock.calls[0]?.[0] as () => void,
  permissionsAdded: vi.mocked(chrome.permissions.onAdded.addListener).mock.calls[0]?.[0],
  permissionsRemoved: vi.mocked(chrome.permissions.onRemoved.addListener).mock.calls[0]?.[0],
  tabsUpdated: vi.mocked(chrome.tabs.onUpdated.addListener).mock.calls[0]?.[0],
};

const client = getNativeClient() as unknown as {
  sendRequest: ReturnType<typeof vi.fn>;
  sendOneShot: ReturnType<typeof vi.fn>;
};

function send(message: unknown, sender: chrome.runtime.MessageSender = {}): Promise<unknown> {
  return new Promise((resolve) => {
    handleRuntimeMessage(message as RuntimeMessage, sender, resolve as (r: RuntimeResponse) => void);
  });
}

let chromeMock: ReturnType<typeof resetChromeMock>;

beforeEach(() => {
  chromeMock = resetChromeMock();
  vi.clearAllMocks();
});

describe("listeners registered at top level", () => {
  it("keeps the tab listeners and adds the permission listeners", () => {
    expect(registered.tabsUpdated).toBe(toolbarIcon.handleTabUpdated);
    expect(registered.permissionsAdded).toBe(toolbarIcon.handlePermissionsAdded);
    expect(registered.permissionsRemoved).toBe(toolbarIcon.handlePermissionsRemoved);
  });

  it.each(["onInstalled", "onStartup"] as const)("pings the app with one one-shot status on %s (liveness)", async (event) => {
    client.sendOneShot.mockResolvedValue({ type: "status_result", ok: true, version: "0.3.0", initialized: true });
    registered[event]();
    await Promise.resolve();
    // The host gets the extension's version and writes the liveness file (contract §3.5, §9).
    expect(client.sendOneShot).toHaveBeenCalledTimes(1);
    expect(client.sendOneShot).toHaveBeenCalledWith({ type: "status", extension_version: "0.3.0" }, expect.any(Number));
    // No port is opened, so neither the host nor the worker stays alive (review finding #9).
    expect(client.sendRequest).not.toHaveBeenCalled();
  });

  it("ignores a failed ping", async () => {
    client.sendOneShot.mockRejectedValue(new Error("Specified native messaging host not found."));
    expect(() => registered.onInstalled()).not.toThrow();
    await Promise.resolve();
});
});

describe("Settings in the toolbar icon's menu", () => {
  it("creates the item next to the side panel item", () => {
    ensureContextMenu();
    const ids = chromeMock.contextMenus.create.mock.calls.map(([item]) => (item as { id: string }).id);
    expect(ids.indexOf(SETTINGS_MENU_ID)).toBe(ids.indexOf(SIDEPANEL_MENU_ID) + 1);
    expect(chromeMock.contextMenus.create).toHaveBeenCalledWith({
      id: "agentmark-settings",
      title: "Settings",
      contexts: ["action"],
    });
  });

  it("asks for the Settings view and opens the panel inside the click, without awaiting", () => {
    handleContextMenuClick({ menuItemId: SETTINGS_MENU_ID, editable: false } as chrome.contextMenus.OnClickData, {
      windowId: 7,
    } as chrome.tabs.Tab);

    // Both calls happen synchronously, so the user gesture still holds for sidePanel.open.
    expect(chromeMock.storage.session.set).toHaveBeenCalledWith({ [SIDEPANEL_VIEW_KEY]: "settings" });
    expect(chromeMock.sidePanel.open).toHaveBeenCalledWith({ windowId: 7 });
    expect(chromeMock.tabs.query).not.toHaveBeenCalled();
  });

  it("falls back to the current window when the menu gives no tab", async () => {
    chromeMock.tabs.query.mockResolvedValue([{ windowId: 3 }]);
    handleContextMenuClick({ menuItemId: SETTINGS_MENU_ID, editable: false } as chrome.contextMenus.OnClickData);
    await vi.waitFor(() => expect(chromeMock.sidePanel.open).toHaveBeenCalledWith({ windowId: 3 }));
  });

  it("survives sidePanel.open rejecting", () => {
    chromeMock.sidePanel.open.mockRejectedValue(new Error("no gesture"));
    expect(() =>
      handleContextMenuClick({ menuItemId: SETTINGS_MENU_ID, editable: false } as chrome.contextMenus.OnClickData, {
        windowId: 7,
      } as chrome.tabs.Tab),
    ).not.toThrow();
  });
});

describe("setup messages (contract §3.2)", () => {
  it("adds this extension's version to status", async () => {
    client.sendRequest.mockResolvedValue({ type: "status_result", ok: true, version: "0.3.0", initialized: false });
    const response = await send({ type: "status" });
    expect(client.sendRequest).toHaveBeenCalledWith({ type: "status", extension_version: "0.3.0" });
    expect(response).toEqual({
      success: true,
      data: { type: "status_result", ok: true, version: "0.3.0", initialized: false },
    });
  });

  it("forwards get_setup_state on the port", async () => {
    client.sendRequest.mockResolvedValue({ type: "setup_state_result", state: CONTRACT_SETUP_STATE });
    const response = await send({ type: "get_setup_state" });
    expect(client.sendRequest).toHaveBeenCalledWith({ type: "get_setup_state" });
    expect((response as { data: { type: string } }).data.type).toBe("setup_state_result");
  });

  it("forwards save_settings unchanged and clears the setup draft on success", async () => {
    chromeMock.storage.session.data[SETUP_DRAFT_KEY] = { storage_path: "~/X", updated_at: Date.now() };
    client.sendRequest.mockResolvedValue({ type: "save_settings_result", state: CONTRACT_SETUP_STATE });
    const settings = { storage_path: "~/Documents/AgentMark", system_prompt: null };
    await send({ type: "save_settings", settings });
    expect(client.sendRequest).toHaveBeenCalledWith({ type: "save_settings", settings });
    expect(chromeMock.storage.session.data[SETUP_DRAFT_KEY]).toBeUndefined();
  });

  it("keeps the draft when settings are invalid", async () => {
    chromeMock.storage.session.data[SETUP_DRAFT_KEY] = { storage_path: "~/X", updated_at: Date.now() };
    client.sendRequest.mockResolvedValue({
      type: "settings_invalid",
      errors: [{ field: "storage_path", index: null, code: "blank", message: "Choose a folder." }],
    });
    const response = await send({ type: "save_settings", settings: { storage_path: "" } });
    expect((response as RuntimeResponse).success).toBe(true);
    expect(chromeMock.storage.session.data[SETUP_DRAFT_KEY]).toBeDefined();
  });

  it("rejects save_settings without a settings object", async () => {
    const response = await send({ type: "save_settings", settings: ["nope"] });
    expect(response).toEqual({ success: false, error: "save_settings needs a settings object" });
    expect(client.sendRequest).not.toHaveBeenCalled();
  });

  it("copies the setup_required code onto the runtime error", async () => {
    client.sendRequest.mockResolvedValue({
      type: "error",
      message: "not initialized: AgentMark is not set up yet. Finish setup in the extension or the AgentMark app.",
      code: "setup_required",
    });
    const response = await handleNativeRequest({ type: "list" });
    expect(response).toEqual({
      success: false,
      error: "not initialized: AgentMark is not set up yet. Finish setup in the extension or the AgentMark app.",
      code: "setup_required",
    });
  });
});

describe("choose_folder (contract §2.4)", () => {
  it("uses a one-shot host with the long timeout, never the port", async () => {
    client.sendOneShot.mockResolvedValue({
      type: "choose_folder_result",
      outcome: "cancelled",
      path: null,
      path_display: null,
      reason: null,
    });
    await send({ type: "choose_folder", start_path: "~/Documents" });
    expect(client.sendOneShot).toHaveBeenCalledWith({ type: "choose_folder", start_path: "~/Documents" }, 310_000);
    expect(CHOOSE_FOLDER_TIMEOUT_MS).toBe(310_000);
    expect(client.sendRequest).not.toHaveBeenCalled();
  });

  it("omits an empty start path", async () => {
    client.sendOneShot.mockResolvedValue({ type: "choose_folder_result", outcome: "cancelled", path: null, path_display: null, reason: null });
    await send({ type: "choose_folder", start_path: "  " });
    expect(client.sendOneShot).toHaveBeenCalledWith({ type: "choose_folder" }, 310_000);
  });

  it("writes a chosen folder into the setup draft before answering (the popup may have closed)", async () => {
    client.sendOneShot.mockResolvedValue({
      type: "choose_folder_result",
      outcome: "chosen",
      path: "/Users/me/Bookmarks",
      path_display: "~/Bookmarks",
      reason: null,
    });
    const response = await send({ type: "choose_folder" });
    const draft = chromeMock.storage.session.data[SETUP_DRAFT_KEY] as Record<string, unknown>;
    expect(draft.storage_path).toBe("~/Bookmarks");
    expect(typeof draft.updated_at).toBe("number");
    expect((response as { data: { outcome: string } }).data.outcome).toBe("chosen");
  });

  it("answers unavailable when the CLI is too old for the dialog", async () => {
    client.sendOneShot.mockResolvedValue({ type: "error", message: "unknown message type: choose_folder" });
    const response = await send({ type: "choose_folder" });
    expect(response).toEqual({
      success: true,
      data: {
        type: "choose_folder_result",
        outcome: "unavailable",
        path: null,
        path_display: null,
        reason: "Update AgentMark to use the folder dialog.",
      },
    });
  });

  it("passes connection errors through", async () => {
    client.sendOneShot.mockRejectedValue(new Error("Specified native messaging host not found."));
    const response = await send({ type: "choose_folder" });
    expect(response).toEqual({ success: false, error: "Specified native messaging host not found." });
  });
});

describe("report_page (contract §3.3)", () => {
  const URL_X = "https://x.com/ada/status/1";
  const tabSender = (overrides: Partial<chrome.runtime.MessageSender> = {}): chrome.runtime.MessageSender => ({
    tab: { id: 5 } as chrome.tabs.Tab,
    frameId: 0,
    url: URL_X,
    ...overrides,
  });

  it("hands a valid report to the toolbar icon", async () => {
    const spy = vi.spyOn(toolbarIcon, "reportPage").mockResolvedValue();
    const response = await send({ type: "report_page", url: URL_X }, tabSender());
    expect(response).toEqual({ ok: true });
    expect(spy).toHaveBeenCalledWith(5, URL_X);
    spy.mockRestore();
  });

  it.each([
    ["no tab", { tab: undefined }, URL_X],
    ["a sub-frame", { frameId: 2 }, URL_X],
    ["a URL the frame is not on", {}, "https://x.com/someone/else"],
    ["an unsupported URL", { url: "chrome://settings" }, "chrome://settings"],
  ] as const)("ignores a report from %s", async (_name, overrides, url) => {
    const spy = vi.spyOn(toolbarIcon, "reportPage").mockResolvedValue();
    const response = await send({ type: "report_page", url }, tabSender(overrides as Partial<chrome.runtime.MessageSender>));
    expect(response).toEqual({ ok: false });
    expect(spy).not.toHaveBeenCalled();
    spy.mockRestore();
  });

  it("ignores a non-string URL", async () => {
    const spy = vi.spyOn(toolbarIcon, "reportPage").mockResolvedValue();
    const response = await send({ type: "report_page", url: 42 }, tabSender());
    expect(response).toEqual({ ok: false });
    expect(spy).not.toHaveBeenCalled();
    spy.mockRestore();
  });
});

describe("messages from content scripts (review finding #8)", () => {
  const page = "https://www.reddit.com/r/programming/";
  const contentScript = { tab: { id: 7 } as chrome.tabs.Tab, frameId: 0, url: page, origin: "https://www.reddit.com" };

  it.each([
    { type: "save_settings", settings: { storage_path: "/tmp/x" } },
    { type: "choose_folder" },
    { type: "get_setup_state" },
    { type: "status" },
    { type: "save", url: page },
    { type: "list" },
    { type: "show", id: "am_1" },
    { type: "update", id: "am_1", changes: { note: "x" } },
    { type: "list_collections" },
    { type: "take_pending_save" },
  ])("refuses $type", async (message) => {
    const response = await send(message, contentScript);
    expect(response).toEqual({ success: false, error: "Not allowed from a web page" });
    expect(client.sendRequest).not.toHaveBeenCalled();
    expect(client.sendOneShot).not.toHaveBeenCalled();
  });

  it("still answers lookup_urls, report_page and open_save_popup", async () => {
    client.sendRequest.mockResolvedValue({ type: "lookup_urls_result", saved: [] });
    expect(await send({ type: "lookup_urls", urls: [page] }, contentScript)).toEqual({
      success: true,
      data: { type: "lookup_urls_result", saved: [] },
    });
    expect(await send({ type: "report_page", url: page }, contentScript)).toEqual({ ok: true });
    chromeMock.action.openPopup = vi.fn().mockResolvedValue(undefined) as never;
    expect(await send({ type: "open_save_popup", url: page, title: "Post" }, contentScript)).toEqual({
      success: true,
      popup_opened: true,
    });
  });

  it("accepts every message from extension pages, also when opened in a tab", async () => {
    client.sendRequest.mockResolvedValue({ type: "list_collections_result", collections: [] });
    await send({ type: "list_collections" }, {});
    const ownPage = `chrome-extension://${chrome.runtime.id}/src/sidepanel/index.html`;
    await send({ type: "list_collections" }, { tab: { id: 3 } as chrome.tabs.Tab, url: ownPage, origin: `chrome-extension://${chrome.runtime.id}` });
    expect(client.sendRequest).toHaveBeenCalledTimes(2);
  });
});
