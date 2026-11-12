/**
 * X (Twitter) adapter for the in-page save button.
 *
 * X keeps stable `data-testid` hooks: each post is `article[data-testid="tweet"]`,
 * its timestamp links to the permalink, and its actions sit in a `role="group"`
 * bar. The permalink is in the DOM, so no MAIN-world helper is needed.
 */

import { isPrivatePageOf } from "./private-content";
import type { SiteAdapter } from "./save-button";

const STATUS_PATH = /^\/([A-Za-z0-9_]{1,15})\/status\/(\d+)/;

/** Canonical permalink for a status path, dropping `/photo/1`, `/analytics`, queries. */
export function statusUrl(href: string, base: string): string | undefined {
  let parsed: URL;
  try {
    parsed = new URL(href, base);
  } catch {
    return undefined;
  }
  const match = STATUS_PATH.exec(parsed.pathname);
  return match ? `https://x.com/${match[1]}/status/${match[2]}` : undefined;
}

/**
 * The permalink of the post an article shows.
 *
 * The timestamp link is the post's own permalink. A quoted post inside the
 * article has a timestamp too but no link around it, so the first *linked*
 * timestamp belongs to the outer post. On a post's own page the focal post may
 * have no linked timestamp; the address bar is used there.
 */
export function resolveTweetUrl(post: Element, location: Location): string | undefined {
  for (const time of Array.from(post.querySelectorAll("time"))) {
    const href = time.closest("a")?.getAttribute("href");
    const url = href ? statusUrl(href, location.origin) : undefined;
    if (url) return url;
  }
  return statusUrl(location.pathname, location.origin);
}

/** Display name of the post's author, e.g. "Ada Lovelace". */
export function tweetAuthor(post: Element): string | undefined {
  const name = post.querySelector('[data-testid="User-Name"] span')?.textContent?.trim();
  return name || undefined;
}

export const xAdapter: SiteAdapter = {
  postSelector: 'article[data-testid="tweet"]',
  // X's own action icons are this grey in both light and dark themes.
  idleColor: "rgb(113, 118, 123)",
  // X is usually dark; the lighter indigo reads better there than the default.
  savingColor: "#a5b4fc",
  findSlot(post) {
    // Messages pages are private: no buttons there.
    if (isPrivatePageOf("x.com", window.location)) return null;
    // The action bar (reply, repost, like, …). Promoted or collapsed items without one are skipped.
    const bars = post.querySelectorAll('[role="group"]');
    const bar = bars[bars.length - 1];
    return bar ? { parent: bar, before: null } : null;
  },
  resolveUrl: resolveTweetUrl,
  title(post) {
    const author = tweetAuthor(post);
    return author ? `Post by ${author} on X` : "Post on X";
  },
  snapshotRoot(post) {
    return post;
  },
};
