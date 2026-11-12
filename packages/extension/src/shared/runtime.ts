/**
 * Promise-based wrappers for Chrome extension APIs used by popup and sidepanel.
 * Keeps callback-style Chrome APIs out of React components.
 */

import type { PendingSave, BookmarkChanges, BookmarkDetail, BookmarkSummary, BookmarkStateFilter, RuntimeMessage, RuntimeResponse, RuntimeSaveMessage, ShowResultResponse, UpdateResultResponse } from "./types";
import { capturePageHtml, shouldScrollBeforeSnapshot } from "./page-snapshot";
import { classifyConnectionError } from "./connection";

export interface ActiveTab {
  url: string;
  title?: string;
  favIconUrl?: string;
  id?: number;
}

const SUPPORTED_SCHEMES = new Set(["http:", "https:"]);

export function isSupportedUrl(url: string | undefined): url is string {
  if (!url) return false;
  try {
    const parsed = new URL(url);
    return SUPPORTED_SCHEMES.has(parsed.protocol);
  } catch {
    return false;
  }
}

export async function queryActiveTab(): Promise<ActiveTab> {
  const tabs = await chrome.tabs.query({ active: true, currentWindow: true });
  const tab = tabs[0];

  if (!tab?.url) {
    throw new Error("No active tab found");
  }

  if (!isSupportedUrl(tab.url)) {
    throw new Error(`Unsupported page: ${tab.url}`);
  }

  return {
    url: tab.url,
    title: tab.title,
    favIconUrl: tab.favIconUrl,
    id: tab.id,
  };
}

/**
 * Send a typed runtime message to the service worker and parse the response.
 */
async function sendRuntimeMessage(message: RuntimeMessage): Promise<RuntimeResponse> {
  try {
    const response = await chrome.runtime.sendMessage(message);
    return parseRuntimeResponse(response);
  } catch (err) {
    const errorMessage = err instanceof Error ? err.message : String(err);
    return { success: false, error: errorMessage };
  }
}

/**
 * Validate a runtime response shape instead of trusting an unchecked cast.
 */
function parseRuntimeResponse(raw: unknown): RuntimeResponse {
  if (raw === null || raw === undefined || typeof raw !== "object") {
    return { success: false, error: "Invalid response from service worker" };
  }
  const obj = raw as Record<string, unknown>;
  if (obj.success === true && obj.data !== null && obj.data !== undefined && typeof obj.data === "object") {
    return { success: true, data: obj.data } as RuntimeResponse;
  }
  if (obj.success === true) {
    return { success: false, error: "Malformed data in service worker response" };
  }
  if (obj.success === false && typeof obj.error === "string") {
    return { success: false, error: obj.error };
  }
  return { success: false, error: "Malformed response from service worker" };
}

export async function sendSaveMessage(
  url: string,
  title?: string,
  options?: {
    tags?: string[];
    collection?: string;
    note?: string;
    action?: string;
    selected_text?: string;
    page_html?: string;
  },
): Promise<RuntimeResponse> {
  const message: RuntimeSaveMessage = {
    type: "save",
    url,
    title,
    ...options,
  };
  return sendRuntimeMessage(message);
}

/**
 * Take the save an in-page button asked the popup to carry out, if any.
 * Returns undefined for a normal toolbar click.
 */
export async function takePendingSave(): Promise<PendingSave | undefined> {
  try {
    const response: unknown = await chrome.runtime.sendMessage({ type: "take_pending_save" });
    const pending = (response as { pending?: unknown } | undefined)?.pending;
    if (pending && typeof pending === "object" && typeof (pending as PendingSave).url === "string") {
      return pending as PendingSave;
    }
  } catch {
    // Service worker unreachable — behave like a normal toolbar click.
  }
  return undefined;
}

export interface ListBookmarksResult {
  bookmarks: BookmarkSummary[];
  error?: string;
}

export async function sendListBookmarksMessage(options?: {
  limit?: number;
  state?: BookmarkStateFilter;
}): Promise<ListBookmarksResult> {
  const response = await sendRuntimeMessage({
    type: "list",
    limit: options?.limit,
    state: options?.state,
  });
  if (response.success && response.data.type === "list_result") {
    return { bookmarks: response.data.bookmarks };
  }
  const error = response.success
    ? `Unexpected response type: ${response.data.type}`
    : response.error;
  return { bookmarks: [], error };
}

export interface ShowBookmarkResult {
  bookmark?: BookmarkDetail;
  error?: string;
}

