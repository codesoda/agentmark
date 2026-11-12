import { describe, it, expect } from "vitest";
import { parseNativeResponse } from "./types";
import { CONTRACT_SETUP_STATE } from "../test/fixtures";

const clone = <T>(value: T): T => JSON.parse(JSON.stringify(value)) as T;

describe("status_result (contract §2.1)", () => {
  it("keeps initialized", () => {
    expect(parseNativeResponse({ type: "status_result", ok: true, version: "0.3.0", initialized: false })).toEqual({
      type: "status_result",
      ok: true,
      version: "0.3.0",
      initialized: false,
    });
  });

  it("accepts an old CLI's status without initialized", () => {
    const parsed = parseNativeResponse({ type: "status_result", ok: true, version: "0.2.6" });
    expect(parsed).toEqual({ type: "status_result", ok: true, version: "0.2.6" });
    expect("initialized" in parsed).toBe(false);
  });

  it("rejects a non-boolean initialized", () => {
    expect(() => parseNativeResponse({ type: "status_result", ok: true, version: "0.3.0", initialized: "no" })).toThrow(
      "non-boolean 'initialized'",
    );
  });
});

describe("error (contract §2.5)", () => {
  const message = "not initialized: AgentMark is not set up yet. Finish setup in the extension or the AgentMark app.";

  it("keeps the setup_required code", () => {
    expect(parseNativeResponse({ type: "error", message, code: "setup_required" })).toEqual({
      type: "error",
      message,
      code: "setup_required",
    });
  });

  it.each(["something_else", 7, null])("drops an unknown code %j", (code) => {
    expect(parseNativeResponse({ type: "error", message: "x", code })).toEqual({ type: "error", message: "x" });
  });
});

describe("setup_state_result / save_settings_result (contract §2.2, §2.3)", () => {
  it.each(["setup_state_result", "save_settings_result"])("parses the contract example as %s", (type) => {
    expect(parseNativeResponse({ type, state: clone(CONTRACT_SETUP_STATE) })).toEqual({
      type,
      state: CONTRACT_SETUP_STATE,
    });
  });

  it("accepts every nullable field as null", () => {
    const state = clone(CONTRACT_SETUP_STATE) as Record<string, unknown>;
    state.config_problem = null;
    state.bookmark_count = null;
    (state.settings as Record<string, unknown>).system_prompt = null;
    const parsed = parseNativeResponse({ type: "setup_state_result", state });
    expect(parsed.type).toBe("setup_state_result");
  });

  it.each<[string, (s: Record<string, unknown>) => void]>([
    ["a missing state", (s) => Object.keys(s).forEach((k) => delete s[k])],
    ["initialized as a string", (s) => (s.initialized = "yes")],
    ["a missing platform", (s) => delete s.platform],
    ["bookmark_count as a string", (s) => (s.bookmark_count = "42")],
    ["agents not an array", (s) => (s.agents = {})],
    ["an agent without found", (s) => delete (s.agents as Array<Record<string, unknown>>)[0].found],
    ["an agent path as a number", (s) => ((s.agents as Array<Record<string, unknown>>)[1].path = 1)],
    ["builtin sites with a number", (s) => (s.builtin_snapshot_sites = ["x.com", 1])],
    ["settings missing", (s) => delete s.settings],
    ["enrichment_enabled as a string", (s) => ((s.settings as Record<string, unknown>).enrichment_enabled = "yes")],
    ["snapshot_sites not an array", (s) => ((s.settings as Record<string, unknown>).snapshot_sites = "x.com")],
    ["system_prompt as a number", (s) => ((s.settings as Record<string, unknown>).system_prompt = 3)],
  ])("rejects %s", (_name, breakIt) => {
    const state = clone(CONTRACT_SETUP_STATE) as unknown as Record<string, unknown>;
    breakIt(state);
    expect(() => parseNativeResponse({ type: "setup_state_result", state })).toThrow(/setup_state_result/);
  });

  it("rejects a missing state object", () => {
    expect(() => parseNativeResponse({ type: "save_settings_result" })).toThrow("missing 'state' object");
  });
});

describe("settings_invalid (contract §2.3)", () => {
  const example = {
    type: "settings_invalid",
    errors: [
      { field: "storage_path", index: null, code: "not_writable", message: "AgentMark cannot write to /Volumes/X." },
      { field: "snapshot_sites", index: 2, code: "invalid_site", message: "\"exa mple.com\" is not a host/path rule." },
    ],
  };

  it("parses the contract example", () => {
    expect(parseNativeResponse(clone(example))).toEqual(example);
  });

  it("keeps an error code this extension does not know, so its message still shows", () => {
    const parsed = parseNativeResponse({
      type: "settings_invalid",
      errors: [{ field: "storage_path", index: null, code: "on_fire", message: "Hot." }],
    });
    expect(parsed).toEqual({
      type: "settings_invalid",
      errors: [{ field: "storage_path", index: null, code: "on_fire", message: "Hot." }],
    });
  });

  it.each([
    ["no errors", { errors: [] }],
    ["errors not an array", { errors: {} }],
    ["an unknown field", { errors: [{ field: "colour", index: null, code: "blank", message: "x" }] }],
    ["a negative index", { errors: [{ field: "snapshot_sites", index: -1, code: "invalid_site", message: "x" }] }],
    ["a missing message", { errors: [{ field: "storage_path", index: null, code: "blank" }] }],
  ])("rejects %s", (_name, body) => {
    expect(() => parseNativeResponse({ type: "settings_invalid", ...body })).toThrow(/settings_invalid/);
  });
});

describe("choose_folder_result (contract §2.4)", () => {
  it.each([
    { type: "choose_folder_result", outcome: "chosen", path: "/Users/me/Bookmarks", path_display: "~/Bookmarks", reason: null },
    { type: "choose_folder_result", outcome: "cancelled", path: null, path_display: null, reason: null },
    {
      type: "choose_folder_result",
      outcome: "unavailable",
      path: null,
      path_display: null,
      reason: "No folder dialog is available on this system.",
    },
  ])("parses the $outcome example", (example) => {
    expect(parseNativeResponse(clone(example))).toEqual(example);
  });

  it.each([
    ["an unknown outcome", { outcome: "maybe", path: null, path_display: null, reason: null }],
    ["chosen without a path", { outcome: "chosen", path: null, path_display: null, reason: null }],
    ["a numeric reason", { outcome: "unavailable", path: null, path_display: null, reason: 5 }],
  ])("rejects %s", (_name, body) => {
    expect(() => parseNativeResponse({ type: "choose_folder_result", ...body })).toThrow(/choose_folder_result/);
  });
});
