import { describe, it, expect, vi, beforeEach } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";
import { queryActiveTab, sendSaveMessage, isConnectionError, isSupportedUrl } from "./runtime";

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

  it("returns active tab data", async () => {
    vi.mocked(chrome.tabs.query).mockResolvedValue([
      { url: "https://example.com", title: "Example", favIconUrl: "https://example.com/favicon.ico" } as chrome.tabs.Tab,
    ]);

    const tab = await queryActiveTab();
    expect(tab).toEqual({
      url: "https://example.com",
      title: "Example",
      favIconUrl: "https://example.com/favicon.ico",
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

  it("sends correct runtime message and returns response", async () => {
    const mockResponse = { success: true as const, data: { type: "save_result" as const, id: "abc123", path: "/tmp/abc", status: "created" } };
    vi.mocked(chrome.runtime.sendMessage).mockResolvedValue(mockResponse);

    const result = await sendSaveMessage("https://example.com", "Example");

    expect(chrome.runtime.sendMessage).toHaveBeenCalledWith(
      { type: "save", url: "https://example.com", title: "Example" },
    );
    expect(result).toEqual(mockResponse);
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
