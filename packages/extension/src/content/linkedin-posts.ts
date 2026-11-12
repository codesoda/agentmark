/**
 * LinkedIn adapter for the in-page save button (timeline, profile activity,
 * post pages).
 *
 * LinkedIn's class names are hashed; the hooks used here are `componentkey`
 * prefixes and `aria-label`s. Timeline posts carry no permalink in the DOM, so
 * the post ID is requested from the MAIN-world script (`linkedin-urn.ts`).
 */

import { buildSnapshot, injectSaveButtons, type SiteAdapter } from "./save-button";

export { handleSaveResult } from "./save-button";

/** Must match `RESOLVE_EVENT` in `linkedin-urn.ts` (the MAIN-world script is a separate bundle). */
export const RESOLVE_EVENT = "agentmark:resolve-post-urn";
/** Must match `URN_ATTR` in `linkedin-urn.ts`. */
export const URN_ATTR = "data-agentmark-post-urn";

const MENU_BUTTON_SELECTOR = 'button[aria-label^="Open control menu for post by "]';
const MENU_LABEL_PREFIX = "Open control menu for post by ";
const URN_PATTERN = /^urn:li:(?:activity|share|ugcPost):\d+$/;
const POST_PAGE_PATH = /^\/(?:feed\/update|posts)\//;

/** Permalink for a post URN. LinkedIn serves it for activity, share and ugcPost URNs. */
export function postUrl(urn: string): string {
  return `https://www.linkedin.com/feed/update/${urn}/`;
}

/** Author named in the post's own control-menu label, if present. */
export function postAuthor(post: Element): string | undefined {
  const label = post.querySelector(MENU_BUTTON_SELECTOR)?.getAttribute("aria-label");
  const author = label?.slice(MENU_LABEL_PREFIX.length).trim();
  return author || undefined;
}

/**
 * Ask the MAIN-world script for the post's URN. Event dispatch is synchronous
 * across script worlds, so the attribute is set by the time this returns.
 */
export function resolvePostUrn(post: Element): string | undefined {
  post.removeAttribute(URN_ATTR);
  post.dispatchEvent(new CustomEvent(RESOLVE_EVENT, { bubbles: true }));
  const urn = post.getAttribute(URN_ATTR) ?? "";
  return URN_PATTERN.test(urn) ? urn : undefined;
}

/**
 * The URL to save a post under: its permalink when the ID can be read, or the
 * current address when the page already is that post's own page.
 */
export function resolvePostUrl(post: Element, location: Location): string | undefined {
  const urn = resolvePostUrn(post);
  if (urn) return postUrl(urn);
  if (POST_PAGE_PATH.test(location.pathname)) return location.href;
  return undefined;
}

export const linkedinAdapter: SiteAdapter = {
  postSelector: '[componentkey^="update-card-focus"]',
  scratchAttributes: [URN_ATTR],
  // The post menu and dismiss buttons sit at the top of the header row, which
  // is as tall as the author block beside them.
  align: "start",
  alignWith(post) {
    return post.querySelector(MENU_BUTTON_SELECTOR);
  },
  findSlot(post) {
    // Anything without a post menu (ads, placeholders) is not a saveable post.
    const menuButton = post.querySelector(MENU_BUTTON_SELECTOR);
    return menuButton?.parentElement ? { parent: menuButton.parentElement, before: menuButton } : null;
  },
  resolveUrl: resolvePostUrl,
  title(post) {
    const author = postAuthor(post);
    return author ? `LinkedIn post by ${author}` : "LinkedIn post";
  },
  // The wrapper that holds a post's comments is preferred over the bare post.
  snapshotRoot(post) {
    return (
      post.closest('[componentkey*="pagedCommentsContainer"]') ??
      post.closest('[componentkey^="expanded"]') ??
      post
    );
  },
};

/** Serialize one post, with the comments shown under it, as a standalone page. */
export function buildPostSnapshot(post: Element, title: string): string {
  return buildSnapshot(post, title, linkedinAdapter);
}

/** Add a save button to every LinkedIn post under `root` that does not have one yet. */
export function injectButtons(root: ParentNode): number {
  return injectSaveButtons(root, linkedinAdapter);
}
