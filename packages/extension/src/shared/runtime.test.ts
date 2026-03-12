import { describe, it, expect, vi, beforeEach } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";
import {
  queryActiveTab,
  sendSaveMessage,
  sendListCollectionsMessage,
  sendListBookmarksMessage,
  sendShowBookmarkMessage,
  sendUpdateBookmarkMessage,
  isConnectionError,
  isSupportedUrl,
  loadLastUsedIntent,
  saveLastUsedIntent,
  querySelectedText,
  normalizeTags,
} from "./runtime";

describe("isSupportedUrl", () => {
  it("accepts http URLs", () => {
    expect(isSupportedUrl("http://example.com")).toBe(true);
  });

  it("accepts https URLs", () => {
    expect(isSupportedUrl("https://example.com/path")).toBe(true);
  });

  it("rejects chrome:// URLs", () => {
    expect(isSupportedUrl("chrome://extensions")).toBe(false);
  });

  it("rejects chrome-extension:// URLs", () => {
    expect(isSupportedUrl("chrome-extension://abc/popup.html")).toBe(false);
  });

  it("rejects file:// URLs", () => {
    expect(isSupportedUrl("file:///tmp/test.html")).toBe(false);
  });

  it("rejects undefined", () => {
    expect(isSupportedUrl(undefined)).toBe(false);
  });

  it("rejects empty string", () => {
    expect(isSupportedUrl("")).toBe(false);
  });

  it("rejects malformed URLs", () => {
    expect(isSupportedUrl("not a url")).toBe(false);
  });
});

describe("queryActiveTab", () => {
  beforeEach(() => {
    resetChromeMock();
  });

  it("returns active tab data with id", async () => {
    vi.mocked(chrome.tabs.query).mockResolvedValue([
      { id: 42, url: "https://example.com", title: "Example", favIconUrl: "https://example.com/favicon.ico" } as chrome.tabs.Tab,
    ]);

    const tab = await queryActiveTab();
    expect(tab).toEqual({
      url: "https://example.com",
      title: "Example",
      favIconUrl: "https://example.com/favicon.ico",
      id: 42,
    });
  });

  it("throws when no tabs returned", async () => {
    vi.mocked(chrome.tabs.query).mockResolvedValue([]);
    await expect(queryActiveTab()).rejects.toThrow("No active tab found");
  });

  it("throws when tab has no URL", async () => {
    vi.mocked(chrome.tabs.query).mockResolvedValue([
      { title: "No URL" } as chrome.tabs.Tab,
    ]);
    await expect(queryActiveTab()).rejects.toThrow("No active tab found");
  });

  it("throws for unsupported URL scheme", async () => {
    vi.mocked(chrome.tabs.query).mockResolvedValue([
      { url: "chrome://extensions", title: "Extensions" } as chrome.tabs.Tab,
    ]);
    await expect(queryActiveTab()).rejects.toThrow("Unsupported page: chrome://extensions");
  });

  it("returns tab without favicon when missing", async () => {
    vi.mocked(chrome.tabs.query).mockResolvedValue([
      { url: "https://example.com", title: "Example" } as chrome.tabs.Tab,
    ]);

    const tab = await queryActiveTab();
    expect(tab.favIconUrl).toBeUndefined();
  });
});

describe("sendSaveMessage", () => {
  beforeEach(() => {
    resetChromeMock();
  });

  it("sends basic save message and returns response", async () => {
    const mockResponse = { success: true as const, data: { type: "save_result" as const, id: "abc123", path: "/tmp/abc", status: "created" } };
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue(mockResponse);

    const result = await sendSaveMessage("https://example.com", "Example");

    expect(chrome.runtime.sendMessage).toHaveBeenCalledWith(
      { type: "save", url: "https://example.com", title: "Example" },
    );
    expect(result).toEqual(mockResponse);
  });

  it("sends save message with options", async () => {
    const mockResponse = { success: true as const, data: { type: "save_result" as const, id: "abc123", path: "/tmp", status: "updated" } };
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue(mockResponse);

    await sendSaveMessage("https://example.com", "Title", {
      tags: ["rust"],
      collection: "reading",
      note: "my note",
      action: "summarize",
      selected_text: "excerpt",
    });

    expect(chrome.runtime.sendMessage).toHaveBeenCalledWith({
      type: "save",
      url: "https://example.com",
      title: "Title",
      tags: ["rust"],
      collection: "reading",
      note: "my note",
      action: "summarize",
      selected_text: "excerpt",
    });
  });

  it("returns error when sendMessage rejects", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockRejectedValue(
      new Error("Extension context invalidated"),
    );

    const result = await sendSaveMessage("https://example.com");

    expect(result).toEqual({
      success: false,
      error: "Extension context invalidated",
    });
  });

  it("returns stringified error for non-Error rejections", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockRejectedValue("connection failed");

    const result = await sendSaveMessage("https://example.com");

    expect(result).toEqual({
      success: false,
      error: "connection failed",
    });
  });

  it("returns error for null response", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue(null);

    const result = await sendSaveMessage("https://example.com");
    expect(result.success).toBe(false);
  });

  it("returns error for malformed response", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({ unexpected: true });

    const result = await sendSaveMessage("https://example.com");
    expect(result.success).toBe(false);
  });
});

