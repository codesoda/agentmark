import { describe, it, expect, vi, beforeEach } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";

// We need to mock the native-messaging module before importing the service worker
vi.mock("../shared/native-messaging", () => {
  const mockClient = {
    sendRequest: vi.fn(),
    getStatus: vi.fn().mockReturnValue("connected"),
    disconnect: vi.fn(),
  };
  return {
    getNativeClient: vi.fn(() => mockClient),
    resetNativeClient: vi.fn(),
    NativeMessagingClient: vi.fn(),
    _mockClient: mockClient,
  };
});

// Import after mock setup
import {
  ensureContextMenu,
  isSupportedUrl,
  handleRuntimeMessage,
  handleContextMenuClick,
  handleNativeRequest,
  handleCommand,
  openSidePanel,
  CONTEXT_MENU_ID,
  SIDEPANEL_MENU_ID,
  toolbarIcon,
} from "./service-worker";
import { getNativeClient } from "../shared/native-messaging";
import { SAVE_RESULT_HOLD_MS } from "./toolbar-icon";

// Listeners are registered once, on import, against the mock that existed then;
// capture them before beforeEach swaps the mock out and clears call history.
const registeredTabListeners = {
  onUpdated: vi.mocked(chrome.tabs.onUpdated.addListener).mock.calls[0]?.[0],
  onActivated: vi.mocked(chrome.tabs.onActivated.addListener).mock.calls[0]?.[0],
  onRemoved: vi.mocked(chrome.tabs.onRemoved.addListener).mock.calls[0]?.[0],
};

function getMockClient() {
  return getNativeClient() as unknown as {
    sendRequest: ReturnType<typeof vi.fn>;
    getStatus: ReturnType<typeof vi.fn>;
    disconnect: ReturnType<typeof vi.fn>;
  };
}

