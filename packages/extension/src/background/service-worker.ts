/// <reference types="chrome" />

/**
 * AgentMark MV3 background service worker.
 * Thin wiring: listener registration, context menu, and message forwarding.
 * All native-port state lives in shared/native-messaging.ts.
 */

import { getNativeClient } from "../shared/native-messaging";
import type { RuntimeMessage, RuntimeResponse, NativeRequest } from "../shared/types";
import { isErrorResponse } from "../shared/types";
import { EXTENSION_NAME } from "../shared/constants";

const CONTEXT_MENU_ID = "agentmark-save";
const SUPPORTED_SCHEMES = new Set(["http:", "https:"]);

// --- Context menu ---

function ensureContextMenu(): void {
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id: CONTEXT_MENU_ID,
      title: `Save to ${EXTENSION_NAME}`,
      contexts: ["page", "selection"],
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

// --- Runtime message handler ---

function handleRuntimeMessage(
  message: RuntimeMessage,
  _sender: chrome.runtime.MessageSender,
  sendResponse: (response: RuntimeResponse) => void,
): boolean {
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
    };

    handleNativeRequest(nativeRequest).then(sendResponse);
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
      iconUrl: "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='48' height='48'%3E%3Crect width='48' height='48' rx='8' fill='%234F46E5'/%3E%3Ctext x='50%25' y='55%25' text-anchor='middle' dominant-baseline='middle' fill='white' font-size='24' font-family='sans-serif'%3EA%3C/text%3E%3C/svg%3E",
    });
  } catch {
    // Notification API may not be available; fail silently
  }
}

// --- Context menu click handler ---

function handleContextMenuClick(
  info: chrome.contextMenus.OnClickData,
  tab?: chrome.tabs.Tab,
): void {
  if (info.menuItemId !== CONTEXT_MENU_ID) return;

  const url = info.pageUrl ?? tab?.url;
  if (!isSupportedUrl(url)) {
    console.warn("[AgentMark] Context menu: unsupported or missing URL");
    return;
  }

  const nativeRequest: NativeRequest = {
    type: "save",
    url,
    title: tab?.title,
    selected_text: info.selectionText,
  };

  handleNativeRequest(nativeRequest).then((response) => {
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

// Exported for testing
export {
  ensureContextMenu,
  isSupportedUrl,
  handleRuntimeMessage,
  handleContextMenuClick,
  handleNativeRequest,
  showNotification,
  CONTEXT_MENU_ID,
};
