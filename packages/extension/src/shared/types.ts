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

export interface ShowRequest {
  type: "show";
  id: string;
}

export interface BookmarkChanges {
  user_tags?: string[];
  suggested_tags?: string[];
  collections?: string[];
  note?: string | null;
  state?: BookmarkStateFilter;
}

export interface UpdateRequest {
  type: "update";
  id: string;
  changes: BookmarkChanges;
}

export type NativeRequest = SaveRequest | StatusRequest | ListCollectionsRequest | ListRequest | ShowRequest | UpdateRequest;

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

export interface BookmarkDetail {
  id: string;
  url: string;
  title: string;
  summary: string | null;
  saved_at: string;
  capture_source: string;
  state: BookmarkStateFilter;
  user_tags: string[];
  suggested_tags: string[];
  collections: string[];
  note: string | null;
}

export interface ShowResultResponse {
  type: "show_result";
  bookmark: BookmarkDetail;
}

export interface UpdateResultResponse {
  type: "update_result";
  bookmark: BookmarkDetail;
}

export interface ErrorResponse {
  type: "error";
  message: string;
}

export type NativeResponse = SaveResultResponse | StatusResultResponse | ListCollectionsResultResponse | ListResultResponse | ShowResultResponse | UpdateResultResponse | ErrorResponse;

// --- Connection status ---

export type ConnectionStatus = "disconnected" | "connecting" | "connected" | "error";

// --- Runtime parsing helpers ---

const RESPONSE_TYPES = new Set(["save_result", "status_result", "list_collections_result", "list_result", "show_result", "update_result", "error"]);

function parseBookmarkDetail(raw: unknown, responseType: string): BookmarkDetail {
  if (raw === null || raw === undefined || typeof raw !== "object") {
    throw new Error(`${responseType} missing 'bookmark' object`);
  }
  const b = raw as Record<string, unknown>;
  const validStates = new Set(["inbox", "processed", "archived"]);

  if (typeof b.id !== "string" || typeof b.url !== "string" || typeof b.title !== "string" ||
      typeof b.saved_at !== "string" || typeof b.capture_source !== "string" ||
      typeof b.state !== "string" || !validStates.has(b.state) ||
      !Array.isArray(b.user_tags) || !Array.isArray(b.suggested_tags) ||
      !Array.isArray(b.collections)) {
    throw new Error(`${responseType} bookmark has invalid or missing fields`);
  }
  if (!(b.user_tags as unknown[]).every((t) => typeof t === "string")) {
    throw new Error(`${responseType} bookmark has non-string user_tags`);
  }
  if (!(b.suggested_tags as unknown[]).every((t) => typeof t === "string")) {
    throw new Error(`${responseType} bookmark has non-string suggested_tags`);
  }
  if (!(b.collections as unknown[]).every((t) => typeof t === "string")) {
    throw new Error(`${responseType} bookmark has non-string collections`);
  }
  if (b.summary !== null && b.summary !== undefined && typeof b.summary !== "string") {
    throw new Error(`${responseType} bookmark has invalid summary`);
  }
  if (b.note !== null && b.note !== undefined && typeof b.note !== "string") {
    throw new Error(`${responseType} bookmark has invalid note`);
  }

  return {
    id: b.id as string,
    url: b.url as string,
    title: b.title as string,
    summary: (b.summary as string | null) ?? null,
    saved_at: b.saved_at as string,
    capture_source: b.capture_source as string,
    state: b.state as BookmarkStateFilter,
    user_tags: b.user_tags as string[],
    suggested_tags: b.suggested_tags as string[],
    collections: b.collections as string[],
    note: (b.note as string | null) ?? null,
  };
}

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
    case "show_result":
    case "update_result": {
      const bookmark = parseBookmarkDetail(obj.bookmark, obj.type as string);
      return { type: obj.type as "show_result" | "update_result", bookmark } as NativeResponse;
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

export interface RuntimeShowMessage {
  type: "show";
  id: string;
}

export interface RuntimeUpdateMessage {
  type: "update";
  id: string;
  changes: BookmarkChanges;
}

export type RuntimeMessage = RuntimeSaveMessage | RuntimeStatusMessage | RuntimeListCollectionsMessage | RuntimeListMessage | RuntimeShowMessage | RuntimeUpdateMessage;

export interface RuntimeSuccessResponse {
  success: true;
  data: NativeResponse;
}

export interface RuntimeErrorResponse {
  success: false;
  error: string;
}

export type RuntimeResponse = RuntimeSuccessResponse | RuntimeErrorResponse;