describe("sendListCollectionsMessage", () => {
  beforeEach(() => {
    resetChromeMock();
  });

  it("returns collection list on success", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: { type: "list_collections_result", collections: ["reading", "work"] },
    });

    const result = await sendListCollectionsMessage();
    expect(result).toEqual(["reading", "work"]);
  });

  it("returns empty array on error response", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: false,
      error: "not connected",
    });

    const result = await sendListCollectionsMessage();
    expect(result).toEqual([]);
  });

  it("returns empty array on unexpected data type", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: { type: "status_result", ok: true, version: "0.1.0" },
    });

    const result = await sendListCollectionsMessage();
    expect(result).toEqual([]);
  });

  it("returns empty array when sendMessage throws", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockRejectedValue(new Error("disconnected"));

    const result = await sendListCollectionsMessage();
    expect(result).toEqual([]);
  });
});

describe("isConnectionError", () => {
  it("detects native messaging host not found", () => {
    expect(isConnectionError("native messaging host not found")).toBe(true);
  });

  it("detects specified native messaging host not found", () => {
    expect(isConnectionError("Specified native messaging host not found")).toBe(true);
  });

  it("detects native host has exited", () => {
    expect(isConnectionError("Native host has exited")).toBe(true);
  });

  it("detects disconnected", () => {
    expect(isConnectionError("Port disconnected")).toBe(true);
  });

  it("detects not connected", () => {
    expect(isConnectionError("not connected to native host")).toBe(true);
  });

  it("returns false for generic errors", () => {
    expect(isConnectionError("Failed to save bookmark")).toBe(false);
  });
});

describe("loadLastUsedIntent", () => {
  beforeEach(() => {
    resetChromeMock();
  });

  it("returns stored tags and collection", async () => {
    (chrome.storage.local.get as ReturnType<typeof vi.fn>).mockResolvedValue({
      agentmark_last_intent: { tags: ["rust", "cli"], collection: "reading" },
    });

    const result = await loadLastUsedIntent();
    expect(result.tags).toEqual(["rust", "cli"]);
    expect(result.collection).toBe("reading");
  });

  it("returns defaults when key is missing", async () => {
    (chrome.storage.local.get as ReturnType<typeof vi.fn>).mockResolvedValue({});

    const result = await loadLastUsedIntent();
    expect(result.tags).toEqual([]);
    expect(result.collection).toBeUndefined();
  });

  it("returns defaults for malformed stored data", async () => {
    (chrome.storage.local.get as ReturnType<typeof vi.fn>).mockResolvedValue({
      agentmark_last_intent: "not an object",
    });

    const result = await loadLastUsedIntent();
    expect(result.tags).toEqual([]);
  });

  it("filters non-string tags", async () => {
    (chrome.storage.local.get as ReturnType<typeof vi.fn>).mockResolvedValue({
      agentmark_last_intent: { tags: ["valid", 42, null, "also-valid"] },
    });

    const result = await loadLastUsedIntent();
    expect(result.tags).toEqual(["valid", "also-valid"]);
  });

  it("returns defaults when storage throws", async () => {
    (chrome.storage.local.get as ReturnType<typeof vi.fn>).mockRejectedValue(new Error("quota exceeded"));

    const result = await loadLastUsedIntent();
    expect(result.tags).toEqual([]);
    expect(result.collection).toBeUndefined();
  });
});

