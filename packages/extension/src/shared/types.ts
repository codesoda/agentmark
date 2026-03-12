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

export type BookmarkStateFilter = "inbox" | "processed" | "archived";

export interface ListRequest {
  type: "list";
  limit?: number;
  state?: BookmarkStateFilter;
}

export type NativeRequest = SaveRequest | StatusRequest | ListCollectionsRequest | ListRequest;

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

export interface BookmarkSummary {
  id: string;
  url: string;
  title: string;
  state: BookmarkStateFilter;
  user_tags: string[];
  suggested_tags: string[];
  saved_at: string;
}

export interface ListResultResponse {
  type: "list_result";
  bookmarks: BookmarkSummary[];
}

export interface ErrorResponse {
  type: "error";
  message: string;
}

export type NativeResponse = SaveResultResponse | StatusResultResponse | ListCollectionsResultResponse | ListResultResponse | ErrorResponse;

// --- Connection status ---

export type ConnectionStatus = "disconnected" | "connecting" | "connected" | "error";

// --- Runtime parsing helpers ---

const RESPONSE_TYPES = new Set(["save_result", "status_result", "list_collections_result", "list_result", "error"]);

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
    case "list_result": {
      if (!Array.isArray(obj.bookmarks)) {
        throw new Error("list_result missing 'bookmarks' array");
      }
      const validStates = new Set(["inbox", "processed", "archived"]);
      const bookmarks = (obj.bookmarks as unknown[]).map((item, i) => {
        if (item === null || item === undefined || typeof item !== "object") {
          throw new Error(`list_result bookmark[${i}] is not an object`);
        }
        const b = item as Record<string, unknown>;
        if (typeof b.id !== "string" || typeof b.url !== "string" || typeof b.title !== "string" ||
            typeof b.state !== "string" || !validStates.has(b.state) ||
            !Array.isArray(b.user_tags) || !Array.isArray(b.suggested_tags) ||
            typeof b.saved_at !== "string") {
          throw new Error(`list_result bookmark[${i}] has invalid or missing fields`);
        }
        if (!(b.user_tags as unknown[]).every((t) => typeof t === "string")) {
          throw new Error(`list_result bookmark[${i}] has non-string user_tags`);
        }
        if (!(b.suggested_tags as unknown[]).every((t) => typeof t === "string")) {
          throw new Error(`list_result bookmark[${i}] has non-string suggested_tags`);
        }
        return {
          id: b.id,
          url: b.url,
          title: b.title,
          state: b.state as BookmarkStateFilter,
          user_tags: b.user_tags as string[],
          suggested_tags: b.suggested_tags as string[],
          saved_at: b.saved_at,
        };
      });
      return { type: "list_result", bookmarks };
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

export interface RuntimeListMessage {
  type: "list";
  limit?: number;
  state?: BookmarkStateFilter;
}

export type RuntimeMessage = RuntimeSaveMessage | RuntimeStatusMessage | RuntimeListCollectionsMessage | RuntimeListMessage;

export interface RuntimeSuccessResponse {
  success: true;
  data: NativeResponse;
}

export interface RuntimeErrorResponse {
  success: false;
  error: string;
}

export type RuntimeResponse = RuntimeSuccessResponse | RuntimeErrorResponse;
