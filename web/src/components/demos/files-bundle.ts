/*
 * The bundle for the shared sample bookmark, file by file. Every file is
 * generated from one record so the id, URL, title, timestamps and tags
 * agree everywhere (the terminal demo reads the same record).
 *
 * Shapes follow the CLI: bookmark.md is `bundle/bookmark_md.rs` (serde_yaml
 * front matter in `models/bookmark.rs` field order, then four body
 * sections), metadata.json is `fetch::PageMetadata` pretty-printed,
 * events.jsonl is `models/event.rs` with `details` keys sorted the way
 * serde_json's map writes them.
 */

import { SAMPLE } from "./kit";

export type BookmarkState = "inbox" | "processed" | "archived";

/** The fields of `models::Bookmark`, in serialisation order. */
export type BookmarkRecord = {
  id: string;
  url: string;
  canonical_url: string;
  title: string;
  description: string | null;
  author: string | null;
  site_name: string | null;
  published_at: string | null;
  saved_at: string;
  capture_source: "cli" | "chrome_extension";
  user_tags: readonly string[];
  suggested_tags: readonly string[];
  collections: readonly string[];
  note: string | null;
  action_prompt: string | null;
  state: BookmarkState;
  content_status: "pending" | "extracted" | "failed";
  summary_status: "pending" | "done" | "failed";
  content_hash: string | null;
  schema_version: number;
};

export const SAMPLE_BOOKMARK: BookmarkRecord = {
  id: SAMPLE.id,
  url: SAMPLE.url,
  canonical_url: SAMPLE.url,
  title: SAMPLE.title,
  description: "Why the notes you keep as files outlast every app you write them in.",
  author: "Mara Okafor",
  site_name: SAMPLE.site,
  published_at: "2026-10-30T09:00:00+00:00",
  saved_at: "2026-11-12T05:14:09.482113Z",
  capture_source: "chrome_extension",
  user_tags: SAMPLE.userTags,
  suggested_tags: SAMPLE.suggestedTags,
  collections: [],
  note: "Settle the notes format for the team wiki.",
  action_prompt: null,
  state: "inbox",
  content_status: "extracted",
  summary_status: "done",
  content_hash: "sha256:7c1e0b94d2a6f35e8b40c7d19a2e6f03b5d8c41e97a0f26b3c5d8e1f4a7b9c02",
  schema_version: 1,
};