describe("saveLastUsedIntent", () => {
  beforeEach(() => {
    resetChromeMock();
  });

  it("persists tags and collection to storage", async () => {
    await saveLastUsedIntent({ tags: ["rust"], collection: "work" });

    expect(chrome.storage.local.set).toHaveBeenCalledWith({
      agentmark_last_intent: { tags: ["rust"], collection: "work" },
    });
  });

  it("does not throw when storage write fails", async () => {
    (chrome.storage.local.set as ReturnType<typeof vi.fn>).mockRejectedValue(new Error("write failed"));

    // Should not throw
    await saveLastUsedIntent({ tags: [] });
  });
});

describe("querySelectedText", () => {
  beforeEach(() => {
    resetChromeMock();
  });

  it("returns selected text from tab", async () => {
    (chrome.scripting.executeScript as ReturnType<typeof vi.fn>).mockResolvedValue([
      { result: "selected content" },
    ]);

    const text = await querySelectedText(42);
    expect(text).toBe("selected content");
  });

  it("returns empty string when no selection", async () => {
    (chrome.scripting.executeScript as ReturnType<typeof vi.fn>).mockResolvedValue([
      { result: "" },
    ]);

    const text = await querySelectedText(42);
    expect(text).toBe("");
  });

  it("returns empty string when result is not a string", async () => {
    (chrome.scripting.executeScript as ReturnType<typeof vi.fn>).mockResolvedValue([
      { result: null },
    ]);

    const text = await querySelectedText(42);
    expect(text).toBe("");
  });

  it("returns empty string when executeScript throws", async () => {
    (chrome.scripting.executeScript as ReturnType<typeof vi.fn>).mockRejectedValue(
      new Error("Cannot access chrome:// URLs"),
    );

    const text = await querySelectedText(42);
    expect(text).toBe("");
  });

  it("returns empty string when results array is empty", async () => {
    (chrome.scripting.executeScript as ReturnType<typeof vi.fn>).mockResolvedValue([]);

    const text = await querySelectedText(42);
    expect(text).toBe("");
  });
});

describe("sendListBookmarksMessage", () => {
  beforeEach(() => {
    resetChromeMock();
  });

  it("sends list message with default params", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: { type: "list_result", bookmarks: [] },
    });

    const result = await sendListBookmarksMessage();
    expect(chrome.runtime.sendMessage).toHaveBeenCalledWith({
      type: "list",
      limit: undefined,
      state: undefined,
    });
    expect(result).toEqual({ bookmarks: [] });
  });

  it("passes limit and state to runtime message", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: { type: "list_result", bookmarks: [] },
    });

    await sendListBookmarksMessage({ limit: 25, state: "inbox" });
    expect(chrome.runtime.sendMessage).toHaveBeenCalledWith({
      type: "list",
      limit: 25,
      state: "inbox",
    });
  });

  it("returns bookmarks from successful response", async () => {
    const bookmarks = [
      {
        id: "am_123",
        url: "https://example.com",
        title: "Example",
        state: "inbox",
        user_tags: ["rust"],
        suggested_tags: [],
        saved_at: "2026-03-12T00:00:00Z",
      },
    ];
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: { type: "list_result", bookmarks },
    });

    const result = await sendListBookmarksMessage();
    expect(result.bookmarks).toEqual(bookmarks);
    expect(result.error).toBeUndefined();
  });

  it("returns error on failure response", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: false,
      error: "not initialized",
    });

    const result = await sendListBookmarksMessage();
    expect(result.bookmarks).toEqual([]);
    expect(result.error).toBe("not initialized");
  });

  it("returns error on unexpected response type", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: { type: "status_result", ok: true, version: "0.1.0" },
    });

    const result = await sendListBookmarksMessage();
    expect(result.bookmarks).toEqual([]);
    expect(result.error).toContain("Unexpected response type");
  });

  it("returns error when sendMessage throws", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockRejectedValue(new Error("disconnected"));

    const result = await sendListBookmarksMessage();
    expect(result.bookmarks).toEqual([]);
    expect(result.error).toBe("disconnected");
  });

  it("returns error when success response has non-object data", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: "not-an-object",
    });

    const result = await sendListBookmarksMessage();
    expect(result.bookmarks).toEqual([]);
    expect(result.error).toBeDefined();
  });

  it("returns error when success response has null data", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: null,
    });

    const result = await sendListBookmarksMessage();
    expect(result.bookmarks).toEqual([]);
    expect(result.error).toBeDefined();
  });
});

