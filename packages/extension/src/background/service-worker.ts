/// <reference types="chrome" />

/**
 * AgentMark MV3 background service worker.
 * Thin wiring: listener registration, context menu, and message forwarding.
 * All native-port state lives in shared/native-messaging.ts.
 */

import { getNativeClient } from "../shared/native-messaging";
import type {
  ChooseFolderResultResponse,
  RuntimeMessage,
  RuntimeResponse,
  NativeRequest,
  NativeResponse,
  PendingSave,
  SaveResultNotice,
} from "../shared/types";
import { isErrorResponse } from "../shared/types";
import { EXTENSION_NAME, REPORT_PAGE_MESSAGE, SIDEPANEL_VIEW_KEY } from "../shared/constants";
import { isUnknownMessageError } from "../shared/connection";
import { queryPageHtml } from "../shared/runtime";
import { clearSetupDraft, updateSetupDraft } from "../shared/setup-draft";
import { ToolbarIcon } from "./toolbar-icon";

const CONTEXT_MENU_ID = "agentmark-save";
const SIDEPANEL_MENU_ID = "agentmark-sidepanel";
const SETTINGS_MENU_ID = "agentmark-settings";
/** The folder dialog may stay open up to the host's 300 s limit; allow a little more. */
const CHOOSE_FOLDER_TIMEOUT_MS = 310_000;
/** The host truncates longer values; never send more. */
const MAX_EXTENSION_VERSION_LENGTH = 64;
const SUPPORTED_SCHEMES = new Set(["http:", "https:"]);
/** The native host considers at most this many URLs per lookup. */
const MAX_LOOKUP_URLS = 200;
/** How long the install/startup ping waits for the host. */
const PING_TIMEOUT_MS = 10_000;
/**
 * The only messages content scripts send (`src/content/`). Anything else from
 * a content script (settings, folder dialog, bookmark reads and edits) is
 * refused: a page should never reach those, even through a content-script bug.
 */
const CONTENT_SCRIPT_MESSAGES: ReadonlySet<string> = new Set(["open_save_popup", "lookup_urls", REPORT_PAGE_MESSAGE]);

/**
 * Whether a message came from a content script. Content scripts run in a tab;
 * extension pages (popup, side panel) have no tab, or, when opened in a tab,
 * this extension's own origin.
 */
function fromContentScript(sender: chrome.runtime.MessageSender): boolean {
  if (sender.tab === undefined) return false;
  const own = `chrome-extension://${chrome.runtime.id}`;
  return sender.origin !== own && !(sender.url ?? "").startsWith(`${own}/`);
}

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
    chrome.contextMenus.create({
      id: SETTINGS_MENU_ID,
      title: "Settings",
      contexts: ["action"],
    });
  });
}

/** This extension's version, sent with `status` so the host can record which build connected. */
function extensionVersion(): string | undefined {
  try {
    return chrome.runtime.getManifest().version.slice(0, MAX_EXTENSION_VERSION_LENGTH);
  } catch {
    return undefined;
  }
}

function statusRequest(): NativeRequest {
  const version = extensionVersion();
  return version ? { type: "status", extension_version: version } : { type: "status" };
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
    return toRuntimeResponse(await client.sendRequest(request));
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { success: false, error: message };
  }
}

function toRuntimeResponse(response: NativeResponse): RuntimeResponse {
  if (isErrorResponse(response)) {
    return response.code
      ? { success: false, error: response.message, code: response.code }
      : { success: false, error: response.message };
  }
  return { success: true, data: response };
}

/** What a CLI too old for the folder dialog answers instead (contract §3.2). */
const FOLDER_DIALOG_NEEDS_UPDATE: ChooseFolderResultResponse = {
  type: "choose_folder_result",
  outcome: "unavailable",
  path: null,
  path_display: null,
  reason: "Update AgentMark to use the folder dialog.",
};

/**
 * Show the system folder dialog in its own host process (contract §2.4).
 * A chosen folder also goes into the setup draft first, because the popup
 * has usually closed by the time the user picks one.
 */
async function chooseFolder(startPath: string | undefined): Promise<RuntimeResponse> {
  const request: NativeRequest = startPath
    ? { type: "choose_folder", start_path: startPath }
    : { type: "choose_folder" };
  let response: NativeResponse;
  try {
    response = await getNativeClient().sendOneShot(request, CHOOSE_FOLDER_TIMEOUT_MS);
  } catch (err) {
    return { success: false, error: err instanceof Error ? err.message : String(err) };
  }
  if (isErrorResponse(response) && isUnknownMessageError(response.message)) {
    return { success: true, data: FOLDER_DIALOG_NEEDS_UPDATE };
  }
  if (response.type === "choose_folder_result" && response.outcome === "chosen" && response.path) {
    await updateSetupDraft({ storage_path: response.path_display ?? response.path });
  }
  return toRuntimeResponse(response);
}

