/**
 * Hacker News adapter for the in-page save button.
 *
 * Each comment on a discussion page gets a button. A comment's own page
 * (`item?id=<comment id>`) shows it with all of its replies, so saving a
 * comment captures that sub-thread: the CLI fetches that page and the comment's
 * markup from the current page is sent as the snapshot.
 *
 * Hacker News markup is plain and stable: one `tr.athing.comtr` per comment,
 * with the author and age in `.comhead`.
 */

import type { SiteAdapter } from "./save-button";

/** Permalink of a comment or story by its numeric ID. */
export function itemUrl(id: string): string | undefined {
  return /^\d+$/.test(id) ? `https://news.ycombinator.com/item?id=${id}` : undefined;
}

/** Username of the comment's author. */
export function commentAuthor(comment: Element): string | undefined {
  const user = comment.querySelector('.comhead a[href^="user?id="]')?.textContent?.trim();
  return user || undefined;
}

export const hnAdapter: SiteAdapter = {
  postSelector: "tr.athing.comtr",
  // The grey of Hacker News's own comment header links.
  idleColor: "#828282",
  size: 18,
  findSlot(comment) {
    const head = comment.querySelector(".comhead");
    // Before the fold/navigation links, after the author and age. Deleted
    // comments have no header and are skipped.
    return head ? { parent: head, before: head.querySelector(".navs") } : null;
  },
  resolveUrl(comment) {
    return itemUrl(comment.id);
  },
  title(comment) {
    const author = commentAuthor(comment);
    return author ? `Hacker News comment by ${author}` : "Hacker News comment";
  },
  snapshotRoot(comment) {
    return comment;
  },
};
