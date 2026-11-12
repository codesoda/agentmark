/**
 * Which post URLs are already bookmarked, for the in-page save buttons.
 *
 * Answers are cached for the life of the page: these sites re-render and
 * recycle post nodes constantly, and a post that scrolls back into view must
 * not cost another round trip to the native host. Every failure is silent; a
 * URL that could not be looked up is simply not known to be saved.
 *
 * Bundled into the posts content script only, so no `src/shared/` imports.
 */

/** The native host considers at most this many URLs per lookup. */
export const MAX_LOOKUP_URLS = 200;

/** URL → saved, for every URL this page has an answer for. */
const savedCache = new Map<string, boolean>();

/** Whether `url` is known to be saved (true), known not to be (false), or not yet asked (undefined). */
export function cachedSaved(url: string): boolean | undefined {
  return savedCache.get(url);
}

/** Record a save that just succeeded, so a re-rendered post comes back as saved. */
export function rememberSaved(url: string): void {
  savedCache.set(url, true);
}

/** Forget every answer. For tests. */
export function resetSavedCache(): void {
  savedCache.clear();
}

/** The `saved` list from the service worker's `RuntimeResponse`, or undefined if it is anything else. */
function parseLookupResponse(response: unknown): string[] | undefined {
  const result = response as { success?: unknown; data?: { type?: unknown; saved?: unknown } } | null | undefined;
  if (result?.success !== true || result.data?.type !== "lookup_urls_result") return undefined;
  const saved = result.data.saved;
  if (!Array.isArray(saved) || !saved.every((url) => typeof url === "string")) return undefined;
  return saved as string[];
}

async function lookupBatch(urls: string[]): Promise<void> {
  let response: unknown;
  try {
    response = await chrome.runtime.sendMessage({ type: "lookup_urls", urls });
  } catch {
    // Extension reloaded or host unavailable: leave these URLs unknown.
    return;
  }
  const saved = parseLookupResponse(response);
  if (!saved) return;
  const savedSet = new Set(saved);
  for (const url of urls) {
    // A save recorded while the lookup was in flight wins over its answer.
    if (savedCache.get(url) !== true) savedCache.set(url, savedSet.has(url));
  }
}

/**
 * Ask which of `urls` are saved, in as few messages as the host's limit
 * allows, and record the answers in the cache. Never rejects.
 */
export async function lookupSavedUrls(urls: string[]): Promise<void> {
  const unique = Array.from(new Set(urls));
  const batches: Promise<void>[] = [];
  for (let i = 0; i < unique.length; i += MAX_LOOKUP_URLS) {
    batches.push(lookupBatch(unique.slice(i, i + MAX_LOOKUP_URLS)));
  }
  await Promise.all(batches);
}
