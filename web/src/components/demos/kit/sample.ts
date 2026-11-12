/*
 * Shared sample content for the demos, so every mock tells the same story.
 * All of it is fictional: example.com domains, invented people, no real
 * brands. Formats follow the real CLI (see packages/cli/src/display.rs and
 * packages/cli/src/bundle/).
 */

export const SAMPLE = {
  id: "am_01JCDX4Q8N3T7V2K9R6M5H1BZE",
  title: "Plain text outlives every app",
  url: "https://notes.example.com/plain-text-outlives-every-app",
  displayUrl: "notes.example.com/plain-text-outlives-every-app",
  site: "Field Notes",
  savedDate: "2026-11-12",
  savedAt: "2026-11-12 05:14:09 UTC",
  bundleDir: "~/bookmarks/2026/11/12/plain-text-outlives-every-app-am_01JCDX4Q8N3T7V2K9R6M5H1BZE",
  bundleDirShort: "2026/11/12/plain-text-outlives-every-app-am_01JC…",
  summary:
    "Argues that notes kept as plain files outlast the apps that edit them. Compares three migrations between note apps and shows that only the markdown export survived each one intact. Ends with a short checklist for picking formats that stay readable in ten years.",
  suggestedTags: ["plain-text", "markdown", "durability"],
  userTags: ["local-first"],
} as const;

/** Files in a bundle, in the order `ls` shows them. fetched.html only exists for some saves. */
export const BUNDLE_FILES = [
  { name: "article.md", note: "The page as clean markdown" },
  { name: "bookmark.md", note: "Front matter, summary, your note" },
  { name: "events.jsonl", note: "Saved, enriched, reprocessed" },
  { name: "metadata.json", note: "OpenGraph and page metadata" },
  { name: "source.html", note: "The page as captured" },
] as const;
