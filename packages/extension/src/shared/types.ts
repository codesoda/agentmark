/**
 * Wire-contract types mirroring the Rust native-host message schema.
 * Uses snake_case field names to match the serde(rename_all = "snake_case") convention.
 *
 * Outgoing = Extension → Native Host
 * Incoming = Native Host → Extension
 */

// --- Outgoing requests (Extension → Native Host) ---

export interface SaveRequest {
  type: "save";
  url: string;
  title?: string;
  tags?: string[];
  collection?: string;
  note?: string;
  selected_text?: string;
  action?: string;
}

export interface StatusRequest {
  type: "status";
}

export interface ListCollectionsRequest {
  type: "list_collections";
}

export type NativeRequest = SaveRequest | StatusRequest | ListCollectionsRequest;

// --- Incoming responses (Native Host → Extension) ---

export interface SaveResultResponse {
  type: "save_result";
  id: string;
  path: string;
  status: string;
}

export interface StatusResultResponse {
  type: "status_result";
  ok: boolean;
  version: string;
}

export interface ListCollectionsResultResponse {
  type: "list_collections_result";
  collections: string[];
}

export interface ErrorResponse {
  type: "error";
  message: string;
}

export type NativeResponse = SaveResultResponse | StatusResultResponse | ListCollectionsResultResponse | ErrorResponse;

// --- Connection status ---

export type ConnectionStatus = "disconnected" | "connecting" | "connected" | "error";

// --- Runtime parsing helpers ---

const RESPONSE_TYPES = new Set(["save_result", "status_result", "list_collections_result", "error"]);

export function parseNativeResponse(raw: unknown): NativeResponse {
  if (raw === null || raw === undefined || typeof raw !== "object") {
    throw new Error("Native response is not an object");
  }

  const obj = raw as Record<string, unknown>;

  if (typeof obj.type !== "string") {
    throw new Error("Native response missing 'type' field");
  }

  if (!RESPONSE_TYPES.has(obj.type)) {
    throw new Error(`Unknown native response type: ${obj.type}`);
  }

  switch (obj.type) {
    case "save_result": {
      if (typeof obj.id !== "string" || typeof obj.path !== "string" || typeof obj.status !== "string") {
        throw new Error("save_result missing required fields (id, path, status)");
      }
      return { type: "save_result", id: obj.id, path: obj.path, status: obj.status };
    }
    case "status_result": {
      if (typeof obj.ok !== "boolean" || typeof obj.version !== "string") {
        throw new Error("status_result missing required fields (ok, version)");
      }
      return { type: "status_result", ok: obj.ok, version: obj.version };
    }
    case "list_collections_result": {
      if (!Array.isArray(obj.collections)) {
        throw new Error("list_collections_result missing 'collections' array");
      }
      return { type: "list_collections_result", collections: obj.collections as string[] };
    }
    case "error": {
      if (typeof obj.message !== "string") {
        throw new Error("error response missing 'message' field");
      }
      return { type: "error", message: obj.message };
    }
    default:
      throw new Error(`Unknown native response type: ${obj.type}`);
  }
}

export function isErrorResponse(response: NativeResponse): response is ErrorResponse {
  return response.type === "error";
}

// --- Internal message types (UI ↔ Service Worker via chrome.runtime.sendMessage) ---

export interface RuntimeSaveMessage {
  type: "save";
  url: string;
  title?: string;
  tags?: string[];
  collection?: string;
  note?: string;
  selected_text?: string;
  action?: string;
}

export interface RuntimeStatusMessage {
  type: "status";
}

export interface RuntimeListCollectionsMessage {
  type: "list_collections";
}

export type RuntimeMessage = RuntimeSaveMessage | RuntimeStatusMessage | RuntimeListCollectionsMessage;

export interface RuntimeSuccessResponse {
  success: true;
  data: NativeResponse;
}

export interface RuntimeErrorResponse {
  success: false;
  error: string;
}

export type RuntimeResponse = RuntimeSuccessResponse | RuntimeErrorResponse;
