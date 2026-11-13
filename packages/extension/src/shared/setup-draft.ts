/**
 * The popup's unfinished setup answers (contract §4.4), kept in
 * `chrome.storage.session` because Chrome closes the popup whenever focus
 * leaves it, e.g. when the system folder dialog opens. The service worker
 * writes a chosen folder here; the popup restores from it when reopened.
 */

import { SETUP_DRAFT_KEY } from "./constants";
import type { PendingSave } from "./types";

export interface SetupDraft {
  storage_path: string | null;
  default_agent: string | null;
  enrichment_enabled: boolean | null;
  /**
   * The in-page item (e.g. a post) whose save setup interrupted, so the
   * reopened popup saves that item, not the whole tab. Same lifetime as the
   * rest of the draft.
   */
  pending_save: PendingSave | null;
  /** Epoch milliseconds of the last change. */
  updated_at: number;
}

/** A draft older than this is ignored. */
export const SETUP_DRAFT_MAX_AGE_MS = 600_000;

function sessionArea(): chrome.storage.StorageArea | undefined {
  return typeof chrome !== "undefined" ? chrome.storage?.session : undefined;
}

function parsePendingSave(raw: unknown): PendingSave | null {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const p = raw as Record<string, unknown>;
  if (typeof p.url !== "string" || p.url === "") return null;
  return {
    url: p.url,
    title: typeof p.title === "string" ? p.title : undefined,
    page_html: typeof p.page_html === "string" ? p.page_html : undefined,
  };
}

function parseDraft(raw: unknown): SetupDraft | null {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null;
  const d = raw as Record<string, unknown>;
  if (typeof d.updated_at !== "number" || !Number.isFinite(d.updated_at)) return null;
  return {
    storage_path: typeof d.storage_path === "string" ? d.storage_path : null,
    default_agent: typeof d.default_agent === "string" ? d.default_agent : null,
    enrichment_enabled: typeof d.enrichment_enabled === "boolean" ? d.enrichment_enabled : null,
    pending_save: parsePendingSave(d.pending_save),
    updated_at: d.updated_at,
  };
}

/** The current draft, or null when there is none, it is malformed, or it is stale. */
export async function loadSetupDraft(now: number = Date.now()): Promise<SetupDraft | null> {
  try {
    const area = sessionArea();
    if (!area) return null;
    const result = await area.get(SETUP_DRAFT_KEY);
    const draft = parseDraft(result?.[SETUP_DRAFT_KEY]);
    if (!draft || now - draft.updated_at > SETUP_DRAFT_MAX_AGE_MS) return null;
    return draft;
  } catch {
    return null;
  }
}

/**
 * Writes run one after another: each is a read-modify-write, and two quick
 * changes (or a change and the clear after saving) must not interleave.
 */
let writes: Promise<void> = Promise.resolve();

function enqueue(write: () => Promise<void>): Promise<void> {
  const next = writes.then(write, write);
  writes = next.catch(() => {});
  return next;
}

/** Merge `changes` into the draft and stamp it. Best-effort; never throws. */
export function updateSetupDraft(
  changes: Partial<Omit<SetupDraft, "updated_at">>,
  now: number = Date.now(),
): Promise<void> {
  return enqueue(() => writeDraft(changes, now));
}

async function writeDraft(changes: Partial<Omit<SetupDraft, "updated_at">>, now: number): Promise<void> {
  try {
    const area = sessionArea();
    if (!area) return;
    const current = (await loadSetupDraft(now)) ?? {
      storage_path: null,
      default_agent: null,
      enrichment_enabled: null,
      pending_save: null,
      updated_at: now,
    };
    await area.set({ [SETUP_DRAFT_KEY]: { ...current, ...changes, updated_at: now } });
  } catch {
    // Session storage unavailable: setup still works, it just is not remembered.
  }
}

/** Forget the draft once setup is saved. Best-effort; never throws. */
export function clearSetupDraft(): Promise<void> {
  return enqueue(async () => {
    try {
      await sessionArea()?.remove(SETUP_DRAFT_KEY);
    } catch {
      // Nothing to clear.
    }
  });
}