describe("service-worker", () => {
  let chromeMock: ReturnType<typeof resetChromeMock>;

  beforeEach(() => {
    chromeMock = resetChromeMock();
    vi.clearAllMocks();
  });

  describe("isSupportedUrl", () => {
    it("accepts http URLs", () => {
      expect(isSupportedUrl("http://example.com")).toBe(true);
    });

    it("accepts https URLs", () => {
      expect(isSupportedUrl("https://example.com/path?q=1")).toBe(true);
    });

    it("rejects chrome:// URLs", () => {
      expect(isSupportedUrl("chrome://extensions")).toBe(false);
    });

    it("rejects chrome-extension:// URLs", () => {
      expect(isSupportedUrl("chrome-extension://abc123")).toBe(false);
    });

    it("rejects about: URLs", () => {
      expect(isSupportedUrl("about:blank")).toBe(false);
    });

    it("rejects undefined", () => {
      expect(isSupportedUrl(undefined)).toBe(false);
    });

    it("rejects empty string", () => {
      expect(isSupportedUrl("")).toBe(false);
    });

    it("rejects malformed URL", () => {
      expect(isSupportedUrl("not a url")).toBe(false);
    });

    it("rejects file:// URLs", () => {
      expect(isSupportedUrl("file:///tmp/test.html")).toBe(false);
    });

    it("rejects ftp:// URLs", () => {
      expect(isSupportedUrl("ftp://example.com")).toBe(false);
    });
  });

  describe("ensureContextMenu", () => {
    it("removes all existing menus then creates the save menu", () => {
      chromeMock.contextMenus.removeAll.mockImplementation((cb?: () => void) => cb?.());

      ensureContextMenu();

      expect(chromeMock.contextMenus.removeAll).toHaveBeenCalledTimes(1);
      expect(chromeMock.contextMenus.create).toHaveBeenCalledWith({
        id: CONTEXT_MENU_ID,
        title: "Save to AgentMark",
        contexts: ["page", "selection"],
      });
    });
  });

  describe("handleRuntimeMessage", () => {
    it("forwards save request to native client", async () => {
      const mockClient = getMockClient();
      const nativeResponse = { type: "save_result" as const, id: "abc", path: "/tmp", status: "created" };
      mockClient.sendRequest.mockResolvedValue(nativeResponse);

      const sendResponse = vi.fn();
      const result = handleRuntimeMessage(
        { type: "save", url: "https://example.com", title: "Test" },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      expect(result).toBe(true); // Async response

      // Wait for the async handler
      await vi.waitFor(() => {
        expect(sendResponse).toHaveBeenCalledWith({
          success: true,
          data: nativeResponse,
        });
      });
    });

    it("forwards status request to native client", async () => {
      const mockClient = getMockClient();
      const nativeResponse = { type: "status_result" as const, ok: true, version: "1.0.0" };
      mockClient.sendRequest.mockResolvedValue(nativeResponse);

      const sendResponse = vi.fn();
      const result = handleRuntimeMessage(
        { type: "status" },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      expect(result).toBe(true);

      await vi.waitFor(() => {
        expect(sendResponse).toHaveBeenCalledWith({
          success: true,
          data: nativeResponse,
        });
      });
    });

    it("rejects unsupported URL synchronously", () => {
      const sendResponse = vi.fn();
      const result = handleRuntimeMessage(
        { type: "save", url: "chrome://extensions" },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      expect(result).toBe(false);
      expect(sendResponse).toHaveBeenCalledWith({
        success: false,
        error: "Unsupported or missing URL",
      });
    });

    it("returns error when native client rejects", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockRejectedValue(new Error("Host not connected"));

      const sendResponse = vi.fn();
      handleRuntimeMessage(
        { type: "save", url: "https://example.com" },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      await vi.waitFor(() => {
        expect(sendResponse).toHaveBeenCalledWith({
          success: false,
          error: "Host not connected",
        });
      });
    });

    it("passes through optional save fields including collection", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({ type: "save_result", id: "1", path: "/a", status: "created" });

      const sendResponse = vi.fn();
      handleRuntimeMessage(
        {
          type: "save",
          url: "https://example.com",
          title: "My Page",
          tags: ["test"],
          collection: "reading",
          note: "a note",
          selected_text: "selected text",
          action: "read",
        },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      await vi.waitFor(() => {
        expect(mockClient.sendRequest).toHaveBeenCalledWith({
          type: "save",
          url: "https://example.com",
          title: "My Page",
          tags: ["test"],
          collection: "reading",
          note: "a note",
          selected_text: "selected text",
          action: "read",
        });
      });
    });

    it("forwards list_collections request to native host", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({
        type: "list_collections_result",
        collections: ["reading", "work"],
      });

      const sendResponse = vi.fn();
      const result = handleRuntimeMessage(
        { type: "list_collections" },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      expect(result).toBe(true);

      await vi.waitFor(() => {
        expect(mockClient.sendRequest).toHaveBeenCalledWith({
          type: "list_collections",
        });
        expect(sendResponse).toHaveBeenCalledWith({
          success: true,
          data: { type: "list_collections_result", collections: ["reading", "work"] },
        });
      });
    });
  });

  describe("saves started by an in-page button", () => {
    const post = {
      type: "open_save_popup" as const,
      url: "https://www.linkedin.com/feed/update/urn:li:share:42/",
      title: "LinkedIn post by Ada Lovelace",
      page_html: "<html><body>the post</body></html>",
    };
    const fromTab = { tab: { id: 7 } } as chrome.runtime.MessageSender;

    function send(message: unknown, sender = {} as chrome.runtime.MessageSender) {
      const sendResponse = vi.fn();
      handleRuntimeMessage(message as Parameters<typeof handleRuntimeMessage>[0], sender, sendResponse);
      return sendResponse;
    }

    it("opens the popup and hands it the save exactly once", async () => {
      const mockClient = getMockClient();
      const opened = send(post, fromTab);
      await vi.waitFor(() => {
        expect(opened).toHaveBeenCalledWith({ success: true, popup_opened: true });
      });
      expect(chrome.action.openPopup).toHaveBeenCalledTimes(1);
      expect(mockClient.sendRequest).not.toHaveBeenCalled();

      expect(send({ type: "take_pending_save" })).toHaveBeenCalledWith({
        pending: { url: post.url, title: post.title, page_html: post.page_html },
      });
      expect(send({ type: "take_pending_save" })).toHaveBeenCalledWith({ pending: null });
    });

    it("tells the originating tab when the popup's save finishes", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({ type: "save_result", id: "1", path: "/a", status: "created" });
      const opened = send(post, fromTab);
      await vi.waitFor(() => expect(opened).toHaveBeenCalled());
      send({ type: "take_pending_save" });

      const saved = send({ type: "save", url: post.url, title: post.title, page_html: post.page_html });
      await vi.waitFor(() => expect(saved).toHaveBeenCalled());

      expect(chrome.tabs.sendMessage).toHaveBeenCalledWith(7, {
        type: "agentmark:save-result",
        url: post.url,
        success: true,
        error: undefined,
      });
    });

    it("does not notify a tab about unrelated saves", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({ type: "save_result", id: "1", path: "/a", status: "created" });
      const opened = send(post, fromTab);
      await vi.waitFor(() => expect(opened).toHaveBeenCalled());
      send({ type: "take_pending_save" });

      const saved = send({ type: "save", url: "https://example.com/other" });
      await vi.waitFor(() => expect(saved).toHaveBeenCalled());

      expect(chrome.tabs.sendMessage).not.toHaveBeenCalled();
    });

    it("saves directly when the popup cannot be opened", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({ type: "save_result", id: "1", path: "/a", status: "created" });
      (chrome.action.openPopup as ReturnType<typeof vi.fn>).mockRejectedValue(
        new Error("Could not find an active browser window."),
      );

      const response = send(post, fromTab);

      await vi.waitFor(() => {
        expect(response).toHaveBeenCalledWith({
          success: true,
          data: { type: "save_result", id: "1", path: "/a", status: "created" },
        });
      });
      expect(mockClient.sendRequest).toHaveBeenCalledWith({
        type: "save",
        url: post.url,
        title: post.title,
        page_html: post.page_html,
      });
      // Nothing is left behind for a later toolbar click to pick up.
      expect(send({ type: "take_pending_save" })).toHaveBeenCalledWith({ pending: null });
    });

    it("drops a pending save the popup did not collect in time", async () => {
      vi.useFakeTimers();
      try {
        const opened = send(post, fromTab);
        await vi.waitFor(() => expect(opened).toHaveBeenCalled());
        vi.advanceTimersByTime(6_000);
        expect(send({ type: "take_pending_save" })).toHaveBeenCalledWith({ pending: null });
      } finally {
        vi.useRealTimers();
      }
    });

    it("rejects unsupported URLs without opening the popup", () => {
      const response = send({ ...post, url: "chrome://extensions" }, fromTab);
      expect(response).toHaveBeenCalledWith({ success: false, error: "Unsupported or missing URL" });
      expect(chrome.action.openPopup).not.toHaveBeenCalled();
    });
  });

  describe("handleContextMenuClick", () => {
    it("saves page URL from context menu click", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({ type: "save_result", id: "1", path: "/a", status: "created" });

      handleContextMenuClick(
        { menuItemId: CONTEXT_MENU_ID, pageUrl: "https://example.com", editable: false } as chrome.contextMenus.OnClickData,
        { title: "Test Page" } as chrome.tabs.Tab,
      );

      await vi.waitFor(() => {
        expect(mockClient.sendRequest).toHaveBeenCalledWith({
          type: "save",
          url: "https://example.com",
          title: "Test Page",
          selected_text: undefined,
        });
      });
    });

    it("includes an HTML snapshot of the tab when it has an id", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({ type: "save_result", id: "1", path: "/a", status: "created" });
      (chrome.scripting.executeScript as ReturnType<typeof vi.fn>).mockResolvedValue([
        { result: "<html><body>post</body></html>" },
      ]);

      handleContextMenuClick(
        { menuItemId: CONTEXT_MENU_ID, pageUrl: "https://example.com", editable: false } as chrome.contextMenus.OnClickData,
        { id: 9, title: "Test Page" } as chrome.tabs.Tab,
      );

      await vi.waitFor(() => {
        expect(mockClient.sendRequest).toHaveBeenCalledWith(
          expect.objectContaining({ page_html: "<html><body>post</body></html>" }),
        );
      });
    });

    it("includes selection text when present", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({ type: "save_result", id: "1", path: "/a", status: "created" });

      handleContextMenuClick(
        {
          menuItemId: CONTEXT_MENU_ID,
          pageUrl: "https://example.com",
          selectionText: "selected content",
          editable: false,
        } as chrome.contextMenus.OnClickData,
        { title: "Test" } as chrome.tabs.Tab,
      );

      await vi.waitFor(() => {
        expect(mockClient.sendRequest).toHaveBeenCalledWith(
          expect.objectContaining({ selected_text: "selected content" }),
        );
      });
    });

    it("falls back to tab.url when pageUrl is missing", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({ type: "save_result", id: "1", path: "/a", status: "created" });

      handleContextMenuClick(
        { menuItemId: CONTEXT_MENU_ID, editable: false } as chrome.contextMenus.OnClickData,
        { url: "https://fallback.com", title: "Fallback" } as chrome.tabs.Tab,
      );

      await vi.waitFor(() => {
        expect(mockClient.sendRequest).toHaveBeenCalledWith(
          expect.objectContaining({ url: "https://fallback.com" }),
        );
      });
    });

    it("ignores clicks on non-matching menu items", () => {
      const mockClient = getMockClient();

      handleContextMenuClick(
        { menuItemId: "other-menu", pageUrl: "https://example.com", editable: false } as chrome.contextMenus.OnClickData,
        {} as chrome.tabs.Tab,
      );

      expect(mockClient.sendRequest).not.toHaveBeenCalled();
    });

    it("does not send request for unsupported URL", () => {
      const mockClient = getMockClient();

      handleContextMenuClick(
        { menuItemId: CONTEXT_MENU_ID, pageUrl: "chrome://settings", editable: false } as chrome.contextMenus.OnClickData,
        {} as chrome.tabs.Tab,
      );

      expect(mockClient.sendRequest).not.toHaveBeenCalled();
    });

    it("does not send request when no URL available", () => {
      const mockClient = getMockClient();

      handleContextMenuClick(
        { menuItemId: CONTEXT_MENU_ID, editable: false } as chrome.contextMenus.OnClickData,
        {} as chrome.tabs.Tab,
      );

      expect(mockClient.sendRequest).not.toHaveBeenCalled();
    });

    it("shows success notification after context-menu save", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({
        type: "save_result",
        id: "bm_123",
        path: "/tmp/bm",
        status: "created",
      });

      handleContextMenuClick(
        { menuItemId: CONTEXT_MENU_ID, pageUrl: "https://example.com", editable: false } as chrome.contextMenus.OnClickData,
        { title: "Example Page" } as chrome.tabs.Tab,
      );

      await vi.waitFor(() => {
        expect(chromeMock.notifications.create).toHaveBeenCalled();
        const [id, opts] = chromeMock.notifications.create.mock.calls[0] as [string, chrome.notifications.NotificationOptions];
        expect(id).toContain("agentmark-save-");
        expect(opts.type).toBe("basic");
        expect(opts.title).toBe("Saved to AgentMark");
        expect(opts.message).toBe("Example Page (created)");
      });
    });

    it("shows error notification after context-menu save failure", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockRejectedValue(new Error("Host disconnected"));

      handleContextMenuClick(
        { menuItemId: CONTEXT_MENU_ID, pageUrl: "https://example.com", editable: false } as chrome.contextMenus.OnClickData,
        { title: "Test" } as chrome.tabs.Tab,
      );

      await vi.waitFor(() => {
        expect(chromeMock.notifications.create).toHaveBeenCalled();
        const [id, opts] = chromeMock.notifications.create.mock.calls[0] as [string, chrome.notifications.NotificationOptions];
        expect(id).toContain("agentmark-error-");
        expect(opts.type).toBe("basic");
        expect(opts.title).toBe("AgentMark Save Failed");
        expect(opts.message).toBe("Host disconnected");
      });
    });

    it("uses URL as notification fallback when no tab title", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({
        type: "save_result",
        id: "bm_1",
        path: "/tmp/bm",
        status: "created",
      });

      handleContextMenuClick(
        { menuItemId: CONTEXT_MENU_ID, pageUrl: "https://example.com", editable: false } as chrome.contextMenus.OnClickData,
        {} as chrome.tabs.Tab,
      );

      await vi.waitFor(() => {
        expect(chromeMock.notifications.create).toHaveBeenCalled();
        const [, opts] = chromeMock.notifications.create.mock.calls[0] as [string, chrome.notifications.NotificationOptions];
        expect(opts.message).toBe("https://example.com (created)");
      });
    });

    it("does not show notification for unsupported URL", () => {
      handleContextMenuClick(
        { menuItemId: CONTEXT_MENU_ID, pageUrl: "chrome://settings", editable: false } as chrome.contextMenus.OnClickData,
        {} as chrome.tabs.Tab,
      );

      expect(chromeMock.notifications.create).not.toHaveBeenCalled();
    });
  });

  describe("handleNativeRequest - error normalization", () => {
    it("normalizes native ErrorResponse to runtime failure", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({
        type: "error",
        message: "Failed to save: invalid URL",
      });

      const result = await handleNativeRequest({ type: "save", url: "https://example.com" });

      expect(result).toEqual({
        success: false,
        error: "Failed to save: invalid URL",
      });
    });

    it("returns success for save_result responses", async () => {
      const mockClient = getMockClient();
      const nativeResponse = { type: "save_result" as const, id: "abc", path: "/tmp", status: "created" };
      mockClient.sendRequest.mockResolvedValue(nativeResponse);

      const result = await handleNativeRequest({ type: "save", url: "https://example.com" });

      expect(result).toEqual({
        success: true,
        data: nativeResponse,
      });
    });

    it("returns success for status_result responses", async () => {
      const mockClient = getMockClient();
      const nativeResponse = { type: "status_result" as const, ok: true, version: "1.0.0" };
      mockClient.sendRequest.mockResolvedValue(nativeResponse);

      const result = await handleNativeRequest({ type: "status" });

      expect(result).toEqual({
        success: true,
        data: nativeResponse,
      });
    });
  });

  describe("handleRuntimeMessage - list", () => {
    it("forwards list message to native client", async () => {
      const mockClient = getMockClient();
      const nativeResponse = {
        type: "list_result" as const,
        bookmarks: [
          {
            id: "am_123",
            url: "https://example.com",
            title: "Example",
            state: "inbox" as const,
            user_tags: [],
            suggested_tags: [],
            saved_at: "2026-03-12T00:00:00Z",
          },
        ],
      };
      mockClient.sendRequest.mockResolvedValue(nativeResponse);

      const sendResponse = vi.fn();
      const result = handleRuntimeMessage(
        { type: "list", limit: 25, state: "inbox" },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      expect(result).toBe(true);
      await vi.waitFor(() => expect(sendResponse).toHaveBeenCalled());
      expect(sendResponse).toHaveBeenCalledWith({
        success: true,
        data: nativeResponse,
      });
      expect(mockClient.sendRequest).toHaveBeenCalledWith({
        type: "list",
        limit: 25,
        state: "inbox",
      });
    });

    it("forwards list message without optional params", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({
        type: "list_result" as const,
        bookmarks: [],
      });

      const sendResponse = vi.fn();
      handleRuntimeMessage(
        { type: "list" },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      await vi.waitFor(() => expect(sendResponse).toHaveBeenCalled());
      expect(mockClient.sendRequest).toHaveBeenCalledWith({
        type: "list",
        limit: undefined,
        state: undefined,
      });
    });
  });

  describe("handleCommand", () => {
    it("opens side panel for open_side_panel command", async () => {
      chromeMock.tabs.query.mockResolvedValue([
        { windowId: 42, url: "https://example.com" },
      ]);

      handleCommand("open_side_panel");

      await vi.waitFor(() =>
        expect(chromeMock.sidePanel.open).toHaveBeenCalledWith({ windowId: 42 }),
      );
    });

    it("ignores unknown commands", () => {
      handleCommand("unknown_command");
      expect(chromeMock.sidePanel.open).not.toHaveBeenCalled();
    });
  });

  describe("openSidePanel", () => {
    it("opens side panel with current window ID", async () => {
      chromeMock.tabs.query.mockResolvedValue([
        { windowId: 99, url: "https://example.com" },
      ]);

      await openSidePanel();
      expect(chromeMock.sidePanel.open).toHaveBeenCalledWith({ windowId: 99 });
    });

    it("does not throw when no active tab", async () => {
      chromeMock.tabs.query.mockResolvedValue([]);

      await openSidePanel();
      expect(chromeMock.sidePanel.open).not.toHaveBeenCalled();
    });

    it("does not throw when sidePanel.open fails", async () => {
      chromeMock.tabs.query.mockResolvedValue([
        { windowId: 42, url: "https://example.com" },
      ]);
      chromeMock.sidePanel.open.mockRejectedValue(new Error("No window"));

      await openSidePanel();
      // Should not throw
    });
  });

  describe("handleContextMenuClick - sidepanel", () => {
    it("opens side panel for sidepanel menu item", async () => {
      chromeMock.tabs.query.mockResolvedValue([
        { windowId: 42, url: "https://example.com" },
      ]);

      handleContextMenuClick(
        { menuItemId: SIDEPANEL_MENU_ID, editable: false } as chrome.contextMenus.OnClickData,
      );

      await vi.waitFor(() =>
        expect(chromeMock.sidePanel.open).toHaveBeenCalledWith({ windowId: 42 }),
      );
    });

    it("still handles save menu item correctly", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({
        type: "save_result" as const,
        id: "abc",
        path: "/tmp",
        status: "created",
      });

      handleContextMenuClick(
        {
          menuItemId: CONTEXT_MENU_ID,
          pageUrl: "https://example.com",
          editable: false,
        } as chrome.contextMenus.OnClickData,
        { title: "Example" } as chrome.tabs.Tab,
      );

      await vi.waitFor(() => expect(mockClient.sendRequest).toHaveBeenCalled());
    });
  });

  describe("ensureContextMenu - sidepanel entry", () => {
    it("creates sidepanel context menu item with action context", () => {
      ensureContextMenu();

      expect(chromeMock.contextMenus.create).toHaveBeenCalledWith(
        expect.objectContaining({
          id: SIDEPANEL_MENU_ID,
          contexts: ["action"],
        }),
      );
    });
  });

  describe("handleRuntimeMessage - show", () => {
    it("forwards show request to native client", async () => {
      const mockClient = getMockClient();
      const bookmark = {
        id: "am_1",
        url: "https://example.com",
        title: "Example",
        summary: "A summary",
        saved_at: "2026-03-12T00:00:00Z",
        capture_source: "cli",
        state: "inbox" as const,
        user_tags: ["rust"],
        suggested_tags: ["ai"],
        collections: ["reading"],
        note: null,
      };
      const nativeResponse = { type: "show_result" as const, bookmark };
      mockClient.sendRequest.mockResolvedValue(nativeResponse);

      const sendResponse = vi.fn();
      const result = handleRuntimeMessage(
        { type: "show", id: "am_1" },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      expect(result).toBe(true);

      await vi.waitFor(() => {
        expect(sendResponse).toHaveBeenCalledWith({
          success: true,
          data: nativeResponse,
        });
      });
      expect(mockClient.sendRequest).toHaveBeenCalledWith({
        type: "show",
        id: "am_1",
      });
    });

    it("returns error when show request fails", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({
        type: "error",
        message: "Bookmark not found",
      });

      const sendResponse = vi.fn();
      handleRuntimeMessage(
        { type: "show", id: "am_missing" },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      await vi.waitFor(() => {
        expect(sendResponse).toHaveBeenCalledWith({
          success: false,
          error: "Bookmark not found",
        });
      });
    });
  });

  describe("handleRuntimeMessage - update", () => {
    it("forwards update request to native client", async () => {
      const mockClient = getMockClient();
      const bookmark = {
        id: "am_1",
        url: "https://example.com",
        title: "Example",
        summary: "A summary",
        saved_at: "2026-03-12T00:00:00Z",
        capture_source: "cli",
        state: "processed" as const,
        user_tags: ["rust", "ai"],
        suggested_tags: [],
        collections: ["reading"],
        note: "Updated",
      };
      const nativeResponse = { type: "update_result" as const, bookmark };
      mockClient.sendRequest.mockResolvedValue(nativeResponse);

      const sendResponse = vi.fn();
      const result = handleRuntimeMessage(
        { type: "update", id: "am_1", changes: { state: "processed" } },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      expect(result).toBe(true);

      await vi.waitFor(() => {
        expect(sendResponse).toHaveBeenCalledWith({
          success: true,
          data: nativeResponse,
        });
      });
      expect(mockClient.sendRequest).toHaveBeenCalledWith({
        type: "update",
        id: "am_1",
        changes: { state: "processed" },
      });
    });

    it("returns error when update request fails", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockRejectedValue(new Error("Host disconnected"));

      const sendResponse = vi.fn();
      handleRuntimeMessage(
        { type: "update", id: "am_1", changes: { note: "test" } },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      await vi.waitFor(() => {
        expect(sendResponse).toHaveBeenCalledWith({
          success: false,
          error: "Host disconnected",
        });
      });
    });
  });

  describe("handleRuntimeMessage - lookup_urls", () => {
    it("forwards supported URLs to the native host", async () => {
      const mockClient = getMockClient();
      const nativeResponse = { type: "lookup_urls_result" as const, saved: ["https://x.com/ada/status/1"] };
      mockClient.sendRequest.mockResolvedValue(nativeResponse);

      const sendResponse = vi.fn();
      const result = handleRuntimeMessage(
        {
          type: "lookup_urls",
          urls: ["https://x.com/ada/status/1", "chrome://settings", 42, "http://news.ycombinator.com/item?id=1"] as string[],
        },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      expect(result).toBe(true);
      await vi.waitFor(() => {
        expect(sendResponse).toHaveBeenCalledWith({ success: true, data: nativeResponse });
      });
      expect(mockClient.sendRequest).toHaveBeenCalledWith({
        type: "lookup_urls",
        urls: ["https://x.com/ada/status/1", "http://news.ycombinator.com/item?id=1"],
      });
    });

    it("caps a lookup at 200 URLs", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({ type: "lookup_urls_result", saved: [] });
      const urls = Array.from({ length: 250 }, (_, i) => `https://news.ycombinator.com/item?id=${i}`);

      const sendResponse = vi.fn();
      handleRuntimeMessage({ type: "lookup_urls", urls }, {} as chrome.runtime.MessageSender, sendResponse);

      await vi.waitFor(() => expect(sendResponse).toHaveBeenCalled());
      expect(mockClient.sendRequest).toHaveBeenCalledWith({ type: "lookup_urls", urls: urls.slice(0, 200) });
    });

    it("answers at once, without the native host, when no URL is usable", () => {
      const mockClient = getMockClient();
      for (const urls of [[], ["chrome://settings", "not a url"], "https://a.example/", undefined]) {
        const sendResponse = vi.fn();
        const result = handleRuntimeMessage(
          { type: "lookup_urls", urls } as unknown as import("../shared/types").RuntimeMessage,
          {} as chrome.runtime.MessageSender,
          sendResponse,
        );

        expect(result).toBe(false);
        expect(sendResponse).toHaveBeenCalledWith({
          success: true,
          data: { type: "lookup_urls_result", saved: [] },
        });
      }
      expect(mockClient.sendRequest).not.toHaveBeenCalled();
    });

    it("reports a native failure", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockRejectedValue(new Error("Native host has exited"));

      const sendResponse = vi.fn();
      handleRuntimeMessage(
        { type: "lookup_urls", urls: ["https://a.example/"] },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      await vi.waitFor(() => {
        expect(sendResponse).toHaveBeenCalledWith({ success: false, error: "Native host has exited" });
      });
    });
  });

  describe("toolbar icon", () => {
    const PAGE = "https://example.com/page";
    const saveResult = { type: "save_result", id: "1", path: "/a", status: "created" };

    function looks(tabId: number): string[] {
      return (chrome.action.setIcon as ReturnType<typeof vi.fn>).mock.calls
        .filter(([details]) => details.tabId === tabId)
        .map(([details]) => details.path[16].replace(/^icons\/toolbar-(.+)-16\.png$/, "$1"));
    }

    it("registers the tab listeners at load", () => {
      expect(registeredTabListeners.onUpdated).toBe(toolbarIcon.handleTabUpdated);
      expect(registeredTabListeners.onActivated).toBe(toolbarIcon.handleTabActivated);
      expect(registeredTabListeners.onRemoved).toBe(toolbarIcon.handleTabRemoved);
    });

    it("marks the active tab through a popup save of its page", async () => {
      vi.useFakeTimers();
      try {
        const mockClient = getMockClient();
        mockClient.sendRequest.mockResolvedValue(saveResult);
        chromeMock.tabs.query.mockResolvedValue([{ id: 21, url: PAGE }]);
        chromeMock.tabs.get.mockResolvedValue({ id: 21, url: PAGE });

        const sendResponse = vi.fn();
        handleRuntimeMessage({ type: "save", url: PAGE }, {} as chrome.runtime.MessageSender, sendResponse);
        await vi.waitFor(() => expect(looks(21)).toEqual(["saving", "saved-fresh"]));
        expect(sendResponse).toHaveBeenCalledWith({ success: true, data: saveResult });

        await vi.advanceTimersByTimeAsync(SAVE_RESULT_HOLD_MS);
        expect(looks(21)).toEqual(["saving", "saved-fresh", "saved"]);
        // Only the save itself reached the native host.
        expect(mockClient.sendRequest).toHaveBeenCalledTimes(1);
      } finally {
        toolbarIcon.forget(21);
        vi.useRealTimers();
      }
    });

    it("leaves the tab alone when the popup saves a post on it", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue(saveResult);
      chromeMock.tabs.query.mockResolvedValue([{ id: 22, url: PAGE }]);
      chromeMock.tabs.get.mockResolvedValue({ id: 22, url: PAGE });

      const sendResponse = vi.fn();
      handleRuntimeMessage(
        { type: "save", url: "https://www.linkedin.com/feed/update/urn:li:share:42/" },
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );
      await vi.waitFor(() => expect(sendResponse).toHaveBeenCalled());
      expect(chromeMock.tabs.query).toHaveBeenCalledWith({ active: true, lastFocusedWindow: true });
      expect(chrome.action.setIcon).not.toHaveBeenCalled();
    });

    it("shows a failed context-menu save on its tab, then repaints from the index", async () => {
      vi.useFakeTimers();
      try {
        const mockClient = getMockClient();
        mockClient.sendRequest.mockImplementation(async (request: { type: string }) =>
          request.type === "save"
            ? { type: "error", message: "disk full" }
            : { type: "lookup_urls_result", saved: [] },
        );
        chromeMock.tabs.get.mockResolvedValue({ id: 23, url: PAGE });

        handleContextMenuClick(
          { menuItemId: CONTEXT_MENU_ID, pageUrl: PAGE, editable: false } as chrome.contextMenus.OnClickData,
          { id: 23, url: PAGE, title: "Page" } as chrome.tabs.Tab,
        );
        await vi.waitFor(() => expect(looks(23)).toEqual(["saving", "error"]));

        await vi.advanceTimersByTimeAsync(SAVE_RESULT_HOLD_MS);
        expect(looks(23)).toEqual(["saving", "error", "idle"]);
        expect(mockClient.sendRequest).toHaveBeenCalledTimes(2);
        expect(mockClient.sendRequest).toHaveBeenLastCalledWith({ type: "lookup_urls", urls: [PAGE] });
      } finally {
        toolbarIcon.forget(23);
        vi.useRealTimers();
      }
    });

    it("does not mark the tab when the direct-save fallback saves a post", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue(saveResult);
      (chrome.action.openPopup as ReturnType<typeof vi.fn>).mockRejectedValue(new Error("no window"));
      chromeMock.tabs.get.mockResolvedValue({ id: 24, url: "https://www.linkedin.com/feed/" });

      const sendResponse = vi.fn();
      handleRuntimeMessage(
        { type: "open_save_popup", url: "https://www.linkedin.com/feed/update/urn:li:share:42/" },
        { tab: { id: 24 } } as chrome.runtime.MessageSender,
        sendResponse,
      );
      await vi.waitFor(() => expect(sendResponse).toHaveBeenCalledWith({ success: true, data: saveResult }));
      await vi.waitFor(() => expect(chromeMock.tabs.get).toHaveBeenCalledWith(24));
      expect(chrome.action.setIcon).not.toHaveBeenCalled();
    });

    it("still answers the popup when icon work fails", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue(saveResult);
      chromeMock.tabs.query.mockRejectedValue(new Error("no window"));

      const sendResponse = vi.fn();
      handleRuntimeMessage({ type: "save", url: PAGE }, {} as chrome.runtime.MessageSender, sendResponse);
      await vi.waitFor(() => expect(sendResponse).toHaveBeenCalledWith({ success: true, data: saveResult }));
    });
  });

  describe("handleRuntimeMessage - unknown type", () => {
    it("returns error for unknown message type", () => {
      const sendResponse = vi.fn();
      const result = handleRuntimeMessage(
        { type: "unknown_type" } as unknown as import("../shared/types").RuntimeMessage,
        {} as chrome.runtime.MessageSender,
        sendResponse,
      );

      expect(result).toBe(false);
      expect(sendResponse).toHaveBeenCalledWith({
        success: false,
        error: "Unknown message type: unknown_type",
      });
    });
  });
});