export async function sendShowBookmarkMessage(id: string): Promise<ShowBookmarkResult> {
  const response = await sendRuntimeMessage({ type: "show", id });
  if (response.success && response.data.type === "show_result") {
    return { bookmark: (response.data as ShowResultResponse).bookmark };
  }
  const error = response.success
    ? `Unexpected response type: ${response.data.type}`
    : response.error;
  return { error };
}

export interface UpdateBookmarkResult {
  bookmark?: BookmarkDetail;
  error?: string;
}

export async function sendUpdateBookmarkMessage(
  id: string,
  changes: BookmarkChanges,
): Promise<UpdateBookmarkResult> {
  const response = await sendRuntimeMessage({ type: "update", id, changes });
  if (response.success && response.data.type === "update_result") {
    return { bookmark: (response.data as UpdateResultResponse).bookmark };
  }
  const error = response.success
    ? `Unexpected response type: ${response.data.type}`
    : response.error;
  return { error };
}

export async function sendListCollectionsMessage(): Promise<string[]> {
  const response = await sendRuntimeMessage({ type: "list_collections" });
  if (
    response.success &&
    response.data.type === "list_collections_result"
  ) {
    return response.data.collections;
  }
  return [];
}

/**
 * Whether an error means the native host or the service worker is unreachable.
 * See `classifyConnectionError` for which kind it is.
 */
export function isConnectionError(error: string): boolean {
  return classifyConnectionError(error) !== null;
}

// --- Storage helpers for last-used intent defaults ---

const STORAGE_KEY = "agentmark_last_intent";

export interface IntentDefaults {
  tags: string[];
  collection?: string;
}

export async function loadLastUsedIntent(): Promise<IntentDefaults> {
  try {
    const result = await chrome.storage.local.get(STORAGE_KEY);
    const raw = result[STORAGE_KEY];
    if (raw && typeof raw === "object" && !Array.isArray(raw)) {
      const obj = raw as Record<string, unknown>;
      return {
        tags: Array.isArray(obj.tags) ? (obj.tags as string[]).filter((t) => typeof t === "string") : [],
        collection: typeof obj.collection === "string" ? obj.collection : undefined,
      };
    }
  } catch {
    // Storage unavailable — degrade gracefully
  }
  return { tags: [], collection: undefined };
}

export async function saveLastUsedIntent(defaults: IntentDefaults): Promise<void> {
  try {
    await chrome.storage.local.set({ [STORAGE_KEY]: defaults });
  } catch {
    // Best-effort — do not fail the save
  }
}

// --- Selected text capture ---

export async function querySelectedText(tabId: number): Promise<string> {
  try {
    const results = await chrome.scripting.executeScript({
      target: { tabId },
      func: () => window.getSelection()?.toString() ?? "",
    });
    const text = results?.[0]?.result;
    return typeof text === "string" ? text : "";
  } catch {
    // Scripting may be denied on certain pages — degrade to empty
    return "";
  }
}

// --- Page HTML snapshot capture ---

/** Upper bound on snapshot length (UTF-16 code units) to keep native messages bounded. */
export const MAX_PAGE_HTML_LENGTH = 4_000_000;

/**
 * Capture the rendered HTML of a tab. Sites such as LinkedIn refuse non-browser
 * fetches (HTTP 999), so the CLI falls back to this snapshot when its own fetch fails.
 * Pages that mount content lazily (see `shouldScrollBeforeSnapshot`) are scrolled
 * through first, which can take several seconds.
 * Returns undefined when scripting is denied or the page is empty.
 */
export async function queryPageHtml(tabId: number, url?: string): Promise<string | undefined> {
  try {
    const results = await chrome.scripting.executeScript({
      target: { tabId },
      func: capturePageHtml,
      args: [shouldScrollBeforeSnapshot(url)],
    });
    const html = results?.[0]?.result;
    if (typeof html !== "string" || html.trim() === "") return undefined;
    return html.length > MAX_PAGE_HTML_LENGTH ? html.slice(0, MAX_PAGE_HTML_LENGTH) : html;
  } catch {
    // Scripting may be denied on certain pages — degrade to no snapshot
    return undefined;
  }
}

// --- Tag normalization ---

/**
 * Normalize a comma-separated tag string into deduplicated, trimmed tags.
 * Preserves first-seen order. Filters blanks.
 */
export function normalizeTags(input: string): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const raw of input.split(",")) {
    const tag = raw.trim().toLowerCase();
    if (tag && !seen.has(tag)) {
      seen.add(tag);
      result.push(tag);
    }
  }
  return result;
}
