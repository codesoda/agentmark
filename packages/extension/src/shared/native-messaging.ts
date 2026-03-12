/**
 * Native messaging client for Chrome extension ↔ AgentMark CLI communication.
 * Owns the single Port lifecycle, FIFO response matching, reconnect policy,
 * and connection state tracking. No other module should call connectNative().
 */

import { NATIVE_HOST_NAME } from "./constants";
import {
  type NativeRequest,
  type NativeResponse,
  type ConnectionStatus,
  parseNativeResponse,
} from "./types";

const MAX_UNSENT_QUEUE = 10;
const DEFAULT_TIMEOUT_MS = 30_000;
const STATUS_TIMEOUT_MS = 5_000;
const MAX_SELECTED_TEXT_LENGTH = 500_000; // Well under 1 MiB frame limit

interface PendingRequest {
  resolve: (response: NativeResponse) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

export class NativeMessagingClient {
  private port: chrome.runtime.Port | null = null;
  private status: ConnectionStatus = "disconnected";
  private pending: PendingRequest[] = [];
  private unsentQueue: NativeRequest[] = [];
  private reconnecting: Promise<void> | null = null;

  getStatus(): ConnectionStatus {
    return this.status;
  }

  /**
   * Send a request to the native host and await the response.
   * Connects lazily if disconnected. Rejects if the connection fails.
   */
  async sendRequest(request: NativeRequest): Promise<NativeResponse> {
    const prepared = this.prepareRequest(request);

    await this.ensureConnected();

    if (!this.port) {
      throw new Error("Native host not connected");
    }

    const timeoutMs = prepared.type === "status" ? STATUS_TIMEOUT_MS : DEFAULT_TIMEOUT_MS;

    return new Promise<NativeResponse>((resolve, reject) => {
      const timer = setTimeout(() => {
        const idx = this.pending.findIndex((p) => p.resolve === resolve);
        if (idx !== -1) {
          this.pending.splice(idx, 1);
        }
        reject(new Error("Native host request timed out"));
      }, timeoutMs);

      this.pending.push({ resolve, reject, timer });
      this.port!.postMessage(prepared);
    });
  }

  /**
   * Disconnect from the native host and clean up all state.
   */
  disconnect(): void {
    if (this.port) {
      try {
        this.port.disconnect();
      } catch {
        // Port may already be disconnected
      }
    }
    this.cleanupPort("Client disconnected");
  }

  private prepareRequest(request: NativeRequest): NativeRequest {
    if (request.type === "save" && request.selected_text) {
      if (request.selected_text.length > MAX_SELECTED_TEXT_LENGTH) {
        return {
          ...request,
          selected_text: request.selected_text.slice(0, MAX_SELECTED_TEXT_LENGTH),
        };
      }
    }
    return request;
  }

  private async ensureConnected(): Promise<void> {
    if (this.status === "connected" && this.port) {
      return;
    }

    if (this.reconnecting) {
      return this.reconnecting;
    }

    this.reconnecting = this.connect();
    try {
      await this.reconnecting;
    } finally {
      this.reconnecting = null;
    }
  }

  private connect(): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      this.status = "connecting";

      try {
        const port = chrome.runtime.connectNative(NATIVE_HOST_NAME);

        // Check for immediate connection error
        const lastError = chrome.runtime.lastError;
        if (lastError) {
          this.status = "error";
          reject(new Error(lastError.message ?? "Failed to connect to native host"));
          return;
        }

        port.onMessage.addListener((msg: unknown) => {
          this.handleMessage(msg);
        });

        port.onDisconnect.addListener(() => {
          const error = chrome.runtime.lastError;
          const reason = error?.message ?? "Native host disconnected";
          this.cleanupPort(reason);
        });

        this.port = port;
        this.status = "connected";

        // Flush any unsent queued requests
        this.flushUnsentQueue();

        resolve();
      } catch (err) {
        this.status = "error";
        reject(err instanceof Error ? err : new Error(String(err)));
      }
    });
  }

  private handleMessage(raw: unknown): void {
    const pending = this.pending.shift();
    if (!pending) {
      // Unsolicited message — no pending request to match. Ignore.
      return;
    }

    clearTimeout(pending.timer);

    try {
      const parsed = parseNativeResponse(raw);
      pending.resolve(parsed);
    } catch (err) {
      pending.reject(err instanceof Error ? err : new Error(String(err)));
    }
  }

  private cleanupPort(reason: string): void {
    this.port = null;
    this.status = "disconnected";

    // Reject all in-flight pending requests
    const pendingCopy = this.pending.splice(0, this.pending.length);
    for (const p of pendingCopy) {
      clearTimeout(p.timer);
      p.reject(new Error(reason));
    }
  }

  private flushUnsentQueue(): void {
    if (!this.port || this.unsentQueue.length === 0) return;

    const queue = this.unsentQueue.splice(0, this.unsentQueue.length);
    for (const request of queue) {
      const timeoutMs = request.type === "status" ? STATUS_TIMEOUT_MS : DEFAULT_TIMEOUT_MS;

      // These are fire-and-forget retries of unsent work.
      // We can't return the promise to the original caller since they already got rejected.
      const promise = new Promise<NativeResponse>((resolve, reject) => {
        const timer = setTimeout(() => {
          const idx = this.pending.findIndex((p) => p.resolve === resolve);
          if (idx !== -1) {
            this.pending.splice(idx, 1);
          }
          reject(new Error("Native host request timed out"));
        }, timeoutMs);
        this.pending.push({ resolve, reject, timer });
      });

      this.port.postMessage(request);
      // Suppress unhandled rejection — these are best-effort retries
      promise.catch(() => {});
    }
  }

  /**
   * Queue a request to be sent on next reconnect.
   * Returns false if the queue is full (bounded to prevent memory leaks).
   */
  queueForRetry(request: NativeRequest): boolean {
    if (this.unsentQueue.length >= MAX_UNSENT_QUEUE) {
      return false;
    }
    this.unsentQueue.push(request);
    return true;
  }
}

// Singleton instance for the service worker
let client: NativeMessagingClient | null = null;

export function getNativeClient(): NativeMessagingClient {
  if (!client) {
    client = new NativeMessagingClient();
  }
  return client;
}

// For testing — allows replacing the singleton
export function resetNativeClient(): void {
  if (client) {
    client.disconnect();
  }
  client = null;
}
