/// <reference types="chrome" />

/**
 * Per-tab toolbar icon: shows whether the tab's page is bookmarked and how a
 * save of it is going. Blue only ever means "saved".
 *
 * All state here is an in-memory optimisation. The MV3 worker can be killed at
 * any time and Chrome resets a tab's icon when the tab navigates, so every
 * navigation repaints from the index and a cold start behaves correctly.
 *
 * `tabs` is an optional permission. Three modes:
 * - Without it, tab events carry no URLs, so they are ignored (Chrome resets
 *   the icon on navigation by itself). The icon still follows saves made from
 *   the popup (the toolbar click grants `activeTab`) and pages that report
 *   themselves through `reportPage` (the LinkedIn, X, Hacker News and Reddit content
 *   script).
 * - With it, every navigation and tab switch is looked up, as well.
 * - Granted or revoked while running: `permissions.onAdded/onRemoved` flip the
 *   mode. The tab listeners stay registered either way (MV3 only wakes the
 *   worker for listeners added at top level); they check the flag instead.
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

const TABS_PERMISSION: chrome.permissions.Permissions = { permissions: ["tabs"] };

function mentionsTabs(permissions: chrome.permissions.Permissions | undefined): boolean {
  return permissions?.permissions?.includes("tabs") ?? false;
}

/** Tracks and paints the toolbar icon of every tab. One instance per worker lifetime. */
export class ToolbarIcon {
  /** Whether the optional `tabs` permission is held; undefined until first asked. */
  private tabsAllowed: boolean | undefined;
  private tabsAllowedQuery: Promise<boolean> | undefined;
  /** The page each tab's content script last reported, for answers that arrive late. */
  private readonly reported = new Map<number, string>();
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

  /** Whether tab events may be used: the optional `tabs` permission is held. Never throws. */
  hasTabsPermission(): Promise<boolean> {
    if (this.tabsAllowed !== undefined) return Promise.resolve(this.tabsAllowed);
    if (!this.tabsAllowedQuery) {
      this.tabsAllowedQuery = (async () => {
        let allowed = false;
        try {
          allowed = (await chrome.permissions.contains(TABS_PERMISSION)) === true;
        } catch {
          allowed = false;
        }
        // A grant or revoke that arrived while we asked wins.
        if (this.tabsAllowed === undefined) this.tabsAllowed = allowed;
        this.tabsAllowedQuery = undefined;
        return this.tabsAllowed;
      })();
    }
    return this.tabsAllowedQuery;
  }

  /**
   * A content script says tab `tabId` shows `url` (already validated by the
   * caller). Paints saved or idle from the index without reading the tab, so
   * it works without the `tabs` permission. Never throws.
   */
  async reportPage(tabId: number, url: string): Promise<void> {
    try {
      this.reported.set(tabId, url);
      const hold = this.holds.get(tabId);
      if (hold && hold.url === url) {
        await setTabIcon(tabId, hold.look);
        return;
      }
      const cached = this.known.get(tabId);
      if (cached?.url === url) {
        await setTabIcon(tabId, cached.saved ? "saved" : "idle");
        return;
      }
      const epoch = this.saveEpoch;
      const saved = await this.isSaved(url);
      // The page moved on, or a save took the icon, while we waited.
      if (this.reported.get(tabId) !== url) return;
      if (this.holds.get(tabId)?.url === url) return;
      if (epoch !== this.saveEpoch) return this.reportPage(tabId, url);
      if (saved === undefined) {
        this.known.delete(tabId);
        await setTabIcon(tabId, "idle");
        return;
      }
      this.known.set(tabId, { url, saved });
      await setTabIcon(tabId, saved ? "saved" : "idle");
    } catch {
      // Icon work must never surface.
    }
  }

  forget(tabId: number): void {
    this.reported.delete(tabId);
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
      // A new page in this tab: its old report no longer applies.
      if (changeInfo.url !== undefined && this.reported.get(tabId) !== changeInfo.url) {
        this.reported.delete(tabId);
      }
      void this.hasTabsPermission().then((allowed) => {
        if (allowed) return this.refresh(tabId, tab.url);
      });
    }
  };

  readonly handleTabActivated = ({ tabId }: chrome.tabs.TabActiveInfo): void => {
    void this.hasTabsPermission().then(async (allowed) => {
      if (allowed) await this.refresh(tabId, await currentTabUrl(tabId));
    });
  };

  /** `chrome.permissions.onAdded`: tab events become usable; repaint the active tab. */
  readonly handlePermissionsAdded = (permissions: chrome.permissions.Permissions): void => {
    if (!mentionsTabs(permissions)) return;
    this.tabsAllowed = true;
    void activeTab().then((tab) => {
      if (tab?.id !== undefined) return this.refresh(tab.id, tab.url);
    });
  };

  /** `chrome.permissions.onRemoved`: stop using tab events and drop what they taught us. */
  readonly handlePermissionsRemoved = (permissions: chrome.permissions.Permissions): void => {
    if (!mentionsTabs(permissions)) return;
    this.tabsAllowed = false;
    this.known.clear();
    this.inflight.clear();
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
