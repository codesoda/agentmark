/// <reference types="chrome" />

/**
 * AgentMark MV3 background service worker.
 * Thin wiring: listener registration, context menu, and message forwarding.
 * All native-port state lives in shared/native-messaging.ts.
 */

import { getNativeClient } from "../shared/native-messaging";
import type {
  RuntimeMessage,
  RuntimeResponse,
  NativeRequest,
  PendingSave,
  SaveResultNotice,
} from "../shared/types";
import { isErrorResponse } from "../shared/types";
import { EXTENSION_NAME } from "../shared/constants";
import { queryPageHtml } from "../shared/runtime";
import { ToolbarIcon } from "./toolbar-icon";

const CONTEXT_MENU_ID = "agentmark-save";
const SIDEPANEL_MENU_ID = "agentmark-sidepanel";
const SUPPORTED_SCHEMES = new Set(["http:", "https:"]);
/** The native host considers at most this many URLs per lookup. */
const MAX_LOOKUP_URLS = 200;

// --- Context menu ---

function ensureContextMenu(): void {
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id: CONTEXT_MENU_ID,
      title: `Save to ${EXTENSION_NAME}`,
      contexts: ["page", "selection"],
    });
    chrome.contextMenus.create({
      id: SIDEPANEL_MENU_ID,
      title: `Open ${EXTENSION_NAME} Side Panel`,
      contexts: ["action"],
    });
  });
}

// --- URL validation ---

function isSupportedUrl(url: string | undefined): url is string {
  if (!url) return false;
  try {
    const parsed = new URL(url);
    return SUPPORTED_SCHEMES.has(parsed.protocol);
  } catch {
    return false;
  }
}

// --- Save dispatch (shared by runtime messages and context menu) ---

async function handleNativeRequest(request: NativeRequest): Promise<RuntimeResponse> {
  try {
    const client = getNativeClient();
    const response = await client.sendRequest(request);
    if (isErrorResponse(response)) {
      return { success: false, error: response.message };
    }
    return { success: true, data: response };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { success: false, error: message };
  }
}

/** Per-tab toolbar icon state; asks the index through the same request path as everything else. */
const toolbarIcon = new ToolbarIcon(handleNativeRequest);

// --- Saves started by an in-page button, carried out by the popup ---

/** How long the popup has to collect a pending save before it is dropped. */
const PENDING_SAVE_TTL_MS = 5_000;

/** A save waiting for the popup to open and take it. */
let pendingSave: { save: PendingSave; tabId?: number; at: number } | null = null;
/** The tab to tell when the popup's save of this URL finishes. */
let resultListener: { url: string; tabId: number } | null = null;

async function openActionPopup(): Promise<boolean> {
  try {
    if (typeof chrome.action?.openPopup !== "function") return false;
    await chrome.action.openPopup();
    return true;
  } catch {
    // Not supported, or the window is not focused.
    return false;
  }
}

function takePendingSave(): PendingSave | null {
  const pending = pendingSave;
  pendingSave = null;
  if (!pending || Date.now() - pending.at > PENDING_SAVE_TTL_MS) return null;
  if (pending.tabId !== undefined) {
    resultListener = { url: pending.save.url, tabId: pending.tabId };
  }
  return pending.save;
}

function notifySaveResult(url: string, response: RuntimeResponse): void {
  if (!resultListener || resultListener.url !== url) return;
  const notice: SaveResultNotice = {
    type: "agentmark:save-result",
    url,
    success: response.success,
    error: response.success ? undefined : response.error,
  };
  try {
    void Promise.resolve(chrome.tabs.sendMessage(resultListener.tabId, notice)).catch(() => {});
  } catch {
    // Tab closed or navigated — nothing to update.
  }
}

// --- Runtime message handler ---

