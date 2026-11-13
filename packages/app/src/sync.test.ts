/**
 * The app copies a few things from the extension because it cannot import
 * from it. These tests read the extension's source as text and fail when the
 * copies drift.
 */

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { MARK_BADGE_PATH, MARK_BOOKMARK_PATH, MARK_PLUS_PATH } from "./components/AgentMarkMark";
import { SITE_URL, WEB_STORE_URL } from "./constants";
import { FOLDER_MOVE_NOTE, SUMMARIES_LABEL, SUMMARIES_NOTE, SUMMARIES_OFF_NOTE } from "./copy";
import { buildChanges, formFromSettings } from "./settings-form";
import { readyState } from "./mock/fixtures";

const extension = (path: string) => readFileSync(resolve(__dirname, "../../extension", path), "utf8");

describe("copies of extension code", () => {
  it("draws the same mark (the shapes are not redrawn)", () => {
    const source = extension("src/shared/AgentMarkMark.tsx");
    for (const path of [MARK_BADGE_PATH, MARK_BOOKMARK_PATH, MARK_PLUS_PATH]) expect(source).toContain(path);
    const icon = extension("icons/icon.svg");
    expect(icon).toContain(MARK_BOOKMARK_PATH);
  });

  it("uses the extension's summaries wording", () => {
    const source = extension("src/shared/settings-shared.ts");
    expect(source).toContain(SUMMARIES_LABEL);
    expect(source).toContain(SUMMARIES_NOTE);
    expect(source).toContain(SUMMARIES_OFF_NOTE);
  });

  it("uses the extension's storage folder warning", () => {
    expect(extension("src/shared/settings-shared.ts")).toContain(FOLDER_MOVE_NOTE.replace(/\s+/g, " "));
  });

  it("has the contract's constants (§7.1)", () => {
    expect(SITE_URL).toBe("https://agentmark.app");
    expect(WEB_STORE_URL === "" || WEB_STORE_URL.startsWith("https://chromewebstore.google.com/")).toBe(true);
  });
});

describe("settings form", () => {
  it("has nothing to save when unchanged", () => {
    const settings = readyState().settings;
    expect(buildChanges(settings, formFromSettings(settings))).toEqual({});
  });

  it("does not treat the absolute form of the same folder as a change", () => {
    const settings = readyState().settings;
    expect(buildChanges(settings, { ...formFromSettings(settings), storage_path: settings.storage_path })).toEqual({});
  });
});
