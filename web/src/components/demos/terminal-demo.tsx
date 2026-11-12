"use client";

import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { TerminalFrame, cx, useInView, usePrefersReducedMotion } from "./kit";
import { AGENT_SESSION, CLI_SESSION, typedText, type Block, type Span, type TermLine, type Tone } from "./terminal-script";
import { buildTimeline, partsOf, type Cursor } from "./terminal-timeline";

/**
 * For your agent: the same lookup two ways. "Agent" shows a coding agent
 * answering a plain question by running the CLI; "CLI" shows a person
 * running the same commands. Each session plays once when it comes into
 * view; switching tabs or Replay restarts it. The server render, and
 * reduced motion, show the finished session.
 */

type TabId = "agent" | "cli";

const SESSIONS: Record<TabId, { label: string; title: string; blocks: Block[] }> = {
  agent: { label: "Agent", title: "agent — ~/notes", blocks: AGENT_SESSION },
  cli: { label: "CLI", title: "zsh — ~/notes", blocks: CLI_SESSION },
};
const TABS: TabId[] = ["agent", "cli"];
const TIMELINES = { agent: buildTimeline(AGENT_SESSION), cli: buildTimeline(CLI_SESSION) };

/** Scroll panes clip the global focus ring (drawn outside, unlayered CSS), so draw it inside. */
const INSET_FOCUS = { outlineOffset: -2 } as const;

const TONE_CLASS: Record<Tone, string> = {
  ink: "text-term-ink",
  muted: "text-term-muted",
  accent: "text-term-accent",
  bold: "font-semibold text-term-ink",
};

/** `step: null` means settled: show the finished session. */
type Run = { tab: TabId; step: number | null };

export default function TerminalDemo() {
  const [run, setRun] = useState<Run>({ tab: "agent", step: null });
  const reduced = usePrefersReducedMotion();
  // Reset to an empty session just before it scrolls into view, then play once it is in view.
  const { ref: nearRef, inView: isNear } = useInView({ rootMargin: "240px 0px", once: true });
  const { ref: viewRef, inView } = useInView({ threshold: 0.4 });
  const armed = useRef(false);

  const timeline = TIMELINES[run.tab];
  const last = timeline.length - 1;

  useEffect(() => {
    if (!isNear || reduced || armed.current) return;
    const id = window.setTimeout(() => {
      armed.current = true;
      setRun((r) => (r.step === null ? { ...r, step: 0 } : r));
    }, 0);
    return () => clearTimeout(id);
  }, [isNear, reduced]);

  useEffect(() => {
    if (reduced || !inView || run.step === null || run.step >= last) return;
    const next = run.step + 1;
    const id = window.setTimeout(() => setRun({ tab: run.tab, step: next }), timeline[next].delay);
    return () => clearTimeout(id);
  }, [run, reduced, inView, last, timeline]);

  const start = (tab: TabId) => {
    armed.current = true;
    setRun({ tab, step: reduced ? null : 0 });
  };

  const settled = reduced || run.step === null;
  const cursor: Cursor = settled ? timeline[last].cursor : timeline[run.step ?? last].cursor;
  const running = !settled && run.step !== last;
  const session = SESSIONS[run.tab];

  // Follow the output like a terminal does, but once a command prints more
  // than fits, keep that command at the top so its first lines stay readable.
  const panelRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const panel = panelRef.current;
    if (!panel) return;
    const anchors = panel.querySelectorAll<HTMLElement>("[data-anchor]");
    const anchor = anchors[anchors.length - 1];
    const bottom = panel.scrollHeight - panel.clientHeight;
    const top = anchor ? Math.min(bottom, anchor.offsetTop - 2) : bottom;
    panel.scrollTo({ top, behavior: running ? "smooth" : "auto" });
  }, [run, reduced, running]);

  return (
    <div
      ref={nearRef}
      role="group"
      aria-label="A terminal showing how to look up a saved page, through an agent or by hand"
      className="h-[460px] w-full sm:h-[480px]"
    >
      <div ref={viewRef} className="h-full">
        <TerminalFrame title={session.title}>
          <div className="absolute inset-0 flex flex-col text-[12px] leading-[1.6] sm:text-[12.5px]">
            <TabBar tab={run.tab} onSelect={start} onReplay={() => start(run.tab)} />
            <div
              ref={panelRef}
              id="terminal-panel"
              role="tabpanel"
              aria-labelledby={`terminal-tab-${run.tab}`}
              aria-busy={running}
              tabIndex={0}
              style={INSET_FOCUS}
              className="relative min-h-0 flex-1 overflow-auto overscroll-contain"
            >
              <div className={cx("px-4 pt-3 pb-3.5", run.tab === "cli" ? "w-max min-w-full" : "w-full")}>
                {run.tab === "agent" ? (
                  <AgentTranscript blocks={session.blocks} cursor={cursor} />
                ) : (
                  <ShellTranscript blocks={session.blocks} cursor={cursor} />
                )}
              </div>
            </div>
          </div>
        </TerminalFrame>
      </div>
    </div>
  );
}