function handleRuntimeMessage(
  message: RuntimeMessage,
  sender: chrome.runtime.MessageSender,
  sendResponse: (response: RuntimeResponse) => void,
): boolean {
  if (message.type === "open_save_popup") {
    if (!isSupportedUrl(message.url)) {
      sendResponse({ success: false, error: "Unsupported or missing URL" });
      return false;
    }
    const save: PendingSave = {
      url: message.url,
      title: message.title,
      page_html: message.page_html,
    };
    pendingSave = { save, tabId: sender.tab?.id, at: Date.now() };
    openActionPopup().then((opened) => {
      if (opened) {
        (sendResponse as (response: unknown) => void)({ success: true, popup_opened: true });
        return;
      }
      // No popup available: save directly so the click is never lost.
      pendingSave = null;
      const saving = handleNativeRequest({ type: "save", ...save });
      if (sender.tab?.id !== undefined) void toolbarIcon.trackTabSave(sender.tab.id, save.url, saving);
      saving.then(sendResponse);
    });
    return true;
  }

  if (message.type === "take_pending_save") {
    (sendResponse as (response: unknown) => void)({ pending: takePendingSave() });
    return false;
  }

  if (message.type === "save") {
    if (!isSupportedUrl(message.url)) {
      sendResponse({ success: false, error: "Unsupported or missing URL" });
      return false;
    }

    const nativeRequest: NativeRequest = {
      type: "save",
      url: message.url,
      title: message.title,
      tags: message.tags,
      collection: message.collection,
      note: message.note,
      selected_text: message.selected_text,
      action: message.action,
      page_html: message.page_html,
    };

    const savedUrl = message.url;
    const saving = handleNativeRequest(nativeRequest);
    // The popup saves the active tab's page, or a post on it; only the former marks the tab.
    void toolbarIcon.trackActiveTabSave(savedUrl, saving);
    saving.then((response) => {
      notifySaveResult(savedUrl, response);
      sendResponse(response);
    });
    return true; // Keep the message channel open for async response
  }

  if (message.type === "status") {
    handleNativeRequest({ type: "status" }).then(sendResponse);
    return true;
  }

  if (message.type === "list_collections") {
    handleNativeRequest({ type: "list_collections" }).then(sendResponse);
    return true;
  }

  if (message.type === "list") {
    handleNativeRequest({ type: "list", limit: message.limit, state: message.state }).then(sendResponse);
    return true;
  }

  if (message.type === "show") {
    handleNativeRequest({ type: "show", id: message.id }).then(sendResponse);
    return true;
  }

  if (message.type === "update") {
    handleNativeRequest({ type: "update", id: message.id, changes: message.changes }).then(sendResponse);
    return true;
  }

  if (message.type === "lookup_urls") {
    const urls = (Array.isArray(message.urls) ? (message.urls as unknown[]) : [])
      .filter((url): url is string => typeof url === "string" && isSupportedUrl(url))
      .slice(0, MAX_LOOKUP_URLS);
    if (urls.length === 0) {
      // Nothing worth asking about: do not wake the native host.
      sendResponse({ success: true, data: { type: "lookup_urls_result", saved: [] } });
      return false;
    }
    handleNativeRequest({ type: "lookup_urls", urls }).then(sendResponse);
    return true;
  }

  sendResponse({ success: false, error: `Unknown message type: ${(message as Record<string, unknown>).type}` });
  return false;
}

// --- Notifications ---

function showNotification(
  id: string,
  title: string,
  message: string,
): void {
  try {
    chrome.notifications.create(id, {
      type: "basic",
      title,
      message,
      iconUrl: "icons/icon-128.png",
    });
  } catch {
    // Notification API may not be available; fail silently
  }
}

// --- Side panel helpers ---

async function openSidePanel(): Promise<void> {
  try {
    const tabs = await chrome.tabs.query({ active: true, currentWindow: true });
    const windowId = tabs[0]?.windowId;
    if (windowId !== undefined) {
      await chrome.sidePanel.open({ windowId });
    }
  } catch {
    // sidePanel.open may fail if no window context — fail silently
  }
}

function handleCommand(command: string): void {
  if (command === "open_side_panel") {
    openSidePanel();
  }
}

// --- Context menu click handler ---

function handleContextMenuClick(
  info: chrome.contextMenus.OnClickData,
  tab?: chrome.tabs.Tab,
): void {
  if (info.menuItemId === SIDEPANEL_MENU_ID) {
    openSidePanel();
    return;
  }
  if (info.menuItemId !== CONTEXT_MENU_ID) return;

  const url = info.pageUrl ?? tab?.url;
  if (!isSupportedUrl(url)) {
    console.warn("[AgentMark] Context menu: unsupported or missing URL");
    return;
  }

  // Snapshot the tab so pages the CLI cannot fetch (e.g. LinkedIn) still save with content.
  const snapshot = tab?.id !== undefined ? queryPageHtml(tab.id, url) : Promise.resolve(undefined);

  const saving = snapshot.then((page_html) =>
    handleNativeRequest({
      type: "save",
      url,
      title: tab?.title,
      selected_text: info.selectionText,
      page_html,
    }),
  );
  if (tab?.id !== undefined) void toolbarIcon.trackTabSave(tab.id, url, saving);

  saving
    .then((response) => {
      if (response.success && response.data.type === "save_result") {
        showNotification(
          `agentmark-save-${Date.now()}`,
          "Saved to AgentMark",
          `${tab?.title ?? url} (${response.data.status})`,
        );
      } else if (!response.success) {
        showNotification(
          `agentmark-error-${Date.now()}`,
          "AgentMark Save Failed",
          response.error,
        );
      }
    });
}

// --- Listener registration (must be at module scope for MV3) ---

chrome.runtime.onInstalled.addListener(() => {
  ensureContextMenu();
});

chrome.runtime.onStartup.addListener(() => {
  ensureContextMenu();
});

chrome.runtime.onMessage.addListener(handleRuntimeMessage);

chrome.contextMenus.onClicked.addListener(handleContextMenuClick);

chrome.commands.onCommand.addListener(handleCommand);

// Chrome resets a tab's icon on navigation, so every page load repaints it.
chrome.tabs.onUpdated.addListener(toolbarIcon.handleTabUpdated);
chrome.tabs.onActivated.addListener(toolbarIcon.handleTabActivated);
chrome.tabs.onRemoved.addListener(toolbarIcon.handleTabRemoved);

// Exported for testing
export {
  ensureContextMenu,
  isSupportedUrl,
  handleRuntimeMessage,
  handleContextMenuClick,
  handleNativeRequest,
  handleCommand,
  openSidePanel,
  showNotification,
  toolbarIcon,
  CONTEXT_MENU_ID,
  SIDEPANEL_MENU_ID,
};
