/// <reference types="chrome" />

/**
 * Per-tab toolbar icon: shows whether the tab's page is bookmarked and how a
 * save of it is going. Blue only ever means "saved".
 *
 * All state here is an in-memory optimisation. The MV3 worker can be killed at
 * any time and Chrome resets a tab's icon when the tab navigates, so every
 * navigation repaints from the index and a cold start behaves correctly.
 */

import type { NativeRequest, RuntimeResponse } from "../shared/types";
import { isSupportedUrl } from "../shared/runtime";

export type ToolbarLook = "idle" | "saving" | "saved-fresh" | "saved" | "error";

/** How long a finished save shows its tick or error. Matches the in-page badge's SAVED_TICK_MS. */
export const SAVE_RESULT_HOLD_MS = 5_000;

/** Performs a native request; injected so this module never touches the native port. */
export type NativeRequester = (request: NativeRequest) => Promise<RuntimeResponse>;

const ICON_SIZES = [16, 32, 48] as const;

export function toolbarIconPaths(look: ToolbarLook): Record<number, string> {
  const paths: Record<number, string> = {};
  for (const size of ICON_SIZES) paths[size] = `icons/toolbar-${look}-${size}.png`;
  return paths;
}

/** Paint a tab's icon. Never throws: the tab may already be gone. */
export async function setTabIcon(tabId: number, look: ToolbarLook): Promise<void> {
  try {
    await chrome.action.setIcon({ tabId, path: toolbarIconPaths(look) });
  } catch {
    // Tab closed or not paintable — nothing to update.
  }
}

async function currentTabUrl(tabId: number): Promise<string | undefined> {
  try {
    const tab = await chrome.tabs.get(tabId);
    return tab?.url;
  } catch {
    return undefined;
  }
}

async function activeTab(): Promise<chrome.tabs.Tab | undefined> {
  try {
    const tabs = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
    return Array.isArray(tabs) ? tabs[0] : undefined;
  } catch {
    return undefined;
  }
}

/** A tab whose icon belongs to a save until the result hold ends. */
interface SaveHold {
  url: string;
  look: ToolbarLook;
  timer?: ReturnType<typeof setTimeout>;
}

/** Tracks and paints the toolbar icon of every tab. One instance per worker lifetime. */
export class ToolbarIcon {
  /** Last index answer per tab, to skip lookups when the URL has not changed. */
  private readonly known = new Map<number, { url: string; saved: boolean }>();
  /** Lookups in flight, so the url-change and load-complete events share one. */
  private readonly inflight = new Map<number, { url: string; done: Promise<void> }>();
  private readonly holds = new Map<number, SaveHold>();
  /** Bumped whenever a save finishes, so a lookup that raced it is not trusted. */
  private saveEpoch = 0;

  constructor(private readonly request: NativeRequester) {}

  /** Paint idle or saved for a tab from the index. Never throws. */
  async refresh(tabId: number, url: string | undefined): Promise<void> {
    const hold = this.holds.get(tabId);
    if (hold && hold.url === url) {
      // A save owns this icon; only restore its look in case Chrome reset it.
      await setTabIcon(tabId, hold.look);
      return;
    }
    if (!isSupportedUrl(url)) {
      this.known.delete(tabId);
      await setTabIcon(tabId, "idle");
      return;
    }
    const cached = this.known.get(tabId);
    if (cached?.url === url) {
      await setTabIcon(tabId, cached.saved ? "saved" : "idle");
      return;
    }
    const pending = this.inflight.get(tabId);
    if (pending?.url === url) return pending.done;

    const done = this.lookUp(tabId, url).finally(() => {
      if (this.inflight.get(tabId)?.done === done) this.inflight.delete(tabId);
    });
    this.inflight.set(tabId, { url, done });
    return done;
  }

  /** Show a save's progress on a tab, but only if the tab is on that URL. Never throws. */
  async trackTabSave(tabId: number, url: string, save: Promise<RuntimeResponse>): Promise<void> {
    try {
      // A save of something else on the page (an in-page post) says nothing about the page.
      if ((await currentTabUrl(tabId)) !== url) return;
      await this.track(tabId, url, save);
    } catch {
      // Icon work must never affect the save.
    }
  }

