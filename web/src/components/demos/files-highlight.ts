/*
 * Just enough syntax colouring for the four bundle file types. Each
 * highlighter turns a file into lines of tokens; the viewer maps tones to
 * theme tokens. Not a general parser: it only has to handle the sample
 * files in files-bundle.ts well.
 */

import type { FileLanguage } from "./files-bundle";

export type Tone =
  | "plain"
  | "prose"
  | "strong"
  | "key"
  | "string"
  | "literal"
  | "punct"
  | "mark"
  | "dim"
  | "quote";

export type Token = { text: string; tone: Tone };
/** Prose soft-wraps like in an editor; code and front matter scroll sideways. */
export type HighlightedLine = { tokens: Token[]; wrap: boolean };

export function highlight(content: string, language: FileLanguage): HighlightedLine[] {
  const lines = content.replace(/\n$/, "").split("\n");
  switch (language) {
    case "markdown":
      return highlightMarkdown(lines);
    case "json":
    case "jsonl":
      return lines.map((line) => ({ tokens: highlightJsonLine(line), wrap: false }));
    case "html":
      return lines.map((line) => ({ tokens: highlightHtmlLine(line), wrap: false }));
  }
}

/* ------------------------------------------------------------ markdown */

function highlightMarkdown(lines: string[]): HighlightedLine[] {
  let inFrontMatter = lines[0] === "---";
  return lines.map((line, i) => {
    if (line === "---") {
      if (i > 0) inFrontMatter = false;
      return { tokens: [{ text: line, tone: "punct" }], wrap: false };
    }
    if (inFrontMatter) return { tokens: yamlLine(line), wrap: false };
    return { tokens: markdownLine(line), wrap: true };
  });
}

function yamlLine(line: string): Token[] {
  const item = /^- (.*)$/.exec(line);
  if (item) return [{ text: "- ", tone: "punct" }, { text: item[1], tone: "string" }];
  const field = /^([a-z_]+):(?: (.*))?$/.exec(line);
  if (!field) return [{ text: line, tone: "plain" }];
  const [, key, value] = field;
  const tokens: Token[] = [
    { text: key, tone: "key" },
    { text: ":", tone: "punct" },
  ];
  if (value !== undefined) {
    const literal = value === "null" || value === "[]" || /^\d+$/.test(value);
    tokens.push({ text: " ", tone: "plain" }, { text: value, tone: literal ? "literal" : "string" });
  }
  return tokens;
}

function markdownLine(line: string): Token[] {
  const heading = /^(#{1,6} )(.*)$/.exec(line);
  if (heading) return [{ text: heading[1], tone: "mark" }, { text: heading[2], tone: "strong" }];
  if (/^\[pending( enrichment)?\]$/.test(line)) return [{ text: line, tone: "dim" }];
  if (line.startsWith("> ")) return [{ text: "> ", tone: "mark" }, { text: line.slice(2), tone: "quote" }];
  const item = /^(- |\d+\. )(.*)$/.exec(line);
  if (item) return [{ text: item[1], tone: "mark" }, ...inlineMarkdown(item[2])];
  return inlineMarkdown(line);
}

/** Code spans are the only inline markup the sample uses. */
function inlineMarkdown(text: string): Token[] {
  return text
    .split(/(`[^`]+`)/)
    .filter(Boolean)
    .map((part) => ({ text: part, tone: part.startsWith("`") ? "key" : "prose" }));
}

/* ---------------------------------------------------------------- json */

const JSON_TOKEN = /("(?:[^"\\]|\\.)*")(\s*:)?|(-?\d+(?:\.\d+)?|true|false|null)|([{}[\],:])|(\s+)/g;

function highlightJsonLine(line: string): Token[] {
  const tokens: Token[] = [];
  for (const m of line.matchAll(JSON_TOKEN)) {
    const [, str, colon, literal, punct, space] = m;
    if (str !== undefined) {
      tokens.push({ text: str, tone: colon ? "key" : "string" });
      if (colon) tokens.push({ text: colon, tone: "punct" });
    } else if (literal !== undefined) tokens.push({ text: literal, tone: "literal" });
    else if (punct !== undefined) tokens.push({ text: punct, tone: "punct" });
    else if (space !== undefined) tokens.push({ text: space, tone: "plain" });
  }
  return tokens;
}

/* ---------------------------------------------------------------- html */

const HTML_TAG = /(<!--.*?-->|<[^>]+>)/;
const TAG_PART = /^(<\/?!?)([\w-]+)(.*?)(\/?>)$/;
const ATTR = /(\s+)([\w:-]+)(?:(=)("[^"]*"))?/g;

/** HTML is shown dimmed: it is kept for the record, not for reading. */
function highlightHtmlLine(line: string): Token[] {
  const tokens: Token[] = [];
  for (const part of line.split(HTML_TAG).filter(Boolean)) {
    if (part.startsWith("<!--")) {
      tokens.push({ text: part, tone: "dim" });
      continue;
    }
    const tag = TAG_PART.exec(part);
    if (!tag) {
      tokens.push({ text: part, tone: "prose" });
      continue;
    }
    const [, open, name, attrs, close] = tag;
    tokens.push({ text: open, tone: "punct" }, { text: name, tone: "literal" });
    for (const [, space, attr, eq, value] of attrs.matchAll(ATTR)) {
      tokens.push({ text: space, tone: "plain" }, { text: attr, tone: "dim" });
      if (eq) tokens.push({ text: eq, tone: "punct" }, { text: value, tone: "dim" });
    }
    tokens.push({ text: close, tone: "punct" });
  }
  return tokens;
}
