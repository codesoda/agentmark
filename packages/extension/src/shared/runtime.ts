/**
 * Promise-based wrappers for Chrome extension APIs used by popup and sidepanel.
 * Keeps callback-style Chrome APIs out of React components.
 */

import type { RuntimeResponse, RuntimeSaveMessage } from "./types";

export interface ActiveTab {
  url: string;
  title?: string;
  favIconUrl?: string;
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
  };
}

export async function sendSaveMessage(
  url: string,
  title?: string,
): Promise<RuntimeResponse> {
  const message: RuntimeSaveMessage = { type: "save", url, title };
  try {
    const response = await chrome.runtime.sendMessage(message);
    return response as RuntimeResponse;
  } catch (err) {
    const errorMessage = err instanceof Error ? err.message : String(err);
    return { success: false, error: errorMessage };
  }
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
