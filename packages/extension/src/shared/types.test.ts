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
