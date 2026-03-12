import { describe, it, expect } from "vitest";
import { parseNativeResponse, isErrorResponse } from "./types";

describe("parseNativeResponse", () => {
  it("parses save_result correctly", () => {
    const raw = { type: "save_result", id: "abc123", path: "/tmp/bundle", status: "created" };
    const result = parseNativeResponse(raw);
    expect(result).toEqual({ type: "save_result", id: "abc123", path: "/tmp/bundle", status: "created" });
  });

  it("parses status_result correctly", () => {
    const raw = { type: "status_result", ok: true, version: "0.1.0" };
    const result = parseNativeResponse(raw);
    expect(result).toEqual({ type: "status_result", ok: true, version: "0.1.0" });
  });

  it("parses error response correctly", () => {
    const raw = { type: "error", message: "something went wrong" };
    const result = parseNativeResponse(raw);
    expect(result).toEqual({ type: "error", message: "something went wrong" });
  });

  it("rejects null", () => {
    expect(() => parseNativeResponse(null)).toThrow("not an object");
  });

  it("rejects undefined", () => {
    expect(() => parseNativeResponse(undefined)).toThrow("not an object");
  });

  it("rejects primitive", () => {
    expect(() => parseNativeResponse("hello")).toThrow("not an object");
  });

  it("rejects missing type field", () => {
    expect(() => parseNativeResponse({ id: "abc" })).toThrow("missing 'type' field");
  });

  it("rejects numeric type field", () => {
    expect(() => parseNativeResponse({ type: 42 })).toThrow("missing 'type' field");
  });

  it("rejects unknown type", () => {
    expect(() => parseNativeResponse({ type: "unknown_thing" })).toThrow("Unknown native response type");
  });

  it("rejects save_result with missing id", () => {
    expect(() => parseNativeResponse({ type: "save_result", path: "/tmp", status: "created" })).toThrow(
      "missing required fields",
    );
  });

  it("rejects save_result with wrong field types", () => {
    expect(() => parseNativeResponse({ type: "save_result", id: 123, path: "/tmp", status: "created" })).toThrow(
      "missing required fields",
    );
  });

  it("rejects status_result with missing ok", () => {
    expect(() => parseNativeResponse({ type: "status_result", version: "1.0" })).toThrow("missing required fields");
  });

  it("rejects status_result with string ok", () => {
    expect(() => parseNativeResponse({ type: "status_result", ok: "true", version: "1.0" })).toThrow(
      "missing required fields",
    );
  });

  it("rejects error with missing message", () => {
    expect(() => parseNativeResponse({ type: "error" })).toThrow("missing 'message' field");
  });

  it("rejects error with numeric message", () => {
    expect(() => parseNativeResponse({ type: "error", message: 42 })).toThrow("missing 'message' field");
  });

  it("ignores extra fields on save_result", () => {
    const raw = { type: "save_result", id: "abc", path: "/tmp", status: "created", extra: true };
    const result = parseNativeResponse(raw);
    expect(result).toEqual({ type: "save_result", id: "abc", path: "/tmp", status: "created" });
  });

  // -- list_collections_result --

  it("parses list_collections_result correctly", () => {
    const raw = { type: "list_collections_result", collections: ["reading", "work"] };
    const result = parseNativeResponse(raw);
    expect(result).toEqual({ type: "list_collections_result", collections: ["reading", "work"] });
  });

  it("parses list_collections_result with empty array", () => {
    const raw = { type: "list_collections_result", collections: [] };
    const result = parseNativeResponse(raw);
    expect(result).toEqual({ type: "list_collections_result", collections: [] });
  });

  it("rejects list_collections_result with missing collections", () => {
    expect(() => parseNativeResponse({ type: "list_collections_result" })).toThrow(
      "missing 'collections' array",
    );
  });

  it("rejects list_collections_result with non-array collections", () => {
    expect(() =>
      parseNativeResponse({ type: "list_collections_result", collections: "not-array" }),
    ).toThrow("missing 'collections' array");
  });

  // -- list_result --

  it("parses list_result with bookmarks correctly", () => {
    const raw = {
      type: "list_result",
      bookmarks: [
        {
          id: "am_123",
          url: "https://example.com",
          title: "Example",
          state: "inbox",
          user_tags: ["rust"],
          suggested_tags: ["dev"],
          saved_at: "2026-03-12T00:00:00Z",
        },
      ],
    };
    const result = parseNativeResponse(raw);
    expect(result).toEqual({
      type: "list_result",
      bookmarks: [
        {
          id: "am_123",
          url: "https://example.com",
          title: "Example",
          state: "inbox",
          user_tags: ["rust"],
          suggested_tags: ["dev"],
          saved_at: "2026-03-12T00:00:00Z",
        },
      ],
    });
  });

  it("parses list_result with empty bookmarks array", () => {
    const raw = { type: "list_result", bookmarks: [] };
    const result = parseNativeResponse(raw);
    expect(result).toEqual({ type: "list_result", bookmarks: [] });
  });

  it("rejects list_result with missing bookmarks", () => {
    expect(() => parseNativeResponse({ type: "list_result" })).toThrow(
      "missing 'bookmarks' array",
    );
  });

  it("rejects list_result with non-array bookmarks", () => {
    expect(() =>
      parseNativeResponse({ type: "list_result", bookmarks: "not-array" }),
    ).toThrow("missing 'bookmarks' array");
  });

  it("rejects list_result with malformed bookmark object", () => {
    expect(() =>
      parseNativeResponse({
        type: "list_result",
        bookmarks: [{ id: "am_123" }],
      }),
    ).toThrow("bookmark[0] has invalid or missing fields");
  });

  it("rejects list_result with invalid state value", () => {
    expect(() =>
      parseNativeResponse({
        type: "list_result",
        bookmarks: [
          {
            id: "am_123",
            url: "https://example.com",
            title: "Example",
            state: "deleted",
            user_tags: [],
            suggested_tags: [],
            saved_at: "2026-03-12T00:00:00Z",
          },
        ],
      }),
    ).toThrow("bookmark[0] has invalid or missing fields");
  });

  it("rejects list_result with non-object bookmark entry", () => {
    expect(() =>
      parseNativeResponse({
        type: "list_result",
        bookmarks: ["not-an-object"],
      }),
    ).toThrow("bookmark[0] is not an object");
  });

  it("rejects list_result with non-string user_tags entries", () => {
    expect(() =>
      parseNativeResponse({
        type: "list_result",
        bookmarks: [
          {
            id: "am_123",
            url: "https://example.com",
            title: "Example",
            state: "inbox",
            user_tags: ["valid", 42, null],
            suggested_tags: [],
            saved_at: "2026-03-12T00:00:00Z",
          },
        ],
      }),
    ).toThrow("bookmark[0] has non-string user_tags");
  });

  it("rejects list_result with non-string suggested_tags entries", () => {
    expect(() =>
      parseNativeResponse({
        type: "list_result",
        bookmarks: [
          {
            id: "am_123",
            url: "https://example.com",
            title: "Example",
            state: "inbox",
            user_tags: [],
            suggested_tags: [true, "valid"],
            saved_at: "2026-03-12T00:00:00Z",
          },
        ],
      }),
    ).toThrow("bookmark[0] has non-string suggested_tags");
  });
});

