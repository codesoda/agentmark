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
  /** HTML snapshot of the live tab; the CLI uses it when it cannot fetch the URL itself. */
  page_html?: string;
}

export interface StatusRequest {
  type: "status";
  /** This extension's version, so the host can record which build connected. */
  extension_version?: string;
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

/** Which of these URLs are already bookmarked. The host considers at most 200 per request. */
export interface LookupUrlsRequest {
  type: "lookup_urls";
  urls: string[];
}

export interface GetSetupStateRequest {
  type: "get_setup_state";
}

/** Each key is optional: absent = keep the current value (contract §5.2). */
export interface SettingsChanges {
  storage_path?: string;
  default_agent?: string;
  enrichment_enabled?: boolean;
  /** `null` clears the instructions. */
  system_prompt?: string | null;
  /** Replaces the whole user list. */
  snapshot_sites?: string[];
}

export interface SaveSettingsRequest {
  type: "save_settings";
  settings: SettingsChanges;
}

/** Sent only one-shot (`sendNativeMessage`), never on the shared port: the dialog can stay open for minutes. */
export interface ChooseFolderRequest {
  type: "choose_folder";
  start_path?: string;
}

export type NativeRequest = SaveRequest | StatusRequest | ListCollectionsRequest | ListRequest | ShowRequest | UpdateRequest | LookupUrlsRequest | GetSetupStateRequest | SaveSettingsRequest | ChooseFolderRequest;

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
  /** Whether AgentMark is set up. Absent from CLIs older than 0.3.0: treat as set up. */
  initialized?: boolean;
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

export interface LookupUrlsResultResponse {
  type: "lookup_urls_result";
  /** The subset of the requested URL strings, verbatim, that are already bookmarked. */
  saved: string[];
}

export type ErrorCode = "setup_required";

export interface ErrorResponse {
  type: "error";
  message: string;
  /** Machine-readable reason; only `setup_required` exists. Unknown codes are dropped on parse. */
  code?: ErrorCode;
}

export type AgentId = "claude" | "codex" | "pi";

export interface AgentInfo {
  id: string;
  /** Display name; the UI never hardcodes agent names. */
  name: string;
  found: boolean;
  path: string | null;
}

export interface Settings {
  storage_path: string;
  storage_path_display: string;
  default_agent: string;
  enrichment_enabled: boolean;
  system_prompt: string | null;
  /** User sites only; the built-in rules are in `SetupState.builtin_snapshot_sites`. */
  snapshot_sites: string[];
}

export interface SetupState {
  initialized: boolean;
  config_problem: string | null;
  version: string;
  platform: { os: string; arch: string };
  settings: Settings;
  default_storage_path: string;
  default_storage_path_display: string;
  agents: AgentInfo[];
  builtin_snapshot_sites: string[];
  bookmark_count: number | null;
}

export interface SetupStateResultResponse {
  type: "setup_state_result";
  state: SetupState;
}

export interface SaveSettingsResultResponse {
  type: "save_settings_result";
  state: SetupState;
}

export type SettingsField = "storage_path" | "default_agent" | "enrichment_enabled" | "system_prompt" | "snapshot_sites";

export type FieldErrorCode =
  | "blank"
  | "not_absolute"
  | "too_long"
  | "not_a_directory"
  | "cannot_create"
  | "not_writable"
  | "invalid_agent"
  | "too_many"
  | "invalid_site";

export interface FieldError {
  field: SettingsField;
  /** Only for `snapshot_sites` entries. */
  index: number | null;
  /** One of `FieldErrorCode`; a newer CLI may send others, so it is kept as a string. */
  code: string;
  /** A sentence for the user, shown as-is. */
  message: string;
}

export interface SettingsInvalidResponse {
  type: "settings_invalid";
  errors: FieldError[];
}

export type ChooseFolderOutcome = "chosen" | "cancelled" | "unavailable";

export interface ChooseFolderResultResponse {
  type: "choose_folder_result";
  outcome: ChooseFolderOutcome;
  path: string | null;
  path_display: string | null;
  reason: string | null;
}

export type NativeResponse = SaveResultResponse | StatusResultResponse | ListCollectionsResultResponse | ListResultResponse | ShowResultResponse | UpdateResultResponse | LookupUrlsResultResponse | SetupStateResultResponse | SaveSettingsResultResponse | SettingsInvalidResponse | ChooseFolderResultResponse | ErrorResponse;

// --- Connection status ---

export type ConnectionStatus = "disconnected" | "connecting" | "connected" | "error";

// --- Runtime parsing helpers ---

const RESPONSE_TYPES = new Set(["save_result", "status_result", "list_collections_result", "list_result", "show_result", "update_result", "lookup_urls_result", "setup_state_result", "save_settings_result", "settings_invalid", "choose_folder_result", "error"]);

const SETTINGS_FIELDS = new Set<string>(["storage_path", "default_agent", "enrichment_enabled", "system_prompt", "snapshot_sites"]);
const FOLDER_OUTCOMES = new Set<string>(["chosen", "cancelled", "unavailable"]);

function isObject(raw: unknown): raw is Record<string, unknown> {
  return raw !== null && typeof raw === "object" && !Array.isArray(raw);
}

function isStringArray(raw: unknown): raw is string[] {
  return Array.isArray(raw) && raw.every((item) => typeof item === "string");
}

function isNullableString(raw: unknown): raw is string | null {
  return raw === null || typeof raw === "string";
}

function parseSettings(raw: unknown, where: string): Settings {
  if (!isObject(raw)) throw new Error(`${where} missing 'settings' object`);
  if (typeof raw.storage_path !== "string" || typeof raw.storage_path_display !== "string" ||
      typeof raw.default_agent !== "string" || typeof raw.enrichment_enabled !== "boolean" ||
      !isNullableString(raw.system_prompt) || !isStringArray(raw.snapshot_sites)) {
    throw new Error(`${where} settings has invalid or missing fields`);
  }
  return {
    storage_path: raw.storage_path,
    storage_path_display: raw.storage_path_display,
    default_agent: raw.default_agent,
    enrichment_enabled: raw.enrichment_enabled,
    system_prompt: raw.system_prompt,
    snapshot_sites: raw.snapshot_sites,
  };
}

function parseAgent(raw: unknown, where: string, i: number): AgentInfo {
  if (!isObject(raw) || typeof raw.id !== "string" || typeof raw.name !== "string" ||
      typeof raw.found !== "boolean" || !isNullableString(raw.path)) {
    throw new Error(`${where} agents[${i}] has invalid or missing fields`);
  }
  return { id: raw.id, name: raw.name, found: raw.found, path: raw.path };
}

function parseSetupState(raw: unknown, where: string): SetupState {
  if (!isObject(raw)) throw new Error(`${where} missing 'state' object`);
  const platform = raw.platform;
  if (typeof raw.initialized !== "boolean" || !isNullableString(raw.config_problem) ||
      typeof raw.version !== "string" ||
      !isObject(platform) || typeof platform.os !== "string" || typeof platform.arch !== "string" ||
      typeof raw.default_storage_path !== "string" || typeof raw.default_storage_path_display !== "string" ||
      !Array.isArray(raw.agents) || !isStringArray(raw.builtin_snapshot_sites) ||
      !(raw.bookmark_count === null || (typeof raw.bookmark_count === "number" && Number.isFinite(raw.bookmark_count)))) {
    throw new Error(`${where} state has invalid or missing fields`);
  }
  return {
    initialized: raw.initialized,
    config_problem: raw.config_problem,
    version: raw.version,
    platform: { os: platform.os, arch: platform.arch },
    settings: parseSettings(raw.settings, where),
    default_storage_path: raw.default_storage_path,
    default_storage_path_display: raw.default_storage_path_display,
    agents: (raw.agents as unknown[]).map((agent, i) => parseAgent(agent, where, i)),
    builtin_snapshot_sites: raw.builtin_snapshot_sites,
    bookmark_count: raw.bookmark_count as number | null,
  };
}

function parseFieldError(raw: unknown, i: number): FieldError {
  if (!isObject(raw) || typeof raw.field !== "string" || !SETTINGS_FIELDS.has(raw.field) ||
      !(raw.index === null || raw.index === undefined || (Number.isInteger(raw.index) && (raw.index as number) >= 0)) ||
      typeof raw.code !== "string" || typeof raw.message !== "string") {
    throw new Error(`settings_invalid errors[${i}] has invalid or missing fields`);
  }
  return {
    field: raw.field as SettingsField,
    index: (raw.index as number | null | undefined) ?? null,
    code: raw.code,
    message: raw.message,
  };
}

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
      if (obj.initialized !== undefined && typeof obj.initialized !== "boolean") {
        throw new Error("status_result has a non-boolean 'initialized'");
      }
      const status: StatusResultResponse = { type: "status_result", ok: obj.ok, version: obj.version };
      if (obj.initialized !== undefined) status.initialized = obj.initialized;
      return status;
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
    case "lookup_urls_result": {
      if (!Array.isArray(obj.saved)) {
        throw new Error("lookup_urls_result missing 'saved' array");
      }
      if (!(obj.saved as unknown[]).every((u) => typeof u === "string")) {
        throw new Error("lookup_urls_result has non-string saved entries");
      }
      return { type: "lookup_urls_result", saved: obj.saved as string[] };
    }
    case "error": {
      if (typeof obj.message !== "string") {
        throw new Error("error response missing 'message' field");
      }
      const error: ErrorResponse = { type: "error", message: obj.message };
      if (obj.code === "setup_required") error.code = obj.code;
      return error;
    }
    case "setup_state_result":
    case "save_settings_result": {
      const state = parseSetupState(obj.state, obj.type);
      return { type: obj.type, state } as SetupStateResultResponse | SaveSettingsResultResponse;
    }
    case "settings_invalid": {
      if (!Array.isArray(obj.errors) || obj.errors.length === 0) {
        throw new Error("settings_invalid missing non-empty 'errors' array");
      }
      return { type: "settings_invalid", errors: obj.errors.map(parseFieldError) };
    }
    case "choose_folder_result": {
      if (typeof obj.outcome !== "string" || !FOLDER_OUTCOMES.has(obj.outcome) ||
          !isNullableString(obj.path ?? null) || !isNullableString(obj.path_display ?? null) ||
          !isNullableString(obj.reason ?? null)) {
        throw new Error("choose_folder_result has invalid or missing fields");
      }
      if (obj.outcome === "chosen" && (typeof obj.path !== "string" || obj.path === "")) {
        throw new Error("choose_folder_result chosen without a path");
      }
      const path = (obj.path as string | null | undefined) ?? null;
      return {
        type: "choose_folder_result",
        outcome: obj.outcome as ChooseFolderOutcome,
        path,
        path_display: (obj.path_display as string | null | undefined) ?? path,
        reason: (obj.reason as string | null | undefined) ?? null,
      };
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
  /** HTML snapshot of the live tab; the CLI uses it when it cannot fetch the URL itself. */
  page_html?: string;
}

/** A save prepared outside the popup (e.g. by an in-page button) for the popup to carry out. */
export interface PendingSave {
  url: string;
  title?: string;
  page_html?: string;
}

/**
 * Ask the service worker to open the popup for this save, so the user gets the
 * usual saving → saved view with tags and collections. If the popup cannot be
 * opened the service worker saves directly instead.
 */
export interface RuntimeOpenSavePopupMessage extends PendingSave {
  type: "open_save_popup";
}

/** Sent by the popup on open: hand over the pending save, if there is one. */
export interface RuntimeTakePendingSaveMessage {
  type: "take_pending_save";
}

/** Sent by the service worker to the tab whose in-page button started a save. */
export interface SaveResultNotice {
  type: "agentmark:save-result";
  url: string;
  success: boolean;
  error?: string;
}

export interface RuntimeStatusMessage {
  type: "status";
}

export interface RuntimeGetSetupStateMessage {
  type: "get_setup_state";
}

export interface RuntimeSaveSettingsMessage {
  type: "save_settings";
  settings: SettingsChanges;
}

export interface RuntimeChooseFolderMessage {
  type: "choose_folder";
  start_path?: string;
}

/** Sent by the posts content script: "this tab shows this page" (contract §3.3). */
export interface RuntimeReportPageMessage {
  type: "report_page";
  url: string;
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

/** Sent by in-page buttons: which of these post URLs are already bookmarked. */
export interface RuntimeLookupUrlsMessage {
  type: "lookup_urls";
  urls: string[];
}

export type RuntimeMessage = RuntimeSaveMessage | RuntimeOpenSavePopupMessage | RuntimeTakePendingSaveMessage | RuntimeStatusMessage | RuntimeListCollectionsMessage | RuntimeListMessage | RuntimeShowMessage | RuntimeUpdateMessage | RuntimeLookupUrlsMessage | RuntimeGetSetupStateMessage | RuntimeSaveSettingsMessage | RuntimeChooseFolderMessage | RuntimeReportPageMessage;

export interface RuntimeSuccessResponse {
  success: true;
  data: NativeResponse;
}

export interface RuntimeErrorResponse {
  success: false;
  error: string;
  /** Copied from the native `error.code`. */
  code?: ErrorCode;
}

export type RuntimeResponse = RuntimeSuccessResponse | RuntimeErrorResponse;