/** Save settings; a successful save ends any setup in progress, so its draft goes. */
async function saveSettings(settings: unknown): Promise<RuntimeResponse> {
  if (!settings || typeof settings !== "object" || Array.isArray(settings)) {
    return { success: false, error: "save_settings needs a settings object" };
  }
  const response = await handleNativeRequest({ type: "save_settings", settings });
  if (response.success && response.data.type === "save_settings_result") await clearSetupDraft();
  return response;
}

/**
 * A content script reports the page it runs on (contract §3.3). Accepted
 * only from a tab's top frame, for the URL Chrome says that frame shows.
 */
function handleReportPage(message: { url?: unknown }, sender: chrome.runtime.MessageSender): boolean {
  const tabId = sender.tab?.id;
  const url = message.url;
  if (
    typeof tabId !== "number" ||
    sender.frameId !== 0 ||
    typeof url !== "string" ||
    !isSupportedUrl(url) ||
    url !== sender.url
  ) {
    return false;
  }
  void toolbarIcon.reportPage(tabId, url);
  return true;
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
  if (fromContentScript(sender) && !CONTENT_SCRIPT_MESSAGES.has(String((message as { type?: unknown }).type))) {
    sendResponse({ success: false, error: "Not allowed from a web page" });
    return false;
  }

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
    handleNativeRequest(statusRequest()).then(sendResponse);
    return true;
  }

  if (message.type === "get_setup_state") {
    handleNativeRequest({ type: "get_setup_state" }).then(sendResponse);
    return true;
  }

  if (message.type === "save_settings") {
    saveSettings((message as { settings?: unknown }).settings).then(sendResponse);
    return true;
  }

  if (message.type === "choose_folder") {
    const start = (message as { start_path?: unknown }).start_path;
    chooseFolder(typeof start === "string" && start.trim() ? start : undefined).then(sendResponse);
    return true;
  }

  if (message.type === REPORT_PAGE_MESSAGE) {
    const ok = handleReportPage(message as { url?: unknown }, sender);
    (sendResponse as (response: unknown) => void)({ ok });
    return false;
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

  sendResponse({ success: false, error: `Unknown message type: ${(message as unknown as Record<string, unknown>).type}` });
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

/**
 * Open the side panel on its Settings view. Must stay synchronous up to
 * `sidePanel.open`, which Chrome allows only inside the click's user gesture.
 */
function openSettings(tab?: chrome.tabs.Tab): void {
  try {
    void Promise.resolve(chrome.storage.session.set({ [SIDEPANEL_VIEW_KEY]: "settings" })).catch(() => {});
  } catch {
    // The panel then opens on its default view.
  }
  if (tab?.windowId !== undefined) {
    try {
      void Promise.resolve(chrome.sidePanel.open({ windowId: tab.windowId })).catch(() => {});
    } catch {
      // No window to open in.
    }
    return;
  }
  void openSidePanel();
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
  if (info.menuItemId === SETTINGS_MENU_ID) {
    openSettings(tab);
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

/**
 * One status request at install and browser start, so the host records that
 * this extension connected (its liveness file, with `extension_version`)
 * without the popup being opened. Sent one-shot: Chrome closes the port after
 * the reply, so the host exits and the worker can sleep, instead of both
 * staying alive for the whole browser session. Errors are ignored.
 */
function pingApp(): void {
  try {
    void getNativeClient()
      .sendOneShot(statusRequest(), PING_TIMEOUT_MS)
      .catch(() => {});
  } catch {
    // No native messaging at all: nothing to record.
  }
}

chrome.runtime.onInstalled.addListener(() => {
  ensureContextMenu();
  pingApp();
});

chrome.runtime.onStartup.addListener(() => {
  ensureContextMenu();
  pingApp();
});

chrome.runtime.onMessage.addListener(handleRuntimeMessage);

chrome.contextMenus.onClicked.addListener(handleContextMenuClick);

chrome.commands.onCommand.addListener(handleCommand);

// Chrome resets a tab's icon on navigation, so every page load repaints it.
chrome.tabs.onUpdated.addListener(toolbarIcon.handleTabUpdated);
chrome.tabs.onActivated.addListener(toolbarIcon.handleTabActivated);
chrome.tabs.onRemoved.addListener(toolbarIcon.handleTabRemoved);
// `tabs` is optional: the tab listeners above check it, these keep the check current.
chrome.permissions.onAdded.addListener(toolbarIcon.handlePermissionsAdded);
chrome.permissions.onRemoved.addListener(toolbarIcon.handlePermissionsRemoved);

// Exported for testing
export {
  ensureContextMenu,
  isSupportedUrl,
  handleRuntimeMessage,
  handleContextMenuClick,
  handleNativeRequest,
  handleCommand,
  openSidePanel,
  openSettings,
  pingApp,
  showNotification,
  toolbarIcon,
  CONTEXT_MENU_ID,
  SIDEPANEL_MENU_ID,
  SETTINGS_MENU_ID,
  CHOOSE_FOLDER_TIMEOUT_MS,
};
