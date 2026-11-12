import { describe, it, expect, beforeEach } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";
import { sendChooseFolder, sendGetSetupState, sendSaveSettings, sendStatus } from "./runtime";
import { clearSetupDraft, loadSetupDraft, SETUP_DRAFT_MAX_AGE_MS, updateSetupDraft } from "./setup-draft";
import { SETUP_DRAFT_KEY } from "./constants";
import { BADGE, BOOKMARK, PLUS_PATH } from "../content/save-button";
import { MARK_BADGE_PATH, MARK_BOOKMARK_PATH, MARK_PLUS_PATH } from "./AgentMarkMark";
import { readFileSync } from "fs";
import { resolve } from "path";

let chromeMock: ReturnType<typeof resetChromeMock>;

beforeEach(() => {
  chromeMock = resetChromeMock();
});

describe("setup runtime helpers (contract §3.2)", () => {
  it("sends the literal messages", async () => {
    chromeMock.runtime.sendMessage.mockResolvedValue({ success: true, data: { type: "status_result", ok: true, version: "0.3.0" } });
    await sendStatus();
    await sendGetSetupState();
    await sendSaveSettings({ enrichment_enabled: false });
    await sendChooseFolder("~/Documents");
    await sendChooseFolder();
    expect(chromeMock.runtime.sendMessage.mock.calls.map(([message]) => message)).toEqual([
      { type: "status" },
      { type: "get_setup_state" },
      { type: "save_settings", settings: { enrichment_enabled: false } },
      { type: "choose_folder", start_path: "~/Documents" },
      { type: "choose_folder" },
    ]);
  });

  it("keeps the setup_required code from the service worker", async () => {
    chromeMock.runtime.sendMessage.mockResolvedValue({ success: false, error: "not initialized: x", code: "setup_required" });
    expect(await sendGetSetupState()).toEqual({ success: false, error: "not initialized: x", code: "setup_required" });
  });

  it("drops any other code", async () => {
    chromeMock.runtime.sendMessage.mockResolvedValue({ success: false, error: "boom", code: "weird" });
    expect(await sendStatus()).toEqual({ success: false, error: "boom" });
  });

  it("turns a missing service worker into a connection error", async () => {
    chromeMock.runtime.sendMessage.mockRejectedValue(new Error("Could not establish connection. Receiving end does not exist."));
    expect(await sendStatus()).toEqual({
      success: false,
      error: "Could not establish connection. Receiving end does not exist.",
    });
  });
});

describe("setup draft (contract §4.4)", () => {
  it("merges changes and stamps the time", async () => {
    await updateSetupDraft({ storage_path: "~/A" }, 1000);
    await updateSetupDraft({ default_agent: "pi" }, 2000);
    expect(chromeMock.storage.session.data[SETUP_DRAFT_KEY]).toEqual({
      storage_path: "~/A",
      default_agent: "pi",
      enrichment_enabled: null,
      pending_save: null,
      updated_at: 2000,
    });
    expect(await loadSetupDraft(2000 + SETUP_DRAFT_MAX_AGE_MS)).not.toBeNull();
    expect(await loadSetupDraft(2001 + SETUP_DRAFT_MAX_AGE_MS)).toBeNull();
  });

  it("does not lose a change when two are made at once", async () => {
    await Promise.all([updateSetupDraft({ storage_path: "~/A" }), updateSetupDraft({ enrichment_enabled: false })]);
    expect(await loadSetupDraft()).toMatchObject({ storage_path: "~/A", enrichment_enabled: false });
  });

  it("clears after pending writes", async () => {
    void updateSetupDraft({ storage_path: "~/A" });
    await clearSetupDraft();
    expect(chromeMock.storage.session.data[SETUP_DRAFT_KEY]).toBeUndefined();
  });

  it("keeps a pending in-page save through other changes, and drops a malformed one", async () => {
    const post = { url: "https://x.com/ada/status/1", title: "Post by Ada", page_html: "<p>post</p>" };
    await updateSetupDraft({ pending_save: post }, 1000);
    await updateSetupDraft({ storage_path: "~/B" }, 2000);
    expect((await loadSetupDraft(2000))?.pending_save).toEqual(post);
    chromeMock.storage.session.data[SETUP_DRAFT_KEY] = { pending_save: { title: "no url" }, updated_at: 3000 };
    expect((await loadSetupDraft(3000))?.pending_save).toBeNull();
  });

  it("ignores malformed drafts", async () => {
    chromeMock.storage.session.data[SETUP_DRAFT_KEY] = { storage_path: 5 };
    expect(await loadSetupDraft()).toBeNull();
    chromeMock.storage.session.data[SETUP_DRAFT_KEY] = { storage_path: 5, default_agent: "pi", updated_at: Date.now() };
    expect(await loadSetupDraft()).toMatchObject({ storage_path: null, default_agent: "pi", enrichment_enabled: null });
  });

  it("never throws when session storage fails", async () => {
    chromeMock.storage.session.get.mockRejectedValue(new Error("nope"));
    chromeMock.storage.session.set.mockRejectedValue(new Error("nope"));
    await expect(updateSetupDraft({ storage_path: "~/A" })).resolves.toBeUndefined();
    await expect(loadSetupDraft()).resolves.toBeNull();
  });
});

describe("AgentMark mark", () => {
  it("draws the same shapes as the in-page badge and the toolbar icons", () => {
    expect(MARK_BADGE_PATH).toBe(BADGE);
    expect(MARK_BOOKMARK_PATH).toBe(BOOKMARK);
    expect(MARK_PLUS_PATH).toBe(PLUS_PATH);
    expect(readFileSync(resolve(__dirname, "../../icons/toolbar-saved.svg"), "utf-8")).toContain(MARK_BADGE_PATH);
    const idleSvg = readFileSync(resolve(__dirname, "../../icons/toolbar-idle.svg"), "utf-8");
    // Idle is the grey outline: hollow until a bookmark exists, like the in-page badge.
    expect(idleSvg).toContain(MARK_BOOKMARK_PATH);
    expect(idleSvg).toContain('stroke="#6b7280"');
    expect(idleSvg).toContain('fill="#6b7280"');
    expect(idleSvg).not.toContain(MARK_PLUS_PATH);
    expect(readFileSync(resolve(__dirname, "../../icons/icon.svg"), "utf-8")).toContain(MARK_PLUS_PATH);
    expect(readFileSync(resolve(__dirname, "../../icons/toolbar-saved.svg"), "utf-8")).toContain('fill="#4f46e5"');
  });
});