describe("isErrorResponse", () => {
  it("returns true for error responses", () => {
    expect(isErrorResponse({ type: "error", message: "fail" })).toBe(true);
  });

  it("returns false for save_result", () => {
    expect(isErrorResponse({ type: "save_result", id: "1", path: "/a", status: "created" })).toBe(false);
  });

  it("returns false for status_result", () => {
    expect(isErrorResponse({ type: "status_result", ok: true, version: "1.0" })).toBe(false);
  });
});

describe("parseNativeResponse - show_result", () => {
  const validBookmark = {
    id: "am_1",
    url: "https://example.com",
    title: "Test",
    summary: "A summary",
    saved_at: "2026-03-12T00:00:00Z",
    capture_source: "cli",
    state: "inbox",
    user_tags: ["rust"],
    suggested_tags: ["ai"],
    collections: ["reading"],
    note: null,
  };

  it("parses valid show_result", () => {
    const result = parseNativeResponse({ type: "show_result", bookmark: validBookmark });
    expect(result.type).toBe("show_result");
    if (result.type === "show_result") {
      expect(result.bookmark.id).toBe("am_1");
      expect(result.bookmark.summary).toBe("A summary");
      expect(result.bookmark.note).toBeNull();
    }
  });

  it("rejects show_result with missing bookmark", () => {
    expect(() => parseNativeResponse({ type: "show_result" })).toThrow("missing 'bookmark' object");
  });

  it("rejects show_result with invalid bookmark fields", () => {
    expect(() => parseNativeResponse({
      type: "show_result",
      bookmark: { ...validBookmark, state: "invalid" },
    })).toThrow("invalid or missing fields");
  });

  it("rejects show_result with non-string collections", () => {
    expect(() => parseNativeResponse({
      type: "show_result",
      bookmark: { ...validBookmark, collections: [1, 2] },
    })).toThrow("non-string collections");
  });

  it("accepts show_result with null summary", () => {
    const result = parseNativeResponse({
      type: "show_result",
      bookmark: { ...validBookmark, summary: null },
    });
    if (result.type === "show_result") {
      expect(result.bookmark.summary).toBeNull();
    }
  });

  it("rejects show_result with numeric summary", () => {
    expect(() => parseNativeResponse({
      type: "show_result",
      bookmark: { ...validBookmark, summary: 42 },
    })).toThrow("invalid summary");
  });
});

describe("parseNativeResponse - update_result", () => {
  const validBookmark = {
    id: "am_1",
    url: "https://example.com",
    title: "Test",
    summary: "A summary",
    saved_at: "2026-03-12T00:00:00Z",
    capture_source: "cli",
    state: "processed",
    user_tags: ["rust", "ai"],
    suggested_tags: [],
    collections: [],
    note: "Updated note",
  };

  it("parses valid update_result", () => {
    const result = parseNativeResponse({ type: "update_result", bookmark: validBookmark });
    expect(result.type).toBe("update_result");
    if (result.type === "update_result") {
      expect(result.bookmark.state).toBe("processed");
      expect(result.bookmark.note).toBe("Updated note");
    }
  });

  it("rejects update_result with missing bookmark", () => {
    expect(() => parseNativeResponse({ type: "update_result" })).toThrow("missing 'bookmark' object");
  });

  it("rejects update_result with non-string user_tags", () => {
    expect(() => parseNativeResponse({
      type: "update_result",
      bookmark: { ...validBookmark, user_tags: [42] },
    })).toThrow("non-string user_tags");
  });

  it("rejects update_result with invalid note type", () => {
    expect(() => parseNativeResponse({
      type: "update_result",
      bookmark: { ...validBookmark, note: 123 },
    })).toThrow("invalid note");
  });
});
