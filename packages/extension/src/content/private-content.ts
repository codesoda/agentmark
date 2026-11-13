/**
 * What counts as private on the sites with in-page save buttons: elements cut
 * out of every snapshot, and pages that get no buttons at all. Bundled into
 * the posts content script only.
 *
 * `src/shared/page-snapshot.ts` holds a copy (`SNAPSHOT_PRIVACY`) for the
 * whole-page snapshot, because content scripts cannot import shared code.
 * `private-content.test.ts` fails when the two drift.
 *
 * Verified markup: Reddit's `#reddit-chat` mount (logged out, 2026-11-12) and
 * X's docked messages drawer (`DMDrawer`). The other Reddit entries cover chat
 * windows that are not visible when logged out.
 */

export interface PrivacyRules {
  /** Removed from snapshots on every site (LinkedIn's messaging dock, X's messages drawer). */
  selector: string;
  /** Also removed on Reddit. */
  redditSelector: string;
  /** Reddit custom elements whose tag matches this (case-insensitive) are removed too. */
  redditTag: string;
  /** Pages with no buttons and no snapshot: per site, private hosts and a private path pattern. */
  pages: Array<{ domain: string; hosts: string[]; path: string }>;
}

export const PRIVACY: PrivacyRules = {
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
 * Whether the page is one of `domain`'s private pages (inbox, messages,
 * notifications, chat, mod mail): no buttons there. The caller already knows
 * the page is on `domain`.
 */
export function isPrivatePageOf(domain: string, location: Pick<Location, "hostname" | "pathname">): boolean {
  const page = PRIVACY.pages.find((rule) => rule.domain === domain);
  if (!page) return false;
  return page.hosts.includes(location.hostname.toLowerCase()) || new RegExp(page.path, "i").test(location.pathname);
}

/**
 * Remove every private element from `root` (a detached clone); `reddit` adds
 * Reddit's rules. Throws when the rules cannot be applied, or when `root`
 * itself is private: callers then send no snapshot.
 */
export function removePrivateElements(root: Element, reddit: boolean): void {
  const selector = reddit ? `${PRIVACY.selector}, ${PRIVACY.redditSelector}` : PRIVACY.selector;
  const tag = new RegExp(PRIVACY.redditTag, "i");
  const isPrivate = (el: Element) => el.matches(selector) || (reddit && el.tagName.includes("-") && tag.test(el.tagName));
  if (isPrivate(root)) throw new Error("the snapshot root is private");
  Array.from(root.querySelectorAll("*"))
    .filter(isPrivate)
    .forEach((el) => el.remove());
}