function TabBar({
  tab,
  onSelect,
  onReplay,
}: {
  tab: TabId;
  onSelect: (tab: TabId) => void;
  onReplay: () => void;
}) {
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const i = TABS.indexOf(tab);
    const n = TABS.length;
    const targets: Record<string, number> = { ArrowRight: (i + 1) % n, ArrowLeft: (i - 1 + n) % n, Home: 0, End: n - 1 };
    if (!(e.key in targets)) return;
    e.preventDefault();
    const target = TABS[targets[e.key]];
    if (target !== tab) onSelect(target);
    e.currentTarget.querySelector<HTMLElement>(`#terminal-tab-${target}`)?.focus();
  };

  return (
    <div className="flex shrink-0 items-center gap-2 border-b border-term-line px-2.5 py-1.5">
      <div role="tablist" aria-label="Session" className="flex gap-1" onKeyDown={onKeyDown}>
        {TABS.map((id) => {
          const selected = id === tab;
          return (
            <button
              key={id}
              id={`terminal-tab-${id}`}
              type="button"
              role="tab"
              aria-selected={selected}
              aria-controls="terminal-panel"
              tabIndex={selected ? 0 : -1}
              onClick={() => !selected && onSelect(id)}
              className={cx(
                "rounded-md px-2.5 py-1 text-[11.5px] transition-colors duration-150 motion-reduce:transition-none",
                selected ? "bg-term-line text-term-ink" : "text-term-muted hover:text-term-ink",
              )}
            >
              {SESSIONS[id].label}
            </button>
          );
        })}
      </div>
      <button
        type="button"
        onClick={onReplay}
        className="ml-auto flex items-center gap-1.5 rounded-md px-2 py-1 text-[11.5px] text-term-muted transition-colors duration-150 hover:text-term-ink motion-reduce:hidden"
      >
        <svg viewBox="0 0 16 16" className="size-3" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
          <path d="M3 8a5 5 0 1 0 1.6-3.7M3 2.5v3h3" />
        </svg>
        Replay
      </button>
    </div>
  );
}

/* ---------------------------------------------------------------- shell */

function ShellTranscript({ blocks, cursor }: { blocks: Block[]; cursor: Cursor }) {
  return (
    <div className="whitespace-pre">
      {blocks.map((block, i) => {
        if (i > cursor.block) return null;
        const part = i < cursor.block ? partsOf(block) : cursor.part;
        if (block.kind === "type") {
          return (
            <p key={i} data-anchor>
              <ShellPrompt />
              {typedText(block, part)}
              {i === cursor.block ? <Caret blink={part === 0} /> : null}
            </p>
          );
        }
        if (block.kind === "out") return <Lines key={i} lines={block.lines.slice(0, part * block.chunk)} />;
        return null;
      })}
      {cursor.block === blocks.length ? (
        <p>
          <ShellPrompt />
          <Caret blink />
        </p>
      ) : null}
    </div>
  );
}

