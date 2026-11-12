import type { ReactNode } from "react";

/*
 * A small renderer for the privacy policy's markdown: headings (#, ##),
 * paragraphs, "- " lists, **bold**, `code` and [links](url). Enough for that
 * one file, so the site needs no markdown dependency. Text that starts with
 * "OWNER:" is a placeholder the owner still has to fill in; it renders as a
 * visible marker so it cannot be missed.
 */

export type Block =
  | { kind: "h1"; text: string }
  | { kind: "h2"; text: string }
  | { kind: "p"; text: string }
  | { kind: "ul"; items: string[] };

export const OWNER_MARK = "OWNER:";

export function parsePolicy(source: string): Block[] {
  const text = source.replace(/<!--[\s\S]*?-->/g, "");
  const blocks: Block[] = [];
  let para: string[] = [];
  let list: string[] | null = null;

  const flush = () => {
    if (para.length) blocks.push({ kind: "p", text: para.join(" ") });
    if (list) blocks.push({ kind: "ul", items: list });
    para = [];
    list = null;
  };

  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line) {
      flush();
    } else if (line.startsWith("## ")) {
      flush();
      blocks.push({ kind: "h2", text: line.slice(3) });
    } else if (line.startsWith("# ")) {
      flush();
      blocks.push({ kind: "h1", text: line.slice(2) });
    } else if (line.startsWith("- ")) {
      if (para.length) flush();
      list ??= [];
      list.push(line.slice(2));
    } else if (list) {
      list[list.length - 1] += ` ${line}`;
    } else {
      para.push(line);
    }
  }
  flush();
  return blocks;
}

/** Placeholders left in the text, for the draft note. */
export function ownerPlaceholders(blocks: Block[]): string[] {
  const all = blocks.flatMap((b) => (b.kind === "ul" ? b.items : [b.text]));
  return all.flatMap((t) => {
    const at = t.indexOf(OWNER_MARK);
    return at === -1 ? [] : [t.slice(at + OWNER_MARK.length).trim()];
  });
}

export function slugify(text: string) {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

const INLINE = /(\*\*[^*]+\*\*|`[^`]+`|\[[^\]]+\]\([^)]+\))/g;

/** Inline markup, with any "OWNER:" tail shown as a placeholder marker. */
export function Inline({ text }: { text: string }) {
  const at = text.indexOf(OWNER_MARK);
  const main = at === -1 ? text : text.slice(0, at);
  const owner = at === -1 ? null : text.slice(at + OWNER_MARK.length).trim();
  return (
    <>
      {renderInline(main)}
      {owner ? <Placeholder>{owner}</Placeholder> : null}
    </>
  );
}

function renderInline(text: string): ReactNode[] {
  return text.split(INLINE).map((part, i) => {
    if (part.startsWith("**") && part.endsWith("**")) {
      return (
        <strong key={i} className="font-medium text-ink">
          {part.slice(2, -2)}
        </strong>
      );
    }
    if (part.startsWith("`") && part.endsWith("`")) {
      return (
        <code key={i} className="whitespace-nowrap rounded-[5px] bg-code-bg px-[0.35em] py-[0.1em] font-mono text-[0.86em] text-ink">
          {part.slice(1, -1)}
        </code>
      );
    }
    const link = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(part);
    if (link) {
      return (
        <a key={i} href={link[2]} className="text-accent underline decoration-accent-line underline-offset-[3px] hover:decoration-accent">
          {link[1]}
        </a>
      );
    }
    return part;
  });
}

/** An unfilled owner placeholder. Deliberately loud: it must not ship unnoticed. */
function Placeholder({ children }: { children: ReactNode }) {
  return (
    <span className="mx-0.5 inline rounded-[5px] border border-dashed border-line-strong bg-sunken px-1.5 py-px font-mono text-[0.8em] text-muted [box-decoration-break:clone]">
      To fill in: {children}
    </span>
  );
}
