/*
 * The rest of the fictional library: other pages saved around the sample,
 * so `list`, `search`, `ls` and the bundle tree have neighbours to show.
 */

import { slugify } from "./files-bundle";
import type { ListedBookmark } from "./terminal-format";

function other(
  id: string,
  title: string,
  saved_at: string,
  tags: string[],
  state: ListedBookmark["state"],
): ListedBookmark {
  const url = `https://example.com/${slugify(title)}`;
  return {
    id,
    url,
    canonical_url: url,
    title,
    saved_at,
    state,
    user_tags: [],
    suggested_tags: tags,
    collections: [],
    author: null,
    site_name: null,
    description: null,
    published_at: null,
    note: null,
    action_prompt: null,
  };
}

export const FTS_GUIDE = other(
  "am_01JCDTF2W7Q9X4B8N1K5R3M6VA",
  "A field guide to SQLite full-text search",
  "2026-11-12T03:52:40.118904Z",
  ["sqlite", "search"],
  "inbox",
);
export const CLI_DESIGN = other(
  "am_01JC6M0R4H8ZP2W5T9KQ3N7XEB",
  "Designing CLIs for people and agents",
  "2026-11-07T21:06:12.550731Z",
  ["cli", "agents"],
  "processed",
);
export const BACKUPS = other(
  "am_01JBWE9K2Q5T8V1X4Z7C0N3M6P",
  "Backups you can actually restore",
  "2026-11-03T08:41:57.204336Z",
  ["backups", "local-first"],
  "archived",
);
export const DOCS = other(
  "am_01JBAT3N6Q9S2V5Y8B1D4F7H0K",
  "Writing docs that survive a rewrite",
  "2026-10-28T16:20:03.917452Z",
  ["docs", "markdown"],
  "processed",
);
