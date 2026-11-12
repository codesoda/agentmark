/**
 * Reddit adapters for the in-page save button: posts in feeds and subreddit
 * listings, the post on its own page, and every comment.
 *
 * Markup checked against live Reddit on 2026-11-12 (logged out, desktop), and
 * old Reddit against Wayback Machine copies (2025-02, 2026-08), because
 * old.reddit.com now sends logged-out visitors to a login page.
 * reddit.com, new.reddit.com and sh.reddit.com all redirect to www.reddit.com.
 *
 * www.reddit.com ("shreddit"), web components with open shadow roots:
 * - Post: `shreddit-post` with `permalink`, `post-title`, `author` and
 *   `view-context` ("SubredditFeed", "CommentsPage", …) attributes. Title,
 *   author line and text body are light-DOM children assigned to named slots;
 *   the action row (vote, comments, share) is drawn inside the shadow root and
 *   exposes the slots `share-button`, `mod-content-actions`,
 *   `promote-post-button` and `action-row-whitespace`.
 * - Comment: `shreddit-comment` with `permalink`
 *   (`/r/<sub>/comments/<post>/comment/<id>/`), `thingid`, `author` and
 *   `depth`. No shadow root of its own; replies are nested light-DOM
 *   `shreddit-comment`s under `#comment-children`. Its controls are a light-DOM
 *   `shreddit-comment-action-row` whose shadow root exposes the slots
 *   `comment-reply`, `comment-award`, `comment-share`, `comment-insight` and
 *   `overflow`, in that order.
 * - The button is a light-DOM child of the post or action row, shown through
 *   one of those slots. Reddit's components render only into their shadow
 *   roots, so they leave it alone; the core's observer re-adds it if a post or
 *   comment is replaced. Infinite scroll and "more replies" add light-DOM
 *   elements, which the observer sees.
 * - Chat mounts in `#reddit-chat` inside `reddit-header-large` (present and
 *   empty when logged out).
 *
 * old.reddit.com: `.thing.link` (posts) and `.thing.comment` rows with
 * `data-permalink`, `data-author` and `data-fullname`; each row's own links
 * are `ul.flat-list.buttons` in `:scope > .entry` (comments) or
 * `:scope > .entry > .top-matter` (posts), and replies nest in
 * `:scope > .child`. Promoted posts carry `.promoted`.
 *
 * Save URLs are absolute www.reddit.com permalinks read from those attributes,
 * so a post or comment saved from either site gets the same URL.
 */

import { isPrivatePageOf } from "./private-content";
import type { SiteAdapter } from "./save-button";

const ORIGIN = "https://www.reddit.com";

/**
 * Inbox, notification, chat and mod mail pages (and chat.reddit.com,
 * mod.reddit.com) are private: no save buttons there. The rules, and the
 * private elements cut out of snapshots, live in `private-content.ts`.
 */
export function isPrivateRedditPage(location: Pick<Location, "hostname" | "pathname">): boolean {
  return isPrivatePageOf("reddit.com", location);
}

// /r/<sub>/comments/<post>[/<slug>][/comment/<id> | /<id>]
const PERMALINK =
  /^\/(r|u|user)\/([^/]+)\/comments\/([a-z0-9]+)(?:\/(comment\/([a-z0-9]+)|[^/]*)(?:\/([a-z0-9]+))?)?\/?$/i;

/**
 * Canonical www.reddit.com URL for a post or comment permalink, or undefined
 * when `href` is not one. Comments use Reddit's own short form
 * (`/comments/<post>/comment/<id>/`) whichever form the page used, so the
 * same comment saved from old and new Reddit gets the same URL.
 */
export function redditPermalink(href: string | null | undefined): string | undefined {
  if (!href) return undefined;
  let parsed: URL;
  try {
    parsed = new URL(href, ORIGIN);
  } catch {
    return undefined;
  }
  const host = parsed.hostname.toLowerCase();
  if (host !== "reddit.com" && !host.endsWith(".reddit.com")) return undefined;
  const match = PERMALINK.exec(parsed.pathname);
  if (!match) return undefined;
  const [, kind, name, post, rest, shortComment, slugComment] = match;
  const base = `${ORIGIN}/${kind}/${name}/comments/${post}`;
  const comment = shortComment ?? slugComment;
  if (comment) return `${base}/comment/${comment}/`;
  return rest ? `${base}/${rest}/` : `${base}/`;
}

