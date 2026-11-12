/*
 * What the two terminal sessions say. Output comes from the formatting
 * ports in terminal-format.ts, run over a small fictional library whose
 * first entry is the shared sample bookmark.
 *
 * `list` and `search` rows carry no ID, so both sessions find it the way a
 * person would: the bundle folder name ends in the ID (see the On disk demo).
 */

import { ARTICLE_MD, SAMPLE_BOOKMARK, SAMPLE_DATE_DIR, bundleDirName } from "./files-bundle";
import { BACKUPS, CLI_DESIGN, DOCS, FTS_GUIDE } from "./files-library";
import { SAMPLE } from "./kit";
import { formatListRow, formatShow } from "./terminal-format";

const LISTED = [SAMPLE_BOOKMARK, FTS_GUIDE, CLI_DESIGN, BACKUPS, DOCS];
const SEARCH_HITS = [SAMPLE_BOOKMARK, DOCS];

const QUERY = "plain text";
const DATE_DIR = `~/Documents/AgentMark/${SAMPLE_DATE_DIR}`;
const LS_LINES = [FTS_GUIDE, SAMPLE_BOOKMARK].map(bundleDirName).sort();

const SHOW_INPUT = { summary: SAMPLE.summary, article: ARTICLE_MD };

/* --------------------------------------------------------------- blocks */

export type Tone = "ink" | "muted" | "accent" | "bold";
export type Span = { text: string; tone?: Tone };
export type TermLine = Span[];

/**
 * One step of a session. Each block reveals in parts: typed characters for
 * `type`, output chunks for `out` and `say`, running then done for `tool`.
 */
export type Block =
  /** A line the person types at a prompt; `paste` lands in one go, the way an ID gets copied in. */
  | { kind: "type"; prompt: "shell" | "agent"; text: string; paste?: string }
  /** Command output. `chunk` lines appear at a time. */
  | { kind: "out"; lines: TermLine[]; chunk: number }
  /** The agent runs a command; the panel shows a preview of its output. */
  | { kind: "tool"; command: string; preview: TermLine[]; hidden: number }
  /** Agent prose, revealed a sentence at a time. */
  | { kind: "say"; sentences: Span[][]; quiet?: boolean };

/** What a `type` block shows after `part` parts. */
export function typedText(block: Extract<Block, { kind: "type" }>, part: number) {
  return part > block.text.length ? block.text + (block.paste ?? "") : block.text.slice(0, part);
}

const plain = (text: string, tone: Tone = "muted"): TermLine => [{ text, tone }];

function showLines(full: boolean): TermLine[] {
  return formatShow(SAMPLE_BOOKMARK, { ...SHOW_INPUT, full }).map((l) =>
    plain(l.text, l.header ? "bold" : "muted"),
  );
}

function tool(command: string, output: TermLine[], keep: number): Block {
  return { kind: "tool", command, preview: output.slice(0, keep), hidden: Math.max(0, output.length - keep) };
}

const title = (text: string): Span => ({ text, tone: "bold" });
const text = (t: string): Span => ({ text: t, tone: "ink" });

export const AGENT_SESSION: Block[] = [
  { kind: "type", prompt: "agent", text: "what did I save about plain text formats?" },
  { kind: "say", quiet: true, sentences: [[{ text: "Using the agentmark skill", tone: "muted" }]] },
  tool(`agentmark search "${QUERY}"`, SEARCH_HITS.map((b) => plain(formatListRow(b))), 2),
  tool(`ls ${DATE_DIR}`, LS_LINES.map((l) => plain(l)), 2),
  tool(`agentmark show ${SAMPLE_BOOKMARK.id} --full`, showLines(true), 2),
  {
    kind: "say",
    sentences: [
      [text("The closest match is "), title(SAMPLE_BOOKMARK.title), text(", saved on 12 November.")],
      [
        text(
          "It argues that plain files outlast the apps that edit them: across three migrations, only the markdown export came through intact.",
        ),
      ],
      [title(DOCS.title), text(", from October, makes a similar case for docs.")],
    ],
  },
];

export const CLI_SESSION: Block[] = [
  { kind: "type", prompt: "shell", text: "agentmark list --limit 5" },
  { kind: "out", chunk: 1, lines: LISTED.map((b) => plain(formatListRow(b))) },
  { kind: "type", prompt: "shell", text: `agentmark search "${QUERY}"` },
  { kind: "out", chunk: 1, lines: SEARCH_HITS.map((b) => plain(formatListRow(b))) },
  { kind: "type", prompt: "shell", text: `ls ${DATE_DIR}` },
  { kind: "out", chunk: 2, lines: LS_LINES.map((l) => plain(l, "accent")) },
  { kind: "type", prompt: "shell", text: "agentmark show ", paste: SAMPLE_BOOKMARK.id },
  { kind: "out", chunk: 12, lines: showLines(false) },
];
