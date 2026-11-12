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

/**
 * What is private on the sites AgentMark snapshots: a copy of `PRIVACY` in
 * `src/content/private-content.ts` (content scripts cannot import shared
 * code). `private-content.test.ts` fails when the two drift. Passed to
 * `capturePageHtml` as an argument, because that function runs in the page
 * and cannot see module-level values.
 */
export interface SnapshotPrivacy {
  /** Removed from snapshots on every site (LinkedIn's messaging dock, X's messages drawer). */
  selector: string;
  /** Also removed on Reddit. */
  redditSelector: string;
  /** Reddit custom elements whose tag matches this (case-insensitive) are removed too. */
  redditTag: string;
  /** Pages with no snapshot: per site, private hosts and a private path pattern. */
  pages: Array<{ domain: string; hosts: string[]; path: string }>;
}

export const SNAPSHOT_PRIVACY: SnapshotPrivacy = {
  selector:
    '[class*="msg-overlay"], [id*="msg-overlay"], [data-testid="DMDrawer"], [data-testid="DMDrawerHeader"]',
  redditSelector: '#reddit-chat, [id^="chat-" i], iframe[src*="chat.reddit.com"]',
  redditTag: "(?:^|-)(?:chat|inbox|notifications?|messages?|messaging)(?:-|$)",
  pages: [
    { domain: "reddit.com", hosts: ["chat.reddit.com", "mod.reddit.com"], path: "^/(?:message|notifications|chat|mail)(?:/|$)" },
    { domain: "x.com", hosts: [], path: "^/(?:messages|i/chat)(?:/|$)" },
    { domain: "twitter.com", hosts: [], path: "^/(?:messages|i/chat)(?:/|$)" },
  ],
};

/**
 * Runs inside the page. Returns the rendered HTML after optional preparation.
 *
 * Private conversations never leave the page: private pages (Reddit inbox,
 * notifications, chat and mod mail, X messages) give no snapshot at all, and
 * chat windows are cut out of every other page (LinkedIn's messaging dock
 * lives in a shadow root, which `outerHTML` never includes). If the private
 * parts cannot be removed for certain, the result is "" (no snapshot): this
 * fails closed.
 */
export async function capturePageHtml(scroll: boolean, privacy: SnapshotPrivacy): Promise<string> {
  let isPrivate: (el: Element) => boolean;
  try {
    const host = location.hostname.toLowerCase();
    const onSite = (domain: string) => host === domain || host.endsWith(`.${domain}`);
    for (const page of privacy.pages) {
      if (onSite(page.domain) && (page.hosts.includes(host) || new RegExp(page.path, "i").test(location.pathname))) {
        return "";
      }
    }
    const onReddit = onSite("reddit.com");
    const selector = onReddit ? `${privacy.selector}, ${privacy.redditSelector}` : privacy.selector;
    const tag = new RegExp(privacy.redditTag, "i");
    isPrivate = (el: Element) => el.matches(selector) || (onReddit && el.tagName.includes("-") && tag.test(el.tagName));
    // An invalid rule throws here, before anything is captured.
    isPrivate(document.documentElement);
  } catch {
    return "";
  }

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
      try {
        Array.from(page.querySelectorAll("*"))
          .filter((el) => isPrivate(el) || el.matches("script, style"))
          .forEach((el) => el.remove());
      } catch {
        return "";
      }
      if ((page.textContent ?? "").trim().length < MIN_TEXT) continue;

      const title = document.title.replace(/&/g, "&amp;").replace(/</g, "&lt;");
      return `<!DOCTYPE html><html><head><title>${title}</title></head><body><div data-agentmark-source="iframe">${page.innerHTML}</div></body></html>`;
    }
  } catch {
    // Fall through to the plain document snapshot.
  }

  try {
    const root = document.documentElement;
    if (!root) return "";
    if (!Array.from(root.querySelectorAll("*")).some(isPrivate)) return root.outerHTML;
    const clone = root.cloneNode(true) as Element;
    Array.from(clone.querySelectorAll("*"))
      .filter(isPrivate)
      .forEach((el) => el.remove());
    return clone.outerHTML;
  } catch {
    // Private parts may still be in the page: send nothing.
    return "";
  }
}
