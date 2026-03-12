import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { createMockPort, resetChromeMock, type MockPort } from "../test/chrome-mock";
import { NativeMessagingClient } from "./native-messaging";

// Helper to flush microtasks so sendRequest proceeds past `await ensureConnected()`
const tick = () => new Promise<void>((r) => setTimeout(r, 0));

describe("NativeMessagingClient", () => {
  let client: NativeMessagingClient;
  let mockPort: MockPort;
  let chromeMock: ReturnType<typeof resetChromeMock>;

  beforeEach(() => {
    chromeMock = resetChromeMock();
    mockPort = createMockPort();
    chromeMock.runtime.connectNative.mockReturnValue(mockPort);
    client = new NativeMessagingClient();
  });

  afterEach(() => {
    // Suppress rejections from cleanup of tests with pending requests
    try { client.disconnect(); } catch { /* expected */ }
    vi.restoreAllMocks();
  });

  describe("connection lifecycle", () => {
    it("starts disconnected", () => {
      expect(client.getStatus()).toBe("disconnected");
    });

    it("connects lazily on first request", async () => {
      mockPort.onMessage.addListener.mockImplementation((cb: (msg: unknown) => void) => {
        mockPort.postMessage.mockImplementation(() => {
          cb({ type: "status_result", ok: true, version: "1.0.0" });
        });
      });

      const result = await client.sendRequest({ type: "status" });
      expect(chromeMock.runtime.connectNative).toHaveBeenCalledWith("com.agentmark.native");
      expect(chromeMock.runtime.connectNative).toHaveBeenCalledTimes(1);
      expect(result).toEqual({ type: "status_result", ok: true, version: "1.0.0" });
      expect(client.getStatus()).toBe("connected");
    });

    it("reuses existing connection", async () => {
      mockPort.onMessage.addListener.mockImplementation((cb: (msg: unknown) => void) => {
        mockPort.postMessage.mockImplementation(() => {
          cb({ type: "status_result", ok: true, version: "1.0.0" });
        });
      });

      await client.sendRequest({ type: "status" });
      await client.sendRequest({ type: "status" });
      expect(chromeMock.runtime.connectNative).toHaveBeenCalledTimes(1);
    });

    it("reconnects after disconnect", async () => {
      let onDisconnectCb: () => void;

      mockPort.onMessage.addListener.mockImplementation((cb: (msg: unknown) => void) => {
        mockPort.postMessage.mockImplementation(() => {
          cb({ type: "status_result", ok: true, version: "1.0.0" });
        });
      });
      mockPort.onDisconnect.addListener.mockImplementation((cb: () => void) => {
        onDisconnectCb = cb;
      });

      await client.sendRequest({ type: "status" });
      expect(client.getStatus()).toBe("connected");

      onDisconnectCb!();
      expect(client.getStatus()).toBe("disconnected");

      const newPort = createMockPort();
      newPort.onMessage.addListener.mockImplementation((cb: (msg: unknown) => void) => {
        newPort.postMessage.mockImplementation(() => {
          cb({ type: "status_result", ok: true, version: "1.0.0" });
        });
      });
      newPort.onDisconnect.addListener.mockImplementation(() => {});
      chromeMock.runtime.connectNative.mockReturnValue(newPort);

      await client.sendRequest({ type: "status" });
      expect(chromeMock.runtime.connectNative).toHaveBeenCalledTimes(2);
      expect(client.getStatus()).toBe("connected");
    });

    it("sets status to error when connectNative throws", async () => {
      chromeMock.runtime.connectNative.mockImplementation(() => {
        throw new Error("Native host not found");
      });

      await expect(client.sendRequest({ type: "status" })).rejects.toThrow("Native host not found");
      expect(client.getStatus()).toBe("error");
    });

    it("disconnect cleans up", () => {
      client.disconnect();
      expect(client.getStatus()).toBe("disconnected");
    });
  });

  describe("FIFO response matching", () => {
    it("resolves two back-to-back requests in order", async () => {
      const responses = [
        { type: "save_result", id: "1", path: "/a", status: "created" },
        { type: "save_result", id: "2", path: "/b", status: "created" },
      ];
      let onMessageCb: (msg: unknown) => void;

      mockPort.onMessage.addListener.mockImplementation((cb: (msg: unknown) => void) => {
        onMessageCb = cb;
      });
      mockPort.onDisconnect.addListener.mockImplementation(() => {});

      const p1 = client.sendRequest({ type: "save", url: "https://a.com" });
      await tick(); // Let sendRequest proceed past ensureConnected
      const p2 = client.sendRequest({ type: "save", url: "https://b.com" });
      await tick();

      onMessageCb!(responses[0]);
      onMessageCb!(responses[1]);

      const r1 = await p1;
      const r2 = await p2;

      expect(r1).toEqual(responses[0]);
      expect(r2).toEqual(responses[1]);
    });
  });

  describe("disconnect handling", () => {
    it("rejects pending requests on disconnect", async () => {
      let onDisconnectCb: () => void;

      mockPort.onMessage.addListener.mockImplementation(() => {});
      mockPort.onDisconnect.addListener.mockImplementation((cb: () => void) => {
        onDisconnectCb = cb;
      });

      const p = client.sendRequest({ type: "status" });
      await tick(); // Let sendRequest proceed past ensureConnected and postMessage

      onDisconnectCb!();

      await expect(p).rejects.toThrow("Native host disconnected");
      expect(client.getStatus()).toBe("disconnected");
    });

    it("rejects multiple pending requests on disconnect", async () => {
      let onDisconnectCb: () => void;

      mockPort.onMessage.addListener.mockImplementation(() => {});
      mockPort.onDisconnect.addListener.mockImplementation((cb: () => void) => {
        onDisconnectCb = cb;
      });

      const p1 = client.sendRequest({ type: "save", url: "https://a.com" });
      await tick();
      const p2 = client.sendRequest({ type: "save", url: "https://b.com" });
      await tick();

      onDisconnectCb!();

      await expect(p1).rejects.toThrow();
      await expect(p2).rejects.toThrow();
    });
  });

  describe("timeout handling", () => {
    afterEach(() => {
      vi.useRealTimers();
    });

    it("rejects request after timeout", async () => {
      vi.useFakeTimers();

      mockPort.onMessage.addListener.mockImplementation(() => {});
      mockPort.onDisconnect.addListener.mockImplementation(() => {});

      const p = client.sendRequest({ type: "save", url: "https://example.com" });
      // Suppress unhandled rejection warning — test explicitly checks the rejection below
      p.catch(() => {});

      await vi.advanceTimersByTimeAsync(0);
      await vi.advanceTimersByTimeAsync(30_001);

      await expect(p).rejects.toThrow("timed out");
    });

    it("uses shorter timeout for status requests", async () => {
      vi.useFakeTimers();

      mockPort.onMessage.addListener.mockImplementation(() => {});
      mockPort.onDisconnect.addListener.mockImplementation(() => {});

      const p = client.sendRequest({ type: "status" });
      p.catch(() => {});

      await vi.advanceTimersByTimeAsync(0);
      await vi.advanceTimersByTimeAsync(5_001);

      await expect(p).rejects.toThrow("timed out");
    });
  });

  describe("malformed responses", () => {
    it("rejects pending request for non-object response", async () => {
      let onMessageCb: (msg: unknown) => void;

      mockPort.onMessage.addListener.mockImplementation((cb: (msg: unknown) => void) => {
        onMessageCb = cb;
      });
      mockPort.onDisconnect.addListener.mockImplementation(() => {});

      const p = client.sendRequest({ type: "status" });
      await tick();

      onMessageCb!("not an object");

      await expect(p).rejects.toThrow("not an object");
    });

    it("rejects for unknown response type", async () => {
      let onMessageCb: (msg: unknown) => void;

      mockPort.onMessage.addListener.mockImplementation((cb: (msg: unknown) => void) => {
        onMessageCb = cb;
      });
      mockPort.onDisconnect.addListener.mockImplementation(() => {});

      const p = client.sendRequest({ type: "status" });
      await tick();

      onMessageCb!({ type: "unknown_thing" });

      await expect(p).rejects.toThrow("Unknown native response type");
    });
  });

  describe("selected_text truncation", () => {
    it("truncates oversized selected_text", async () => {
      let postedMessage: unknown;

      mockPort.onMessage.addListener.mockImplementation((cb: (msg: unknown) => void) => {
        mockPort.postMessage.mockImplementation((msg: unknown) => {
          postedMessage = msg;
          cb({ type: "save_result", id: "1", path: "/a", status: "created" });
        });
      });
      mockPort.onDisconnect.addListener.mockImplementation(() => {});

      const longText = "x".repeat(600_000);
      await client.sendRequest({
        type: "save",
        url: "https://example.com",
        selected_text: longText,
      });

      const sent = postedMessage as { selected_text: string };
      expect(sent.selected_text.length).toBe(500_000);
    });
  });

  describe("unsent queue", () => {
    it("accepts requests up to the queue limit", () => {
      for (let i = 0; i < 10; i++) {
        expect(client.queueForRetry({ type: "save", url: `https://${i}.com` })).toBe(true);
      }
      expect(client.queueForRetry({ type: "save", url: "https://overflow.com" })).toBe(false);
    });
  });

  describe("concurrent connect prevention", () => {
    it("does not open multiple connections for concurrent requests", async () => {
      mockPort.onMessage.addListener.mockImplementation((cb: (msg: unknown) => void) => {
        mockPort.postMessage.mockImplementation(() => {
          cb({ type: "status_result", ok: true, version: "1.0.0" });
        });
      });
      mockPort.onDisconnect.addListener.mockImplementation(() => {});

      await Promise.all([
        client.sendRequest({ type: "status" }),
        client.sendRequest({ type: "status" }),
      ]);

      expect(chromeMock.runtime.connectNative).toHaveBeenCalledTimes(1);
    });
  });
});
