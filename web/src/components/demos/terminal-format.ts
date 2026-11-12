/*
 * TypeScript ports of the CLI's output formatting, so the terminal demo
 * prints exactly what `agentmark` prints. Keep in step with
 * packages/cli/src/display.rs (`format_list_row`, `format_show`).
 */

import type { BookmarkRecord } from "./files-bundle";

/** The CLI reads `COLUMNS` and falls back to 80. Shells do not export it, so 80 is what users and agents get. */
export const TERMINAL_WIDTH = 80;

/** A bookmark as list/search/show need it. */
export type ListedBookmark = Pick<
  BookmarkRecord,
  | "id"
  | "url"
  | "canonical_url"
  | "title"
  | "saved_at"
  | "state"
  | "user_tags"
  | "suggested_tags"
  | "collections"
  | "author"
  | "site_name"
  | "description"
  | "published_at"
  | "note"
  | "action_prompt"
>;

/** `display::truncate`: counts characters, the ellipsis takes the last slot. */
function truncate(s: string, max: number) {
  const chars = Array.from(s);
  if (max === 0) return "";
  if (chars.length <= max) return s;
  if (max === 1) return "…";
  return `${chars.slice(0, max - 1).join("")}…`;
}

function mergeTags(b: ListedBookmark) {
  return [...b.user_tags, ...b.suggested_tags.filter((t) => !b.user_tags.includes(t))];
}

/** `display::format_list_row`: `date  title  [tags]  state`, title truncated to fit. */
export function formatListRow(b: ListedBookmark, width = TERMINAL_WIDTH) {
  const date = b.saved_at.slice(0, 10);
  const merged = mergeTags(b);
  const tags = merged.length ? `[${merged.join(", ")}]` : "";
  const totalFixed = date.length + 2 + b.state.length + 2 + (tags ? tags.length + 2 : 0);
  const budget = width > totalFixed + 5 ? width - totalFixed : Math.min(60, Math.max(0, width - 4));
  const title = truncate(b.title, budget);
  return tags ? `${date}  ${title}  ${tags}  ${b.state}` : `${date}  ${title}  ${b.state}`;
}

export type ShowLine = { text: string; header?: boolean };

/**
 * `display::format_show`. Headers are bold when stdout is a terminal;
 * `header` marks them so the renderer can do the same.
 */
export function formatShow(
  b: ListedBookmark,
  { summary, article, full }: { summary: string; article: string; full: boolean },
): ShowLine[] {
  const out: ShowLine[] = [{ text: b.title, header: true }];
  const field = (label: string, value: string) => out.push({ text: `${label.padEnd(8)}${value}` });
  const section = (title: string, body: string) => {
    out.push({ text: "" }, { text: title, header: true });
    body.split("\n").forEach((text) => out.push({ text }));
  };

  field("ID:", b.id);
  field("URL:", b.url);
  if (b.canonical_url !== b.url) field("Canon:", b.canonical_url);
  field("Saved:", `${b.saved_at.slice(0, 10)} ${b.saved_at.slice(11, 19)} UTC`);
  field("State:", b.state);
  if (b.author) field("Author:", b.author);
  if (b.site_name) field("Site:", b.site_name);
  if (b.description) field("Desc:", b.description);
  if (b.published_at) field("Pub:", b.published_at);
  const tags = mergeTags(b);
  if (tags.length) field("Tags:", tags.join(", "));
  if (b.collections.length) field("Colls:", b.collections.join(", "));
  if (b.note) section("Note", b.note);
  if (b.action_prompt) section("Action", b.action_prompt);
  section("Summary", summary || "[enrichment pending]");

  out.push({ text: "" }, { text: "Article", header: true });
  const lines = article.replace(/\n$/, "").split("\n");
  if (full || lines.length <= 20) {
    lines.forEach((text) => out.push({ text }));
  } else {
    lines.slice(0, 20).forEach((text) => out.push({ text }));
    out.push({ text: "" }, { text: `... (${lines.length - 20} more lines, use --full to see all)` });
  }
  return out;
}
