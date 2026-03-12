/// <reference types="chrome" />

/**
 * AgentMark MV3 background service worker.
 * Thin wiring: listener registration, context menu, and message forwarding.
 * All native-port state lives in shared/native-messaging.ts.
 */

import { getNativeClient } from "../shared/native-messaging";
import type { RuntimeMessage, RuntimeResponse, NativeRequest } from "../shared/types";
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

async function handleSaveRequest(request: NativeRequest): Promise<RuntimeResponse> {
  try {
    const client = getNativeClient();
    const response = await client.sendRequest(request);
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
      note: message.note,
      selected_text: message.selected_text,
      action: message.action,
    };

    handleSaveRequest(nativeRequest).then(sendResponse);
    return true; // Keep the message channel open for async response
  }

  if (message.type === "status") {
    handleSaveRequest({ type: "status" }).then(sendResponse);
    return true;
  }

  sendResponse({ success: false, error: `Unknown message type: ${(message as Record<string, unknown>).type}` });
  return false;
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

  handleSaveRequest(nativeRequest).then((response) => {
    if (!response.success) {
      console.error("[AgentMark] Context menu save failed:", response.error);
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
  handleSaveRequest,
  CONTEXT_MENU_ID,
};