describe("normalizeTags", () => {
  it("splits on commas and trims", () => {
    expect(normalizeTags("rust, cli, web")).toEqual(["rust", "cli", "web"]);
  });

  it("lowercases tags", () => {
    expect(normalizeTags("Rust, CLI")).toEqual(["rust", "cli"]);
  });

  it("deduplicates preserving first-seen order", () => {
    expect(normalizeTags("a, b, a, c")).toEqual(["a", "b", "c"]);
  });

  it("filters blank tags from repeated commas", () => {
    expect(normalizeTags(",,,")).toEqual([]);
  });

  it("handles empty input", () => {
    expect(normalizeTags("")).toEqual([]);
  });

  it("trims whitespace-only segments", () => {
    expect(normalizeTags("  ,  tag  ,  ")).toEqual(["tag"]);
  });
});

describe("sendShowBookmarkMessage", () => {
  beforeEach(() => {
    resetChromeMock();
  });

  it("returns bookmark on success", async () => {
    const bookmark = {
      id: "am_1",
      url: "https://example.com",
      title: "Example",
      summary: "A summary",
      saved_at: "2026-03-12T00:00:00Z",
      capture_source: "cli",
      state: "inbox",
      user_tags: ["rust"],
      suggested_tags: ["ai"],
      collections: ["reading"],
      note: null,
    };
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: { type: "show_result", bookmark },
    });

    const result = await sendShowBookmarkMessage("am_1");
    expect(result.bookmark).toEqual(bookmark);
    expect(result.error).toBeUndefined();
    expect(chrome.runtime.sendMessage).toHaveBeenCalledWith({
      type: "show",
      id: "am_1",
    });
  });

  it("returns error on failure response", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: false,
      error: "bookmark not found",
    });

    const result = await sendShowBookmarkMessage("am_missing");
    expect(result.bookmark).toBeUndefined();
    expect(result.error).toBe("bookmark not found");
  });

  it("returns error on unexpected response type", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: { type: "status_result", ok: true, version: "0.1.0" },
    });

    const result = await sendShowBookmarkMessage("am_1");
    expect(result.bookmark).toBeUndefined();
    expect(result.error).toContain("Unexpected response type");
  });

  it("returns error when sendMessage throws", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockRejectedValue(new Error("disconnected"));

    const result = await sendShowBookmarkMessage("am_1");
    expect(result.bookmark).toBeUndefined();
    expect(result.error).toBe("disconnected");
  });

  it("returns error when success response has malformed data", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: null,
    });

    const result = await sendShowBookmarkMessage("am_1");
    expect(result.error).toBeDefined();
  });
});

describe("sendUpdateBookmarkMessage", () => {
  beforeEach(() => {
    resetChromeMock();
  });

  it("returns updated bookmark on success", async () => {
    const bookmark = {
      id: "am_1",
      url: "https://example.com",
      title: "Example",
      summary: "A summary",
      saved_at: "2026-03-12T00:00:00Z",
      capture_source: "cli",
      state: "processed",
      user_tags: ["rust", "ai"],
      suggested_tags: [],
      collections: ["reading"],
      note: "Updated note",
    };
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: { type: "update_result", bookmark },
    });

    const result = await sendUpdateBookmarkMessage("am_1", { state: "processed" });
    expect(result.bookmark).toEqual(bookmark);
    expect(result.error).toBeUndefined();
    expect(chrome.runtime.sendMessage).toHaveBeenCalledWith({
      type: "update",
      id: "am_1",
      changes: { state: "processed" },
    });
  });

  it("returns error on failure response", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: false,
      error: "update failed",
    });

    const result = await sendUpdateBookmarkMessage("am_1", { note: "test" });
    expect(result.bookmark).toBeUndefined();
    expect(result.error).toBe("update failed");
  });

  it("returns error on unexpected response type", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue({
      success: true,
      data: { type: "status_result", ok: true, version: "0.1.0" },
    });

    const result = await sendUpdateBookmarkMessage("am_1", { note: "test" });
    expect(result.error).toContain("Unexpected response type");
  });

  it("returns error when sendMessage throws", async () => {
    vi.mocked(chrome.runtime.sendMessage).mockRejectedValue(new Error("host exited"));

    const result = await sendUpdateBookmarkMessage("am_1", { note: "test" });
    expect(result.error).toBe("host exited");
  });
});
