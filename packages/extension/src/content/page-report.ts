/**
 * Tells the service worker which page this tab shows, so the toolbar icon can
 * turn blue on a saved page without the optional `tabs` permission
 * (docs/setup-contract.md §3.3). Sent once per distinct `location.href`.
 *
 * Bundled into the posts content script only, so no `src/shared/` imports:
 * `REPORT_PAGE_MESSAGE` duplicates the shared constant (a test keeps them equal).
 */

export const REPORT_PAGE_MESSAGE = "report_page";

let lastReported: string | undefined;

/** Report `location.href` if it changed since the last report. Never throws. */
export function reportPageIfChanged(location: Location = window.location): void {
  const url = location.href;
  if (url === lastReported) return;
  lastReported = url;
  try {
    void Promise.resolve(chrome.runtime.sendMessage({ type: REPORT_PAGE_MESSAGE, url })).catch(() => {});
  } catch {
    // Extension reloaded: nothing to tell.
  }
}

/** Forget what was reported. For tests. */
export function resetPageReport(): void {
  lastReported = undefined;
}
