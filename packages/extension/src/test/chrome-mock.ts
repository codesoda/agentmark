/**
 * Lightweight Chrome API mock for vitest.
 * Provides the minimum surface needed for service-worker and native-messaging tests.
 * Each test should reset/customize via createMockPort() and resetChromeMock().
 */

import { vi } from "vitest";

export interface MockPort {
  name: string;
  onMessage: {
    addListener: ReturnType<typeof vi.fn>;
    removeListener: ReturnType<typeof vi.fn>;
  };
  onDisconnect: {
    addListener: ReturnType<typeof vi.fn>;
    removeListener: ReturnType<typeof vi.fn>;
  };
  postMessage: ReturnType<typeof vi.fn>;
  disconnect: ReturnType<typeof vi.fn>;
}

export function createMockPort(name = "com.agentmark.native"): MockPort {
  return {
    name,
    onMessage: {
      addListener: vi.fn(),
      removeListener: vi.fn(),
    },
    onDisconnect: {
      addListener: vi.fn(),
      removeListener: vi.fn(),
    },
    postMessage: vi.fn(),
    disconnect: vi.fn(),
  };
}

function createEventTarget() {
  return {
    addListener: vi.fn(),
    removeListener: vi.fn(),
    hasListeners: vi.fn().mockReturnValue(false),
  };
}

/**
 * An in-memory `chrome.storage` area. `onChange` is called with Chrome's
 * `(changes, areaName)` shape on every set/remove, so tests can wire it to
 * `storage.onChanged` listeners.
 */
export function createStorageArea(areaName: string, onChange?: (changes: Record<string, chrome.storage.StorageChange>, area: string) => void) {
  const data: Record<string, unknown> = {};
  return {
    data,
    get: vi.fn(async (keys?: string | string[] | null) => {
      if (keys === undefined || keys === null) return { ...data };
      const list = Array.isArray(keys) ? keys : [keys];
      const out: Record<string, unknown> = {};
      for (const key of list) if (key in data) out[key] = data[key];
      return out;
    }),
    set: vi.fn(async (items: Record<string, unknown>) => {
      const changes: Record<string, chrome.storage.StorageChange> = {};
      for (const [key, value] of Object.entries(items)) {
        changes[key] = { oldValue: data[key], newValue: value };
        data[key] = value;
      }
      onChange?.(changes, areaName);
    }),
    remove: vi.fn(async (keys: string | string[]) => {
      const changes: Record<string, chrome.storage.StorageChange> = {};
      for (const key of Array.isArray(keys) ? keys : [keys]) {
        if (key in data) changes[key] = { oldValue: data[key] };
        delete data[key];
      }
      onChange?.(changes, areaName);
    }),
  };
}

export function resetChromeMock() {
  const storageOnChanged = createEventTarget();
  /** Delivers storage changes to every listener registered on `storage.onChanged`. */
  const emitStorageChange = (changes: Record<string, chrome.storage.StorageChange>, area: string) => {
    for (const [listener] of storageOnChanged.addListener.mock.calls) {
      (listener as (c: typeof changes, a: string) => void)(changes, area);
    }
  };
  const mock = {
    runtime: {
      id: "test-extension-id" as string | undefined,
      connectNative: vi.fn(),
      sendNativeMessage: vi.fn(),
      getManifest: vi.fn(() => ({ version: "0.3.0" })),
      getPlatformInfo: vi.fn(async () => ({ os: "mac", arch: "arm64", nacl_arch: "arm" })),
      sendMessage: vi.fn(),
      onMessage: createEventTarget(),
      onInstalled: createEventTarget(),
      onStartup: createEventTarget(),
      lastError: null as chrome.runtime.LastError | null,
    },
    contextMenus: {
      create: vi.fn(),
      removeAll: vi.fn((_cb?: () => void) => {
        _cb?.();
      }),
      onClicked: createEventTarget(),
    },
    tabs: {
      query: vi.fn(),
      get: vi.fn<(tabId: number) => Promise<Partial<chrome.tabs.Tab>>>(() => Promise.resolve({})),
      sendMessage: vi.fn(() => Promise.resolve()),
      onUpdated: createEventTarget(),
      onActivated: createEventTarget(),
      onRemoved: createEventTarget(),
    },
    action: {
      openPopup: vi.fn(() => Promise.resolve()),
      setIcon: vi.fn<(details: chrome.action.TabIconDetails) => Promise<void>>(() => Promise.resolve()),
    },
    notifications: {
      create: vi.fn(
        (
          _id: string,
          _opts: unknown,
          cb?: (notificationId: string) => void,
        ) => {
          cb?.(_id);
        },
      ),
    },
    storage: {
      local: {
        get: vi.fn(() => Promise.resolve({})),
        set: vi.fn(() => Promise.resolve()),
      },
      session: createStorageArea("session", emitStorageChange),
      onChanged: storageOnChanged,
    },
    permissions: {
      contains: vi.fn(async () => false),
      request: vi.fn(async () => false),
      remove: vi.fn(async () => true),
      onAdded: createEventTarget(),
      onRemoved: createEventTarget(),
    },
    scripting: {
      executeScript: vi.fn(() => Promise.resolve([{ result: "" }])),
    },
    sidePanel: {
      open: vi.fn(() => Promise.resolve()),
    },
    commands: {
      onCommand: createEventTarget(),
    },
  };

  (globalThis as Record<string, unknown>).chrome = mock;
  return mock;
}

// Initialize on setup
resetChromeMock();