  /** As trackTabSave, for the active tab of the last focused window (the popup's tab). */
  async trackActiveTabSave(url: string, save: Promise<RuntimeResponse>): Promise<void> {
    try {
      const tab = await activeTab();
      if (tab?.id === undefined || tab.url !== url) return;
      await this.track(tab.id, url, save);
    } catch {
      // Icon work must never affect the save.
    }
  }

  forget(tabId: number): void {
    this.known.delete(tabId);
    this.inflight.delete(tabId);
    const hold = this.holds.get(tabId);
    if (hold?.timer !== undefined) clearTimeout(hold.timer);
    this.holds.delete(tabId);
  }

  // Listener entry points, bound so they can be passed to chrome.*.addListener directly.

  readonly handleTabUpdated = (
    tabId: number,
    changeInfo: chrome.tabs.TabChangeInfo,
    tab: chrome.tabs.Tab,
  ): void => {
    if (changeInfo.url !== undefined || changeInfo.status === "complete") {
      void this.refresh(tabId, tab.url);
    }
  };

  readonly handleTabActivated = ({ tabId }: chrome.tabs.TabActiveInfo): void => {
    void currentTabUrl(tabId).then((url) => this.refresh(tabId, url));
  };

  readonly handleTabRemoved = (tabId: number): void => {
    this.forget(tabId);
  };

  private async isSaved(url: string): Promise<boolean | undefined> {
    try {
      const response = await this.request({ type: "lookup_urls", urls: [url] });
      if (!response.success || response.data.type !== "lookup_urls_result") return undefined;
      return response.data.saved.includes(url);
    } catch {
      return undefined;
    }
  }

  private async lookUp(tabId: number, url: string): Promise<void> {
    const epoch = this.saveEpoch;
    const saved = await this.isSaved(url);
    // The tab may have moved on, or a save may have started, while we waited.
    if ((await currentTabUrl(tabId)) !== url) return;
    if (this.holds.get(tabId)?.url === url) return;
    if (epoch !== this.saveEpoch) return this.lookUp(tabId, url);
    if (saved === undefined) {
      // Host missing or failing: show nothing rather than a guess, and ask again next time.
      this.known.delete(tabId);
      await setTabIcon(tabId, "idle");
      return;
    }
    this.known.set(tabId, { url, saved });
    await setTabIcon(tabId, saved ? "saved" : "idle");
  }

  /** Give the tab's icon to a save; `then` runs when the result hold ends. */
  private hold(tabId: number, url: string, look: ToolbarLook, then?: () => void): SaveHold {
    const previous = this.holds.get(tabId);
    if (previous?.timer !== undefined) clearTimeout(previous.timer);
    const entry: SaveHold = { url, look };
    if (then) {
      entry.timer = setTimeout(() => {
        if (this.holds.get(tabId) !== entry) return;
        this.holds.delete(tabId);
        then();
      }, SAVE_RESULT_HOLD_MS);
    }
    this.holds.set(tabId, entry);
    void setTabIcon(tabId, look);
    return entry;
  }

  private async track(tabId: number, url: string, save: Promise<RuntimeResponse>): Promise<void> {
    const saving = this.hold(tabId, url, "saving");
    let response: RuntimeResponse;
    try {
      response = await save;
    } catch {
      response = { success: false, error: "save failed" };
    }
    this.forgetUrl(url);
    this.saveEpoch += 1;
    // A newer save or a closed tab now owns this icon.
    if (this.holds.get(tabId) !== saving) return;
    if ((await currentTabUrl(tabId)) !== url || this.holds.get(tabId) !== saving) {
      // Navigated away: Chrome reset the icon and onUpdated paints the new page.
      if (this.holds.get(tabId) === saving) this.holds.delete(tabId);
      return;
    }
    if (response.success) this.known.set(tabId, { url, saved: true });
    this.hold(tabId, url, response.success ? "saved-fresh" : "error", () => {
      void currentTabUrl(tabId).then((current) => {
        if (current === url) return this.refresh(tabId, url);
      });
    });
  }

  private forgetUrl(url: string): void {
    for (const [tabId, entry] of this.known) {
      if (entry.url === url) this.known.delete(tabId);
    }
  }
}