function ShellPrompt() {
  return (
    <>
      <span className="text-term-accent">~/notes</span>
      <span className="text-term-muted"> $ </span>
    </>
  );
}

/* ---------------------------------------------------------------- agent */

function AgentTranscript({ blocks, cursor }: { blocks: Block[]; cursor: Cursor }) {
  return (
    <div className="space-y-2 leading-[1.55]">
      {blocks.map((block, i) => {
        if (i > cursor.block) return null;
        const active = i === cursor.block;
        const part = active ? cursor.part : partsOf(block);
        switch (block.kind) {
          case "type":
            return (
              <UserMessage key={i}>
                {typedText(block, part)}
                {active ? <Caret blink={part === 0} /> : null}
              </UserMessage>
            );
          case "tool":
            return <ToolCall key={i} block={block} done={part === 1} />;
          case "say":
            if (part === 0) return null;
            return block.quiet ? (
              <p key={i} className="flex items-center gap-2 text-term-muted">
                <span aria-hidden className="text-term-accent">◆</span>
                <Spans spans={block.sentences[0]} />
              </p>
            ) : (
              <p key={i} className="text-pretty whitespace-pre-wrap text-term-ink">
                {block.sentences.slice(0, part).map((sentence, j) => (
                  <span key={j}>
                    {j > 0 ? " " : null}
                    <Spans spans={sentence} />
                  </span>
                ))}
              </p>
            );
          case "out":
            return null;
        }
      })}
      {cursor.block === blocks.length ? (
        <p className="flex gap-2.5 px-3">
          <span aria-hidden className="text-term-accent">›</span>
          <Caret blink />
        </p>
      ) : null}
    </div>
  );
}

function UserMessage({ children }: { children: ReactNode }) {
  return (
    <p className="flex gap-2.5 rounded-md border border-term-line bg-term-chrome px-3 py-1.5 whitespace-pre-wrap text-term-ink">
      <span aria-hidden className="text-term-accent">›</span>
      <span className="min-w-0">{children}</span>
    </p>
  );
}

function ToolCall({ block, done }: { block: Extract<Block, { kind: "tool" }>; done: boolean }) {
  return (
    <div>
      <p className="flex gap-2.5">
        <span
          aria-hidden
          className={cx("w-3 shrink-0 text-center", done ? "text-term-ok" : "animate-pulse text-term-accent motion-reduce:animate-none")}
        >
          {done ? "✓" : "●"}
        </span>
        <span className="min-w-0 whitespace-pre-wrap text-term-ink [overflow-wrap:anywhere]">{block.command}</span>
      </p>
      {done ? (
        <div className="mt-0.5 ml-[5px] border-l border-term-line pl-[17px] text-[11.5px] sm:text-[11px]">
          {block.preview.map((line, i) => (
            <p key={i} className="truncate whitespace-pre">
              <Spans spans={line} />
            </p>
          ))}
          {block.hidden > 0 ? <p className="text-term-muted/70">… {block.hidden} more lines</p> : null}
        </div>
      ) : null}
    </div>
  );
}

/* --------------------------------------------------------------- shared */

function Lines({ lines }: { lines: TermLine[] }) {
  return lines.map((line, i) => (
    <p key={i}>
      {line.length === 1 && line[0].text === "" ? "\u200b" : <Spans spans={line} />}
    </p>
  ));
}

function Spans({ spans }: { spans: Span[] }) {
  return spans.map((s, i) => (
    <span key={i} className={TONE_CLASS[s.tone ?? "ink"]}>
      {s.text}
    </span>
  ));
}

/** Solid while typing, blinking while it waits. */
function Caret({ blink }: { blink: boolean }) {
  return (
    <span
      aria-hidden
      className={cx(
        "ml-px inline-block h-[1.1em] w-[0.6em] translate-y-[0.2em] bg-term-ink/75",
        blink && "animate-pulse motion-reduce:animate-none",
      )}
    />
  );
}
