/**
 * Promise-based wrappers for Chrome extension APIs used by popup and sidepanel.
 * Keeps callback-style Chrome APIs out of React components.
 */

import type { RuntimeMessage, RuntimeResponse, RuntimeSaveMessage } from "./types";

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
  if (obj.success === true && obj.data !== undefined) {
    return { success: true, data: obj.data } as RuntimeResponse;
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
 * Classify whether a save error indicates the native host is not installed/connected.
 * Used by popup to show install guidance instead of a generic error.
 */
export function isConnectionError(error: string): boolean {
  const patterns = [
    "native messaging host not found",
    "Specified native messaging host not found",
    "Native host has exited",
    "disconnected",
    "not connected",
  ];
  const lower = error.toLowerCase();
  return patterns.some((p) => lower.includes(p.toLowerCase()));
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
