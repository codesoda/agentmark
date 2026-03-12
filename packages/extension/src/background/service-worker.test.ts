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
  handleSaveRequest,
  CONTEXT_MENU_ID,
} from "./service-worker";
import { getNativeClient } from "../shared/native-messaging";

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

    it("passes through optional save fields", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({ type: "save_result", id: "1", path: "/a", status: "created" });

      const sendResponse = vi.fn();
      handleRuntimeMessage(
        {
          type: "save",
          url: "https://example.com",
          title: "My Page",
          tags: ["test"],
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
          note: "a note",
          selected_text: "selected text",
          action: "read",
        });
      });
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
        const [id, opts] = chromeMock.notifications.create.mock.calls[0];
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
        const [id, opts] = chromeMock.notifications.create.mock.calls[0];
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
        const [, opts] = chromeMock.notifications.create.mock.calls[0];
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

  describe("handleSaveRequest - error normalization", () => {
    it("normalizes native ErrorResponse to runtime failure", async () => {
      const mockClient = getMockClient();
      mockClient.sendRequest.mockResolvedValue({
        type: "error",
        message: "Failed to save: invalid URL",
      });

      const result = await handleSaveRequest({ type: "save", url: "https://example.com" });

      expect(result).toEqual({
        success: false,
        error: "Failed to save: invalid URL",
      });
    });

    it("returns success for save_result responses", async () => {
      const mockClient = getMockClient();
      const nativeResponse = { type: "save_result" as const, id: "abc", path: "/tmp", status: "created" };
      mockClient.sendRequest.mockResolvedValue(nativeResponse);

      const result = await handleSaveRequest({ type: "save", url: "https://example.com" });

      expect(result).toEqual({
        success: true,
        data: nativeResponse,
      });
    });

    it("returns success for status_result responses", async () => {
      const mockClient = getMockClient();
      const nativeResponse = { type: "status_result" as const, ok: true, version: "1.0.0" };
      mockClient.sendRequest.mockResolvedValue(nativeResponse);

      const result = await handleSaveRequest({ type: "status" });

      expect(result).toEqual({
        success: true,
        data: nativeResponse,
      });
    });
  });
});
