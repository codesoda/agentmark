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

export function resetChromeMock() {
  const mock = {
    runtime: {
      connectNative: vi.fn(),
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
  };

  (globalThis as Record<string, unknown>).chrome = mock;
  return mock;
}

// Initialize on setup
resetChromeMock();
