import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";
import { ToolbarIcon, SAVE_RESULT_HOLD_MS, setTabIcon, toolbarIconPaths, type ToolbarLook } from "./toolbar-icon";
import type { RuntimeResponse } from "../shared/types";

const PAGE = "https://example.com/article";
const OTHER = "https://example.com/other";
const TAB = 3;

const savedResult = (saved: string[]): RuntimeResponse => ({
  success: true,
  data: { type: "lookup_urls_result", saved },
});
const saveOk: RuntimeResponse = {
  success: true,
  data: { type: "save_result", id: "1", path: "/a", status: "created" },
};
const saveFailed: RuntimeResponse = { success: false, error: "boom" };

describe("toolbar-icon", () => {
  let chromeMock: ReturnType<typeof resetChromeMock>;
  let request: ReturnType<typeof vi.fn>;
  let icon: ToolbarIcon;
  /** The URL chrome.tabs.get reports for TAB; change it to simulate navigation. */
  let tabUrl: string | undefined;

  function lastLook(tabId = TAB): ToolbarLook | undefined {
    const calls = chromeMock.action.setIcon.mock.calls.filter(([d]) => d.tabId === tabId);
    const path = calls[calls.length - 1]?.[0].path as Record<number, string> | undefined;
    return path?.[16].match(/toolbar-(.+)-16\.png$/)?.[1] as ToolbarLook | undefined;
  }

  /** Let every pending promise chain settle. */
  async function settle() {
    for (let i = 0; i < 20; i++) await Promise.resolve();
  }

  beforeEach(() => {
    vi.useFakeTimers();
    chromeMock = resetChromeMock();
    tabUrl = PAGE;
    chromeMock.tabs.get.mockImplementation(async (tabId: number) => ({ id: tabId, url: tabUrl }) as chrome.tabs.Tab);
    chromeMock.tabs.query.mockImplementation(async () => [{ id: TAB, url: tabUrl }]);
    request = vi.fn(async () => savedResult([]));
    icon = new ToolbarIcon(request);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("points each look at its 16/32/48 PNGs", () => {
    expect(toolbarIconPaths("saved-fresh")).toEqual({
      16: "icons/toolbar-saved-fresh-16.png",
      32: "icons/toolbar-saved-fresh-32.png",
      48: "icons/toolbar-saved-fresh-48.png",
    });
  });

  describe("refresh", () => {
    it.each(["chrome://extensions", "about:blank", "file:///tmp/a.html", undefined])(
      "paints %s idle without asking the native host",
      async (url) => {
        await icon.refresh(TAB, url);
        expect(lastLook()).toBe("idle");
        expect(request).not.toHaveBeenCalled();
      },
    );

    it("paints a bookmarked page saved", async () => {
      request.mockResolvedValue(savedResult([PAGE]));
      await icon.refresh(TAB, PAGE);
      expect(request).toHaveBeenCalledWith({ type: "lookup_urls", urls: [PAGE] });
      expect(lastLook()).toBe("saved");
    });

    it("paints an unknown page idle", async () => {
      request.mockResolvedValue(savedResult([OTHER]));
      await icon.refresh(TAB, PAGE);
      expect(lastLook()).toBe("idle");
    });

    it("drops an answer for a page the tab has left", async () => {
      request.mockImplementation(async () => {
        tabUrl = OTHER;
        return savedResult([PAGE]);
      });
      await icon.refresh(TAB, PAGE);
      expect(chromeMock.action.setIcon).not.toHaveBeenCalled();
    });

    it.each<[string, () => Promise<RuntimeResponse>]>([
      ["an error response", async () => ({ success: false, error: "Native host has exited" })],
      ["a rejected request", async () => Promise.reject(new Error("timeout"))],
      ["an unexpected response", async () => saveOk],
    ])("paints idle on %s and asks again next time", async (_name, failure) => {
      request.mockImplementationOnce(failure);
      await icon.refresh(TAB, PAGE);
      expect(lastLook()).toBe("idle");

      request.mockResolvedValue(savedResult([PAGE]));
      await icon.refresh(TAB, PAGE);
      expect(request).toHaveBeenCalledTimes(2);
      expect(lastLook()).toBe("saved");
    });

    it("does not ask again while the URL is unchanged, but still repaints", async () => {
      request.mockResolvedValue(savedResult([PAGE]));
      await icon.refresh(TAB, PAGE);
      await icon.refresh(TAB, PAGE);
      expect(request).toHaveBeenCalledTimes(1);
      expect(chromeMock.action.setIcon).toHaveBeenCalledTimes(2);
      expect(lastLook()).toBe("saved");

      tabUrl = OTHER;
      await icon.refresh(TAB, OTHER);
      expect(request).toHaveBeenCalledTimes(2);
    });

    it("shares one lookup between back-to-back events for the same page", async () => {
      await Promise.all([icon.refresh(TAB, PAGE), icon.refresh(TAB, PAGE)]);
      expect(request).toHaveBeenCalledTimes(1);
    });

    it("swallows a setIcon rejection", async () => {
      chromeMock.action.setIcon.mockRejectedValue(new Error("No tab with id: 3"));
      await expect(icon.refresh(TAB, "chrome://newtab")).resolves.toBeUndefined();
      await expect(setTabIcon(TAB, "saved")).resolves.toBeUndefined();
    });

    it("swallows a setIcon that throws synchronously", async () => {
      chromeMock.action.setIcon.mockImplementation(() => {
        throw new Error("No tab with id: 3");
      });
      await expect(setTabIcon(TAB, "idle")).resolves.toBeUndefined();
    });
  });

  describe("forget", () => {
    it("asks again after the tab was removed", async () => {
      await icon.refresh(TAB, PAGE);
      icon.handleTabRemoved(TAB);
      await icon.refresh(TAB, PAGE);
      expect(request).toHaveBeenCalledTimes(2);
    });
  });

  describe("save lifecycle", () => {
    it("shows saving, then saved-fresh, then saved after the hold", async () => {
      let finish!: (r: RuntimeResponse) => void;
      const save = new Promise<RuntimeResponse>((resolve) => (finish = resolve));

      void icon.trackTabSave(TAB, PAGE, save);
      await settle();
      expect(lastLook()).toBe("saving");

      finish(saveOk);
      await settle();
      expect(lastLook()).toBe("saved-fresh");

      await vi.advanceTimersByTimeAsync(SAVE_RESULT_HOLD_MS - 1);
      expect(lastLook()).toBe("saved-fresh");
      await vi.advanceTimersByTimeAsync(1);
      expect(lastLook()).toBe("saved");
      // The save itself told us the page is bookmarked.
      expect(request).not.toHaveBeenCalled();
    });

    it("shows error for the hold, then repaints from the index", async () => {
      void icon.trackTabSave(TAB, PAGE, Promise.resolve(saveFailed));
      await settle();
      expect(lastLook()).toBe("error");
      expect(request).not.toHaveBeenCalled();

      await vi.advanceTimersByTimeAsync(SAVE_RESULT_HOLD_MS);
      expect(request).toHaveBeenCalledWith({ type: "lookup_urls", urls: [PAGE] });
      expect(lastLook()).toBe("idle");
    });

    it("treats a rejected save as a failure", async () => {
      void icon.trackTabSave(TAB, PAGE, Promise.reject(new Error("gone")));
      await settle();
      expect(lastLook()).toBe("error");
    });

    it("keeps the save's look when the tab is refreshed meanwhile", async () => {
      request.mockResolvedValue(savedResult([PAGE]));
      void icon.trackTabSave(TAB, PAGE, new Promise<RuntimeResponse>(() => {}));
      await settle();

      await icon.refresh(TAB, PAGE);
      expect(request).not.toHaveBeenCalled();
      expect(chromeMock.action.setIcon.mock.calls.every(([d]) => (d.path as Record<number, string>)[16].includes("saving"))).toBe(true);
    });

    it("drops a lookup that finishes after a save of the page started", async () => {
      let answer!: (r: RuntimeResponse) => void;
      request.mockImplementationOnce(() => new Promise<RuntimeResponse>((resolve) => (answer = resolve)));
      const refreshing = icon.refresh(TAB, PAGE);
      void icon.trackTabSave(TAB, PAGE, new Promise<RuntimeResponse>(() => {}));
      await settle();

      answer(savedResult([]));
      await refreshing;
      expect(lastLook()).toBe("saving");
    });

    it("re-asks when a save finished while a lookup was in flight", async () => {
      let answer!: (r: RuntimeResponse) => void;
      request.mockImplementationOnce(() => new Promise<RuntimeResponse>((resolve) => (answer = resolve)));
      const refreshing = icon.refresh(7, PAGE);
      // The same page saved from another tab.
      await icon.trackTabSave(TAB, PAGE, Promise.resolve(saveOk));

      request.mockResolvedValue(savedResult([PAGE]));
      answer(savedResult([]));
      await refreshing;
      expect(request).toHaveBeenCalledTimes(2);
      expect(lastLook(7)).toBe("saved");
    });

    it("leaves the tab alone when the saved URL is a post, not the page", async () => {
      await icon.trackTabSave(TAB, OTHER, Promise.resolve(saveOk));
      await icon.trackActiveTabSave(OTHER, Promise.resolve(saveOk));
      await vi.advanceTimersByTimeAsync(SAVE_RESULT_HOLD_MS);
      expect(chromeMock.action.setIcon).not.toHaveBeenCalled();
    });

    it("marks the active tab when the popup saves its page", async () => {
      await icon.trackActiveTabSave(PAGE, Promise.resolve(saveOk));
      expect(chromeMock.tabs.query).toHaveBeenCalledWith({ active: true, lastFocusedWindow: true });
      expect(lastLook()).toBe("saved-fresh");
    });

    it("does nothing more once the tab navigated during the save", async () => {
      let finish!: (r: RuntimeResponse) => void;
      void icon.trackTabSave(TAB, PAGE, new Promise<RuntimeResponse>((resolve) => (finish = resolve)));
      await settle();
      tabUrl = OTHER;
      await icon.refresh(TAB, OTHER);
      chromeMock.action.setIcon.mockClear();

      finish(saveOk);
      await settle();
      await vi.advanceTimersByTimeAsync(SAVE_RESULT_HOLD_MS);
      expect(chromeMock.action.setIcon).not.toHaveBeenCalled();
    });

    it("does not repaint after the hold when the tab navigated during it", async () => {
      await icon.trackTabSave(TAB, PAGE, Promise.resolve(saveFailed));
      tabUrl = OTHER;
      await icon.refresh(TAB, OTHER);
      chromeMock.action.setIcon.mockClear();
      request.mockClear();

      await vi.advanceTimersByTimeAsync(SAVE_RESULT_HOLD_MS);
      expect(chromeMock.action.setIcon).not.toHaveBeenCalled();
      expect(request).not.toHaveBeenCalled();
    });

    it("forgets a held tab when it is removed", async () => {
      await icon.trackTabSave(TAB, PAGE, Promise.resolve(saveOk));
      icon.handleTabRemoved(TAB);
      chromeMock.action.setIcon.mockClear();
      await vi.advanceTimersByTimeAsync(SAVE_RESULT_HOLD_MS);
      expect(chromeMock.action.setIcon).not.toHaveBeenCalled();
    });

    it("never rejects when the tab cannot be read", async () => {
      chromeMock.tabs.get.mockRejectedValue(new Error("No tab with id: 3"));
      chromeMock.tabs.query.mockRejectedValue(new Error("no window"));
      await expect(icon.trackTabSave(TAB, PAGE, Promise.resolve(saveOk))).resolves.toBeUndefined();
      await expect(icon.trackActiveTabSave(PAGE, Promise.resolve(saveOk))).resolves.toBeUndefined();
      expect(chromeMock.action.setIcon).not.toHaveBeenCalled();
    });
  });

  describe("tab events with the tabs permission", () => {
    beforeEach(() => {
      chromeMock.permissions.contains.mockResolvedValue(true);
    });

    it("refreshes on a URL change or a finished load, not on other updates", async () => {
      icon.handleTabUpdated(TAB, { title: "x" }, { url: PAGE } as chrome.tabs.Tab);
      icon.handleTabUpdated(TAB, { status: "loading" }, { url: PAGE } as chrome.tabs.Tab);
      await settle();
      expect(chromeMock.action.setIcon).not.toHaveBeenCalled();

      icon.handleTabUpdated(TAB, { url: PAGE }, { url: PAGE } as chrome.tabs.Tab);
      await settle();
      expect(lastLook()).toBe("idle");
      icon.handleTabUpdated(TAB, { status: "complete" }, { url: PAGE } as chrome.tabs.Tab);
      await settle();
      expect(chromeMock.action.setIcon).toHaveBeenCalledTimes(2);
      expect(request).toHaveBeenCalledTimes(1);
    });

    it("refreshes the tab the user switches to", async () => {
      request.mockResolvedValue(savedResult([PAGE]));
      icon.handleTabActivated({ tabId: TAB, windowId: 1 });
      await settle();
      expect(chromeMock.tabs.get).toHaveBeenCalledWith(TAB);
      expect(lastLook()).toBe("saved");
    });
  });

  describe("tab events without the tabs permission", () => {
    it("ignores tab events, which carry no URLs then, and never asks the index", async () => {
      chromeMock.permissions.contains.mockResolvedValue(false);
      icon.handleTabUpdated(TAB, { status: "complete" }, { id: TAB } as chrome.tabs.Tab);
      icon.handleTabActivated({ tabId: TAB, windowId: 1 });
      await settle();

      expect(chromeMock.permissions.contains).toHaveBeenCalledWith({ permissions: ["tabs"] });
      expect(chromeMock.action.setIcon).not.toHaveBeenCalled();
      expect(request).not.toHaveBeenCalled();
    });

    it("asks Chrome for the permission once and remembers the answer", async () => {
      chromeMock.permissions.contains.mockResolvedValue(false);
      icon.handleTabUpdated(TAB, { status: "complete" }, { id: TAB } as chrome.tabs.Tab);
      icon.handleTabUpdated(TAB, { status: "complete" }, { id: TAB } as chrome.tabs.Tab);
      await settle();
      icon.handleTabActivated({ tabId: TAB, windowId: 1 });
      await settle();
      expect(chromeMock.permissions.contains).toHaveBeenCalledTimes(1);
    });

    it("treats a failing permissions check as no permission", async () => {
      chromeMock.permissions.contains.mockRejectedValue(new Error("nope"));
      icon.handleTabUpdated(TAB, { status: "complete" }, { url: PAGE } as chrome.tabs.Tab);
      await settle();
      expect(request).not.toHaveBeenCalled();
    });

    it("still shows a popup save of the active tab (the click grants activeTab)", async () => {
      chromeMock.permissions.contains.mockResolvedValue(false);
      await icon.trackActiveTabSave(PAGE, Promise.resolve(saveOk));
      expect(lastLook()).toBe("saved-fresh");
    });
  });

  describe("permission granted or revoked while running", () => {
    it("starts using tab events once granted, and repaints the active tab", async () => {
      chromeMock.permissions.contains.mockResolvedValue(false);
      icon.handleTabUpdated(TAB, { status: "complete" }, { url: PAGE } as chrome.tabs.Tab);
      await settle();
      expect(request).not.toHaveBeenCalled();

      request.mockResolvedValue(savedResult([PAGE]));
      icon.handlePermissionsAdded({ permissions: ["tabs"] });
      await settle();
      expect(lastLook()).toBe("saved");

      icon.handleTabUpdated(TAB, { url: OTHER }, { url: OTHER } as chrome.tabs.Tab);
      tabUrl = OTHER;
      await settle();
      expect(request).toHaveBeenLastCalledWith({ type: "lookup_urls", urls: [OTHER] });
    });

    it("ignores grants of other permissions", async () => {
      chromeMock.permissions.contains.mockResolvedValue(false);
      icon.handlePermissionsAdded({ permissions: ["notifications"] });
      icon.handleTabUpdated(TAB, { status: "complete" }, { url: PAGE } as chrome.tabs.Tab);
      await settle();
      expect(request).not.toHaveBeenCalled();
    });

    it("stops using tab events when revoked, and forgets cached answers", async () => {
      chromeMock.permissions.contains.mockResolvedValue(true);
      request.mockResolvedValue(savedResult([PAGE]));
      icon.handleTabUpdated(TAB, { status: "complete" }, { url: PAGE } as chrome.tabs.Tab);
      await settle();
      expect(request).toHaveBeenCalledTimes(1);

      icon.handlePermissionsRemoved({ permissions: ["tabs"] });
      icon.handleTabUpdated(TAB, { status: "complete" }, { url: PAGE } as chrome.tabs.Tab);
      icon.handleTabActivated({ tabId: TAB, windowId: 1 });
      await settle();
      expect(request).toHaveBeenCalledTimes(1);

      // The cache went with the permission: a report asks the index again.
      await icon.reportPage(TAB, PAGE);
      expect(request).toHaveBeenCalledTimes(2);
    });

    it("lets a grant that lands during the first permissions check win", async () => {
      let answer!: (value: boolean) => void;
      chromeMock.permissions.contains.mockImplementation(() => new Promise<boolean>((r) => { answer = r; }));
      icon.handleTabUpdated(TAB, { status: "complete" }, { url: PAGE } as chrome.tabs.Tab);
      icon.handlePermissionsAdded({ permissions: ["tabs"] });
      answer(false);
      await settle();
      expect(await icon.hasTabsPermission()).toBe(true);
    });
  });

  describe("reportPage (a content script reports its page)", () => {
    beforeEach(() => {
      chromeMock.permissions.contains.mockResolvedValue(false);
      // Without `tabs`, Chrome does not tell us the tab's URL.
      chromeMock.tabs.get.mockImplementation(async (tabId: number) => ({ id: tabId }) as chrome.tabs.Tab);
    });

    it("paints a saved page blue without reading the tab", async () => {
      request.mockResolvedValue(savedResult([PAGE]));
      await icon.reportPage(TAB, PAGE);
      expect(request).toHaveBeenCalledWith({ type: "lookup_urls", urls: [PAGE] });
      expect(lastLook()).toBe("saved");
    });

    it("paints an unsaved page idle", async () => {
      await icon.reportPage(TAB, PAGE);
      expect(lastLook()).toBe("idle");
    });

    it("uses the cached answer for the same page", async () => {
      request.mockResolvedValue(savedResult([PAGE]));
      await icon.reportPage(TAB, PAGE);
      await icon.reportPage(TAB, PAGE);
      expect(request).toHaveBeenCalledTimes(1);
      expect(lastLook()).toBe("saved");
    });

    it("drops a late answer when the tab has reported another page since", async () => {
      let answer!: (r: RuntimeResponse) => void;
      request.mockImplementationOnce(() => new Promise<RuntimeResponse>((r) => { answer = r; }));
      const first = icon.reportPage(TAB, PAGE);
      await icon.reportPage(TAB, OTHER);
      expect(lastLook()).toBe("idle");
      answer(savedResult([PAGE]));
      await first;
      expect(lastLook()).toBe("idle");
    });

    it("drops a late answer when the tab navigated to another URL", async () => {
      let answer!: (r: RuntimeResponse) => void;
      request.mockImplementationOnce(() => new Promise<RuntimeResponse>((r) => { answer = r; }));
      const report = icon.reportPage(TAB, PAGE);
      icon.handleTabUpdated(TAB, { url: OTHER }, { id: TAB } as chrome.tabs.Tab);
      answer(savedResult([PAGE]));
      await report;
      expect(chromeMock.action.setIcon).not.toHaveBeenCalled();
    });

    it("shows idle when the host cannot answer", async () => {
      request.mockResolvedValue({ success: false, error: "Specified native messaging host not found." });
      await icon.reportPage(TAB, PAGE);
      expect(lastLook()).toBe("idle");
    });

    it("leaves a running save's look alone", async () => {
      let finish!: (r: RuntimeResponse) => void;
      // A popup save: the click's activeTab grant lets the worker read this tab's URL.
      chromeMock.tabs.get.mockImplementation(async (tabId: number) => ({ id: tabId, url: PAGE }) as chrome.tabs.Tab);
      const saving = icon.trackTabSave(TAB, PAGE, new Promise<RuntimeResponse>((r) => { finish = r; }));
      await settle();
      await icon.reportPage(TAB, PAGE);
      expect(lastLook()).toBe("saving");
      finish(saveOk);
      await saving;
    });
  });
});