/** Plain author name, without the "u/" prefix; undefined for deleted authors. */
function authorName(raw: string | null | undefined): string | undefined {
  const name = raw?.trim().replace(/^u\//, "");
  return name && name !== "[deleted]" ? name : undefined;
}

const NEW_POST = "shreddit-post";
const NEW_COMMENT = "shreddit-comment";

/** Author of a post or comment on www.reddit.com. */
export function newRedditAuthor(item: Element): string | undefined {
  return authorName(item.getAttribute("author"));
}

/** The comment's own action row, not one of its replies'. */
function ownActionRow(comment: Element): Element | null {
  for (const row of Array.from(comment.querySelectorAll("shreddit-comment-action-row"))) {
    if (row.closest(NEW_COMMENT) === comment) return row;
  }
  return null;
}

export const redditAdapter: SiteAdapter = {
  postSelector: `${NEW_POST}, ${NEW_COMMENT}`,
  nested: true,
  // Reddit's own weak text colour, which follows its light and dark themes.
  idleColor: "var(--color-neutral-content-weak, #5C6C74)",
  // Reddit's action buttons are 32px tall.
  size: 32,
  privacy: "reddit",
  findSlot(item) {
    if (isPrivateRedditPage(window.location)) return null;
    if (item.tagName.toLowerCase() === NEW_POST) {
      // The right-hand group of the post's action row, drawn in its shadow root.
      return { parent: item, before: null, slot: "promote-post-button" };
    }
    // Right after Share, before the overflow menu. Comments without controls
    // (deleted, collapsed placeholders) are skipped.
    const row = ownActionRow(item);
    return row ? { parent: row, before: null, slot: "comment-share" } : null;
  },
  resolveUrl(item) {
    return redditPermalink(item.getAttribute("permalink"));
  },
  title(item) {
    const author = newRedditAuthor(item);
    if (item.tagName.toLowerCase() === NEW_POST) {
      const title = item.getAttribute("post-title")?.trim();
      if (title) return title;
      return author ? `Reddit post by u/${author}` : "Reddit post";
    }
    return author ? `Reddit comment by u/${author}` : "Reddit comment";
  },
  snapshotRoot(item) {
    // A post on its own page is saved with the comments shown under it.
    if (item.tagName.toLowerCase() === NEW_POST && item.getAttribute("view-context") === "CommentsPage") {
      return item.closest("main") ?? item;
    }
    return item;
  },
};

const OLD_ITEM = ".thing.link, .thing.comment";

export const oldRedditAdapter: SiteAdapter = {
  postSelector: OLD_ITEM,
  nested: true,
  // The grey of old Reddit's own "comments · share · save" links.
  idleColor: "#888",
  size: 16,
  privacy: "reddit",
  findSlot(thing) {
    if (isPrivateRedditPage(window.location)) return null;
    if (thing.classList.contains("promoted")) return null;
    const buttons = thing.querySelector(
      ":scope > .entry > ul.flat-list.buttons, :scope > .entry > .top-matter > ul.flat-list.buttons",
    );
    return buttons ? { parent: buttons, before: null } : null;
  },
  resolveUrl(thing) {
    return redditPermalink(thing.getAttribute("data-permalink"));
  },
  title(thing) {
    const author = authorName(thing.getAttribute("data-author"));
    if (thing.classList.contains("link")) {
      const title = thing.querySelector(":scope > .entry a.title")?.textContent?.trim();
      if (title) return title;
      return author ? `Reddit post by u/${author}` : "Reddit post";
    }
    return author ? `Reddit comment by u/${author}` : "Reddit comment";
  },
  snapshotRoot(thing) {
    // A post on its own page is saved with the comments shown under it.
    if (thing.classList.contains("link") && thing.getAttribute("data-context") === "comments") {
      return thing.closest(".content") ?? thing;
    }
    return thing;
  },
};
