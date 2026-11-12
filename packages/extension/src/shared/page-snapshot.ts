/**
 * Page snapshot capture, injected into the live tab via chrome.scripting.
 *
 * `capturePageHtml` is serialized and run inside the page, so it must stay
 * self-contained: no imports, no references to module-level values.
 */

/**
 * Decide whether a page must be scrolled before it is snapshotted.
 * LinkedIn mounts profile and company sections (Experience, Education, About, …)
 * only when they are scrolled into view. Feeds scroll forever, so the timeline
 * and a company's posts tab are not scrolled.
 *
 * Truncated "… more" text needs no handling: the full text is already in the
 * DOM and only clamped visually. Do not click those buttons — on posts they sit
 * inside a link and a synthetic click navigates away from the page.
 */
export function shouldScrollBeforeSnapshot(url: string | undefined): boolean {
  let parsed: URL;
  try {
    parsed = new URL(url ?? "");
  } catch {
    return false;
  }
  const isLinkedIn = parsed.hostname === "linkedin.com" || parsed.hostname.endsWith(".linkedin.com");
  if (!isLinkedIn) return false;
  const path = parsed.pathname;
  if (path.startsWith("/in/")) return true;
  return path.startsWith("/company/") && !/\/posts\/?$/.test(path);
}

/** Runs inside the page. Returns the rendered HTML after optional preparation. */
export async function capturePageHtml(scroll: boolean): Promise<string> {
  const BUDGET_MS = 10_000;
  const STEP_WAIT_MS = 300;
  const deadline = Date.now() + BUDGET_MS;
  const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

  try {
    if (scroll) {
      // App-shell layouts scroll an inner container rather than the document.
      const root = document.scrollingElement ?? document.documentElement;
      let scroller: Element = root;
      if (root.scrollHeight <= root.clientHeight + 1) {
        for (let el: Element | null = document.querySelector("main"); el; el = el.parentElement) {
          const overflowY = getComputedStyle(el).overflowY;
          if ((overflowY === "auto" || overflowY === "scroll") && el.scrollHeight > el.clientHeight + 1) {
            scroller = el;
            break;
          }
        }
      }

      const start = scroller.scrollTop;
      let settled = 0;
      while (Date.now() < deadline && settled < 3) {
        const heightBefore = scroller.scrollHeight;
        scroller.scrollTop = scroller.scrollTop + Math.max(scroller.clientHeight * 0.9, 200);
        await sleep(STEP_WAIT_MS);
        const atBottom = scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 2;
        settled = atBottom && scroller.scrollHeight === heightBefore ? settled + 1 : 0;
      }
      scroller.scrollTop = start;
    }
  } catch {
    // Preparation is best-effort — always fall through to the snapshot.
  }

  // LinkedIn renders its older pages (company pages, among others) inside a
  // same-origin iframe laid over the app. The main document then still holds
  // the *previous* page, and `outerHTML` does not include the iframe. The iframe
  // is always present; it is the page only while it is opaque.
  //
  // Never read the `#interop-outlet` shadow root: it hosts the messaging
  // overlay, i.e. the user's private conversations, on every page.
  try {
    const MIN_TEXT = 200;
    for (const frame of Array.from(
      document.querySelectorAll<HTMLIFrameElement>('iframe[data-testid="interop-iframe"]'),
    )) {
      if (getComputedStyle(frame).opacity === "0") continue;
      let body: HTMLElement | null = null;
      try {
        body = frame.contentDocument?.body ?? null;
      } catch {
        // Cross-origin frame — not readable.
      }
      if (!body) continue;

      const page = body.cloneNode(true) as HTMLElement;
      // The iframe carries its own copy of the messaging dock: private, and not page content.
      page
        .querySelectorAll('[class*="msg-overlay"], [id*="msg-overlay"], script, style')
        .forEach((el) => el.remove());
      if ((page.textContent ?? "").trim().length < MIN_TEXT) continue;

      const title = document.title.replace(/&/g, "&amp;").replace(/</g, "&lt;");
      return `<!DOCTYPE html><html><head><title>${title}</title></head><body><div data-agentmark-source="iframe">${page.innerHTML}</div></body></html>`;
    }
  } catch {
    // Fall through to the plain document snapshot.
  }

  return document.documentElement?.outerHTML ?? "";
}