/** `Bookmark::slug` for ASCII titles: lowercase, non-alphanumeric runs become one hyphen. */
export function slugify(title: string) {
  return title
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

/** `<YYYY>/<MM>/<DD>` of a `saved_at`, as in the bundle path. */
export function dayDir(b: { saved_at: string }) {
  return b.saved_at.slice(0, 10).replaceAll("-", "/");
}

export const SAMPLE_DATE_DIR = dayDir(SAMPLE_BOOKMARK);
/** A bundle's folder name: `<slug>-<id>`. */
export function bundleDirName(b: { title: string; id: string }) {
  return `${slugify(b.title)}-${b.id}`;
}

export const SAMPLE_BUNDLE_DIR = bundleDirName(SAMPLE_BOOKMARK);

export const ARTICLE_MD = `Every note-taking app I have used promised to be the last one. None of them were. What lasted was the folder of plain files I kept on the side.

## Three migrations

Over twelve years I moved my notes between apps three times. Each move came with an export button, and each export lost something different.

- The first app exported rich text. Links survived, but every heading turned into bold body text.
- The second exported a database dump that only its own importer could read. The importer was retired a year later.
- The third exported markdown. It was the only export that came through all three moves intact.

The pattern was the same each time. The app was the fragile part. The text was fine.

## Why plain files last

A plain text file needs nothing but a text editor, and every computer has one. You can search it with \`grep\`, diff it, sync it with whatever you already use, and read it in fifty years without an emulator.

Markdown adds just enough structure: headings, lists, links. It degrades well. If the renderer disappears, the raw file still reads like a tidy note.

> The best format is the one you can read without the program that wrote it.

## A short checklist

Before you trust a tool with notes you want to keep, ask:

1. Can I see the files on disk, one note per file?
2. Can I open them in a different editor today?
3. Is the metadata in the file, or locked in a database next to it?
4. Could a script I write in an afternoon move everything?

If any answer is no, keep a plain copy alongside.

## What I do now

Each note is a markdown file in a dated folder. A small index makes them searchable, but the index is disposable: delete it and rebuild it from the files.

Apps come and go on top of that folder. The folder stays.
`;

/** `bookmark_md::render`: YAML front matter, then the four body sections. */
function renderBookmarkMd(b: BookmarkRecord, summary: string): string {
  const yaml = Object.entries(b)
    .map(([key, value]) => yamlField(key, value as BookmarkRecord[keyof BookmarkRecord]))
    .join("");
  return [
    `---\n${yaml}---\n`,
    `# Summary\n\n${summary}\n`,
    `# Why I Saved This\n\n${b.note ?? ""}\n`,
    "# Suggested Next Actions\n\n[pending enrichment]\n",
    "# Related Items\n\n[pending]\n",
  ].join("\n");
}

/** serde_yaml layout: block sequences, `[]` when empty, bare `null`. */
function yamlField(key: string, value: BookmarkRecord[keyof BookmarkRecord]): string {
  if (value === null) return `${key}: null\n`;
  if (typeof value === "string" || typeof value === "number") return `${key}: ${value}\n`;
  if (value.length === 0) return `${key}: []\n`;
  return `${key}:\n${value.map((v) => `- ${v}\n`).join("")}`;
}

export const BOOKMARK_MD = renderBookmarkMd(SAMPLE_BOOKMARK, SAMPLE.summary);

export const METADATA_JSON = JSON.stringify(
  {
    title: SAMPLE_BOOKMARK.title,
    description: SAMPLE_BOOKMARK.description,
    author: SAMPLE_BOOKMARK.author,
    site_name: SAMPLE_BOOKMARK.site_name,
    published_at: SAMPLE_BOOKMARK.published_at,
    canonical_url: SAMPLE_BOOKMARK.canonical_url,
    favicon_url: "https://notes.example.com/favicon.svg",
    hero_image_url: "https://notes.example.com/img/plain-text-outlives-every-app.jpg",
  },
  null,
  2,
);

export const EVENTS_JSONL = [
  {
    timestamp: "2026-11-12T05:14:09.483027Z",
    event_type: "saved",
    details: { capture_source: SAMPLE_BOOKMARK.capture_source, url: SAMPLE_BOOKMARK.url },
  },
  {
    timestamp: "2026-11-12T05:14:31.906214Z",
    event_type: "enriched",
    details: {
      article_length: ARTICLE_MD.length,
      provider: "agent",
      status: "success",
      suggested_collection: null,
      suggested_tags: SAMPLE_BOOKMARK.suggested_tags,
      summary_length: SAMPLE.summary.length,
    },
  },
]
  .map((event) => JSON.stringify(event))
  .join("\n")
  .concat("\n");

export const SOURCE_HTML = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${SAMPLE_BOOKMARK.title} | ${SAMPLE_BOOKMARK.site_name}</title>
  <meta name="description" content="${SAMPLE_BOOKMARK.description}">
  <meta name="author" content="${SAMPLE_BOOKMARK.author}">
  <link rel="canonical" href="${SAMPLE_BOOKMARK.canonical_url}">
  <link rel="icon" href="/favicon.svg">
  <meta property="og:site_name" content="${SAMPLE_BOOKMARK.site_name}">
  <meta property="og:title" content="${SAMPLE_BOOKMARK.title}">
  <meta property="og:image" content="https://notes.example.com/img/plain-text-outlives-every-app.jpg">
  <meta property="article:published_time" content="${SAMPLE_BOOKMARK.published_at}">
  <link rel="stylesheet" href="/assets/site.3f9a1c.css">
  <script defer src="/assets/app.81d2e4.js"></script>
</head>
<body class="post">
  <header class="site-header">
    <a class="logo" href="/">${SAMPLE_BOOKMARK.site_name}</a>
    <nav><a href="/archive">Archive</a> <a href="/about">About</a></nav>
  </header>
  <main>
    <article>
      <h1>${SAMPLE_BOOKMARK.title}</h1>
      <p class="byline">By ${SAMPLE_BOOKMARK.author} · <time datetime="2026-10-30">30 October 2026</time></p>
      <p>Every note-taking app I have used promised to be the last one. None of them were.</p>
      <h2>Three migrations</h2>
      <p>Over twelve years I moved my notes between apps three times.</p>
      <!-- … -->
    </article>
    <aside class="newsletter">
      <form action="/subscribe" method="post">…</form>
    </aside>
  </main>
  <footer class="site-footer">© 2026 ${SAMPLE_BOOKMARK.site_name}</footer>
</body>
</html>
`;

export type BundleFileName = "article.md" | "bookmark.md" | "events.jsonl" | "metadata.json" | "source.html";
export type FileLanguage = "markdown" | "json" | "jsonl" | "html";

export type BundleFile = {
  name: BundleFileName;
  language: FileLanguage;
  caption: string;
  content: string;
};

/** In `ls` order, as the bundle directory lists them. */
export const BUNDLE: readonly BundleFile[] = [
  {
    name: "article.md",
    language: "markdown",
    caption: "Clean markdown of the page, for your agent.",
    content: ARTICLE_MD,
  },
  {
    name: "bookmark.md",
    language: "markdown",
    caption: "Front matter, your note and the summary.",
    content: BOOKMARK_MD,
  },
  {
    name: "events.jsonl",
    language: "jsonl",
    caption: "One event per line: saved, then enriched.",
    content: EVENTS_JSONL,
  },
  {
    name: "metadata.json",
    language: "json",
    caption: "Title, author and images from the page.",
    content: METADATA_JSON,
  },
  {
    name: "source.html",
    language: "html",
    caption: "The page's HTML, exactly as captured.",
    content: SOURCE_HTML,
  },
];
