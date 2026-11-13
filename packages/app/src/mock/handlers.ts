/**
 * A fake command layer for tests and the browser preview. Each command name
 * maps to a function of its arguments; unknown commands throw like Tauri does.
 */

import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

export type Handler = (args: Record<string, unknown>) => unknown | Promise<unknown>;
export type Handlers = Record<string, Handler>;

export interface Calls {
  [command: string]: Record<string, unknown>[];
}

/** Install `handlers` as the IPC layer; returns the calls made, by command. */
export function installHandlers(handlers: Handlers): Calls {
  const calls: Calls = {};
  mockWindows("main");
  mockIPC(async (cmd, args) => {
    const payload = (args ?? {}) as Record<string, unknown>;
    (calls[cmd] ??= []).push(payload);
    const handler = handlers[cmd];
    if (!handler) throw `unknown command ${cmd}`;
    return handler(payload);
  });
  return calls;
}
