"use client";

import { useCallback, useRef, useState, type ReactNode } from "react";
import { METHOD_SUBLINE, cliCommand, installMethod, orderPlatforms, platformByKey, type Platform, type PlatformKey } from "@/lib/platforms";
import { AgentMarkMark, BrowserFrame, SAMPLE, cx, useInView, usePrefersReducedMotion, type MarkLook } from "./kit";
import { MockArticle } from "./kit/mock-article";
import { BrowserCursor, CURSOR_HIDDEN, pointAt, type CursorState } from "./browser-cursor";
import { useStory } from "./browser-story";
import { LiveRegion, ReplayButton } from "./browser-ui";

/*
 * First run, from the toolbar click to the first save. A replica of the
 * extension's own screens (packages/extension/src/shared/AppMissing.tsx,
 * SetupForm.tsx, settings-controls.tsx, CopyCommand.tsx and the popup's save
 * view), with the same wording. Not an import: web/ stays self-contained.
 *
 * Beats, in ms from the start of a run:
 * 0 idle · 1 pointer to the icon · 2 press · 3 "Finish setting up", this
 *   computer first · 4 pointer to it · 5 press · 6 "Install on …" · 7 pointer
 *   to Copy (or Download) · 8 press · 9 copied · 10 Terminal takes focus, so
 *   the popup closes, as Chrome popups do · 11 installer done · 12 pointer
 *   back to the icon · 13 press · 14 the app answers: "Set up AgentMark" ·
 *   15 scroll to the button · 16 pointer to it · 17 press · 18 "Saving…" on
 *   the button · 19 popup "Saving..." · 20 "Saved!" with a tick · 21 the
 *   tick's hold ends.
 */
const TIMES = [
  0, 600, 1150, 1300, 2900, 3450, 3600, 4700, 5250, 5400, 6300, 7500, 9100, 9650, 9800, 11400, 12300, 12850, 13000,
  13800, 14800, 19800,
] as const;

const B = {
  open: 3,
  toRow: 4,
  pressRow: 5,
  install: 6,
  toCopy: 7,
  pressCopy: 8,
  copied: 9,
  terminal: 10,
  installed: 11,
  toIcon: 12,
  pressIcon: 13,
  setup: 14,
  scroll: 15,
  toSubmit: 16,
  pressSubmit: 17,
  submitting: 18,
  saving: 19,
  saved: 20,
} as const;

/** The step buttons under the frame: each is current from `from`, and shows `beat` when chosen. */
const STEPS = [
  { label: "Not found", from: B.open, beat: B.open },
  { label: "Install", from: B.install, beat: B.copied },
  { label: "Run it", from: B.terminal, beat: B.installed },
  { label: "Set up", from: B.setup, beat: B.setup },
  { label: "Saved", from: B.saving, beat: TIMES.length - 1 },
] as const;

/** The demo's computer. A real popup puts the visitor's own platform here. */
const DETECTED: PlatformKey = "mac_arm64";
const PLATFORM_INFO = { os: "mac", arch: "arm64" };

/**
 * The copy loaded from `~/.agentmark/extension` carries the store's key, so it
 * runs under the store ID and shows the short command (contract §7.2).
 */
const COMMAND = cliCommand();

const APP_VERSION = "0.3.1";
const FOLDER = "~/Documents/AgentMark";
const AGENTS = [
  { id: "claude", name: "Claude Code", found: true },
  { id: "codex", name: "Codex", found: true },
  { id: "pi", name: "pi", found: false },
] as const;
const SUMMARIES_LABEL = "Write a summary and tags after each save";
const SUMMARIES_NOTE =
  "Sends the page text to your agent's provider. Your agent also turns LinkedIn, X, Hacker News and Reddit pages into readable text.";

const TERMINAL_LINES = [
  { text: "✓ Binary installed (~/.agentmark/bin/agentmark)", tone: "ok" },
  { text: "✓ Native messaging host registered", tone: "ok" },
  { text: "✓ Agent skill installed", tone: "ok" },
  { text: "Done. Go back to Chrome: AgentMark finishes setting up there.", tone: "done" },
] as const;

type Screen = "closed" | "missing" | "install" | "setup" | "saving" | "saved";

function screenAt(beat: number): Screen {
  if (beat < B.open) return "closed";
  if (beat < B.install) return "missing";
  if (beat < B.terminal) return "install";
  if (beat < B.setup) return "closed";
  if (beat < B.saving) return "setup";
  return beat < B.saved ? "saving" : "saved";
}

function iconAt(beat: number, last: number): MarkLook {
  if (beat < B.saving) return "idle";
  if (beat < B.saved) return "saving";
  return beat < last ? "saved-fresh" : "saved";
}

const MESSAGES: Partial<Record<number, string>> = {
  [B.open]: "Finish setting up AgentMark. Your computer, Mac Apple Silicon, is listed first.",
  [B.install]: "Install on Mac Apple Silicon. Open Terminal and paste the command.",
  [B.installed]: "The installer finished in Terminal. Back in Chrome, click the AgentMark icon again.",
  [B.setup]: "Set up AgentMark: choose a folder and an agent.",
  [B.saved]: "Saved.",
};

export default function FirstRunDemo() {
  const { ref, inView } = useInView<HTMLDivElement>({ threshold: 0.4 });
  const reduced = usePrefersReducedMotion();
  const iconRef = useRef<HTMLSpanElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const [cursor, setCursor] = useState<CursorState>(CURSOR_HIDDEN);
  const [message, setMessage] = useState("");
  /** A step the visitor chose; it holds until they pick another or replay. */
  const [pinned, setPinned] = useState<number | null>(null);
  /** True while the popup has more of its screen below the fold. */
  const [more, setMore] = useState(false);
  const measureMore = useCallback(() => {
    const body = bodyRef.current;
    const next = !!body && body.scrollTop + body.clientHeight < body.scrollHeight - 2;
    setMore((prev) => (prev === next ? prev : next));
  }, []);
  // Each screen mounts a fresh scroll body; watch its size and its content's.
  const attachBody = useCallback(
    (el: HTMLDivElement | null) => {
      bodyRef.current = el;
      if (!el || typeof ResizeObserver === "undefined") return;
      const ro = new ResizeObserver(measureMore);
      ro.observe(el);
      if (el.firstElementChild) ro.observe(el.firstElementChild);
      return () => ro.disconnect();
    },
    [measureMore],
  );
  const method = installMethod(DETECTED);
  const download = method === "app_download";

  const cursorTo = (target: Element | null | undefined) => {
    const root = ref.current;
    if (root && target) setCursor({ ...pointAt(root, target), visible: true, pressed: false });
  };
  const find = (name: string) => ref.current?.querySelector(`[data-demo="${name}"]`);

  const story = useStory({
    times: TIMES,
    inView,
    reduced,
    onBeat: (b) => {
      const root = ref.current;
      const icon = iconRef.current;
      if (b === 0 && root && icon) {
        const target = pointAt(root, icon);
        setCursor({ x: target.x - 180, y: target.y + 160, visible: false, pressed: false });
      }
      if (b === 1) cursorTo(iconRef.current);
      if (b === 2 || b === B.pressRow || b === B.pressCopy || b === B.pressIcon || b === B.pressSubmit)
        setCursor((c) => ({ ...c, pressed: true }));
      if (b === B.open || b === B.setup) setCursor((c) => ({ ...c, pressed: false, visible: false }));
      if (b === B.toIcon) cursorTo(iconRef.current);
      if (b === B.toRow) cursorTo(find("detected"));
      if (b === B.install) setCursor((c) => ({ ...c, pressed: false }));
      if (b === B.toCopy) cursorTo(find(download ? "download" : "copy"));
      if (b === B.copied || b === B.submitting) setCursor((c) => ({ ...c, pressed: false, visible: false }));
      if (b === B.scroll) {
        const body = bodyRef.current;
        body?.scrollTo({ top: body.scrollHeight, behavior: "smooth" });
      }
      if (b === B.toSubmit) cursorTo(find("submit"));
      if (b === TIMES.length - 1) setCursor(CURSOR_HIDDEN);
      const m = MESSAGES[b];
      if (m) setMessage(m);
    },
  });

  const beat = pinned ?? story.beat;
  const last = story.last;
  const screen = screenAt(beat);
  const look = iconAt(beat, last);
  const terminalLines = beat === B.terminal ? 0 : beat >= B.installed && beat < B.setup ? TERMINAL_LINES.length : -1;
  const showTerminal = !download && terminalLines >= 0;
  const settled = pinned !== null || beat === last;
  const activeStep = STEPS.reduce((at, s, i) => (beat >= s.from ? i : at), -1);

  const pick = (i: number) => {
    if (story.running) story.settle();
    setCursor(CURSOR_HIDDEN);
    const b = STEPS[i].beat;
    setPinned(b === last ? null : b);
    setMessage(MESSAGES[b] ?? STEPS[i].label);
  };

  const replay = () => {
    setPinned(null);
    story.play(0);
  };

  return (
    <div
      ref={ref}
      className="@container relative flex h-[500px] w-full flex-col gap-3 sm:h-[560px]"
      role="group"
      aria-label="Demo: the first click on the AgentMark icon, from installing the app to the first saved page"
    >
      <div className="relative min-h-0 flex-1">
        <BrowserFrame
          url={SAMPLE.displayUrl}
          tabTitle={SAMPLE.title}
          extension={
            <span ref={iconRef} className="grid size-7 place-items-center rounded-md" aria-hidden>
              <AgentMarkMark look={look} size={16} />
            </span>
          }
        >
          <div aria-hidden className="h-full">
            <MockArticle compact />
          </div>
          <MiniTerminal visible={showTerminal} lines={terminalLines} />
          <MiniApp visible={download && terminalLines >= 0} done={terminalLines > 0} />
          <Popup open={screen !== "closed"} more={more}>
            <div
              key={screen === "saving" || screen === "saved" ? "save" : screen}
              ref={attachBody}
              onScroll={measureMore}
              className="max-h-full overflow-y-auto overscroll-contain p-3.5 [scrollbar-width:none] transition-opacity duration-300 ease-out starting:opacity-0 motion-reduce:transition-none @min-[400px]:p-4"
            >
              {screen === "missing" && <MissingScreen />}
              {screen === "install" && (
                <InstallScreen platform={platformByKey(DETECTED)} copied={beat >= B.copied} download={download} />
              )}
              {screen === "setup" && <SetupScreen submitting={beat === B.submitting} />}
              {(screen === "saving" || screen === "saved") && <SaveScreen saved={screen === "saved"} />}
            </div>
          </Popup>
        </BrowserFrame>
        {settled ? (
          <ReplayButton onClick={replay} label="Replay the first-run demo" className="absolute right-2.5 top-1.5 z-20" />
        ) : null}
      </div>

      <ol className="flex shrink-0 gap-1" aria-label="Steps of the first run">
        {STEPS.map((s, i) => {
          const active = i === activeStep;
          return (
            <li key={s.label} className="min-w-0 flex-1">
              <button
                type="button"
                onClick={() => pick(i)}
                aria-current={active ? "step" : undefined}
                className={cx(
                  "flex h-9 w-full min-w-0 flex-col justify-center gap-1 rounded-md px-1 text-left transition-colors",
                  active ? "text-ink" : "text-subtle hover:text-ink",
                )}
              >
                <span
                  aria-hidden
                  className={cx(
                    "h-0.5 w-full rounded-full transition-colors duration-300 motion-reduce:transition-none",
                    i <= activeStep ? "bg-ink" : "bg-line-strong",
                  )}
                />
                <span className="truncate text-[11.5px] font-medium">
                  <span className="mr-1 hidden font-mono text-[10.5px] text-subtle @min-[420px]:inline">{i + 1}</span>
                  {s.label}
                </span>
              </button>
            </li>
          );
        })}
      </ol>

      <BrowserCursor state={cursor} />
      <LiveRegion message={message} />
    </div>
  );
}

/* ----------------------------------------------------------- Popup shell */

function Popup({ open, more, children }: { open: boolean; more: boolean; children: ReactNode }) {
  return (
    <div className="pointer-events-none absolute inset-y-1.5 right-2 z-10 flex w-[min(300px,calc(100%-16px))] items-start sm:right-3 @min-[400px]:w-[304px] @min-[560px]:w-[328px]">
      <div
        className={cx(
          "flex max-h-full w-full origin-top-right flex-col overflow-hidden rounded-lg border border-line-strong bg-surface shadow-frame",
          "transition-[opacity,transform] duration-200 ease-out motion-reduce:transition-none",
          open ? "translate-y-0 scale-100 opacity-100" : "-translate-y-1 scale-[0.98] opacity-0",
        )}
        aria-hidden={!open}
      >
        <div className="relative flex min-h-0 flex-col">
          {children}
          {/* More of the screen below: fade the edge, as a scrolling popup does. */}
          <div
            aria-hidden
            className={cx(
              "pointer-events-none absolute inset-x-0 bottom-0 h-8 bg-linear-to-t from-surface to-transparent transition-opacity duration-200 motion-reduce:transition-none",
              more ? "opacity-100" : "opacity-0",
            )}
          />
        </div>
      </div>
    </div>
  );
}

/* --------------------------------------------- Screen A: app not found */

function Heading({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-2">
      <AgentMarkMark look="idle-solid" size={18} />
      <p className="text-[14.5px] font-semibold tracking-tight text-ink">{children}</p>
    </div>
  );
}

function MissingScreen() {
  const { detected, others } = orderPlatforms(PLATFORM_INFO);
  return (
    <div>
      <Heading>Finish setting up AgentMark</Heading>
      <p className="mt-1.5 text-[12.5px] leading-snug text-muted">
        The extension saves pages through the AgentMark app on this computer.
      </p>
      {detected ? (
        <div className="mt-3">
          <PlatformRow platform={detected} detected />
        </div>
      ) : null}
      <p className="mb-1.5 mt-3 text-[11.5px] font-medium text-subtle">Other platforms</p>
      <ul className="space-y-1.5">
        {others.map((p) => (
          <li key={p.key}>
            <PlatformRow platform={p} />
          </li>
        ))}
      </ul>
      <p className="mt-3 border-t border-line pt-2.5 text-[11.5px] text-subtle">
        Already installed? <span className="font-medium text-accent">Check again</span>
      </p>
    </div>
  );
}

function PlatformRow({ platform, detected = false }: { platform: Platform; detected?: boolean }) {
  const subline = METHOD_SUBLINE[installMethod(platform.key)];
  return (
    <div
      data-demo={detected ? "detected" : undefined}
      className={cx(
        "flex items-center gap-2.5 rounded-lg border px-2.5 py-1.5",
        detected ? "border-accent bg-accent-soft" : "border-line bg-surface",
      )}
    >
      <PlatformGlyph platformKey={platform.key} />
      <span className="min-w-0 flex-1">
        <span className="block text-[12.5px] font-semibold text-ink">{platform.name}</span>
        <span className="block text-[11.5px] leading-snug text-subtle">
          {detected ? `Your computer · ${subline}` : subline}
        </span>
      </span>
      <svg viewBox="0 0 16 16" className={cx("size-3.5 shrink-0", detected ? "text-accent" : "text-subtle")} aria-hidden>
        <path d="M6 3.5 10.5 8 6 12.5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </div>
  );
}

/** The extension's neutral line glyphs (AppMissing.tsx), not vendor logos. */
function PlatformGlyph({ platformKey }: { platformKey: PlatformKey }) {
  const stroke = { fill: "none", stroke: "currentColor", strokeWidth: 1.4, strokeLinecap: "round", strokeLinejoin: "round" } as const;
  let body: ReactNode;
  if (platformKey === "linux") {
    body = (
      <>
        <rect x="3" y="4.5" width="14" height="11" rx="1.5" {...stroke} />
        <path d="m6 8.5 2 1.75-2 1.75M10 12.25h3.5" {...stroke} />
      </>
    );
  } else if (platformKey === "windows") {
    body = (
      <>
        <rect x="3" y="4" width="14" height="9.5" rx="1.2" {...stroke} />
        <path d="M8 16.5h4M10 13.5v3" {...stroke} />
      </>
    );
  } else {
    body = (
      <>
        <rect x="4" y="5" width="12" height="8.5" rx="1.2" {...stroke} />
        <path d="M2.5 15.5h15" {...stroke} />
      </>
    );
  }
  return (
    <span className="flex size-7 shrink-0 items-center justify-center rounded-md border border-line bg-surface text-subtle">
      <svg viewBox="0 0 20 20" width="16" height="16" aria-hidden>
        {body}
      </svg>
    </span>
  );
}

/* -------------------------------------------- Screen B: how to install */

function Step({ n, children }: { n: number; children: ReactNode }) {
  return (
    <li className="flex min-w-0 gap-2">
      <span
        aria-hidden
        className="mt-px flex size-[18px] shrink-0 items-center justify-center rounded-full bg-accent-soft text-[10.5px] font-semibold text-accent"
      >
        {n}
      </span>
      <div className="min-w-0 flex-1 text-[12.5px] leading-snug text-ink">{children}</div>
    </li>
  );
}

function InstallScreen({ platform, copied, download }: { platform: Platform; copied: boolean; download: boolean }) {
  return (
    <div>
      <p className="-ml-0.5 inline-flex items-center gap-0.5 text-[11.5px] font-medium text-accent">
        <svg viewBox="0 0 16 16" className="size-3" aria-hidden>
          <path d="M10 3.5 5.5 8l4.5 4.5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        All platforms
      </p>
      <p className="mt-1.5 text-[14.5px] font-semibold tracking-tight text-ink">Install on {platform.name}</p>
      {download ? (
        <>
          <span
            data-demo="download"
            className="mt-3 flex w-full items-center justify-center rounded-lg bg-accent-solid px-4 py-2 text-[13px] font-semibold text-accent-solid-ink"
          >
            Download AgentMark for Mac
          </span>
          <p className="mt-2 text-[11.5px] text-subtle">Open the app, then come back here.</p>
          <p className="mt-3 text-[11.5px] font-medium text-muted">Or use Terminal instead:</p>
          <CommandBlock command={cliCommand()} copied={false} />
        </>
      ) : (
        <ol className="mt-3 space-y-3">
          <Step n={1}>
            Open <strong className="font-semibold">Terminal</strong> and paste this:
            <CommandBlock command={COMMAND} copied={copied} />
          </Step>
          <Step n={2}>Come back here. This updates by itself.</Step>
        </ol>
      )}
      <div className="mt-4 flex items-center gap-2 rounded-lg border border-dashed border-accent-line bg-accent-soft/50 px-3 py-2">
        <span className="relative flex size-2 shrink-0" aria-hidden>
          <span className="absolute inline-flex size-full rounded-full bg-accent opacity-50 motion-safe:animate-ping" />
          <span className="relative inline-flex size-2 rounded-full bg-accent" />
        </span>
        <span className="flex-1 text-[12.5px] text-accent">Waiting for the app…</span>
        <span className="px-1 text-[11.5px] font-medium text-accent">Check again</span>
      </div>
      <p className="mt-3 text-right text-[10.5px] text-subtle">Setup guide</p>
    </div>
  );
}

/** Splits a command after each "/" and space, like the extension's command-pieces.ts. */
function commandPieces(command: string): string[] {
  return command.match(/[^/\s]*(?:\/+|\s+|$)/g)?.filter((piece) => piece !== "") ?? [command];
}

function CommandBlock({ command, copied }: { command: string; copied: boolean }) {
  return (
    <div className="mt-1.5 flex min-w-0 items-start gap-2 rounded-lg border border-line bg-sunken py-2 pl-2.5 pr-1.5">
      <code className="min-w-0 flex-1 whitespace-pre-wrap font-mono text-[10.5px] leading-[1.45] text-ink [overflow-wrap:anywhere]">
        {commandPieces(command).map((piece, i) => (
          <span key={i}>
            {i > 0 && <wbr />}
            {piece}
          </span>
        ))}
      </code>
      <span
        data-demo="copy"
        className="grid shrink-0 rounded-md border border-line bg-surface px-2 py-0.5 text-[10.5px] font-medium text-muted shadow-sm *:col-start-1 *:row-start-1"
      >
        {/* Both labels share one cell, so the button keeps its width. */}
        <span className={copied ? "invisible" : undefined}>Copy</span>
        <span className={copied ? undefined : "invisible"}>Copied</span>
      </span>
    </div>
  );
}

/* ----------------------------------------------------- Screen C: setup */

function SetupScreen({ submitting }: { submitting: boolean }) {
  return (
    <div>
      <Heading>Set up AgentMark</Heading>
      <p className="mt-1.5 text-[12.5px] leading-snug text-muted">
        The app is installed ({APP_VERSION}). Two choices and you&apos;re done.
      </p>

      <p className="mt-3.5 text-[12.5px] font-semibold text-ink">Where should bookmarks live?</p>
      <p className="mb-1.5 text-[11.5px] text-subtle">A normal folder. One sub-folder per saved page.</p>
      <div className="flex gap-1.5">
        <span className="block min-w-0 flex-1 truncate rounded-lg border border-line-strong bg-surface px-2.5 py-1.5 font-mono text-[11.5px] text-ink">
          {FOLDER}
        </span>
        <span className="shrink-0 rounded-lg border border-line-strong bg-surface px-2.5 py-1.5 text-[12px] text-muted shadow-sm">
          Choose…
        </span>
      </div>

      <p className="mt-3.5 text-[12.5px] font-semibold text-ink">Which agent writes summaries?</p>
      <p className="mb-1.5 text-[11.5px] text-subtle">AgentMark uses one you already have.</p>
      <div className="space-y-1.5">
        {AGENTS.map((agent, i) => {
          const selected = i === 0;
          return (
            <div
              key={agent.id}
              className={cx(
                "flex items-center gap-2.5 rounded-lg border px-3 py-1.5",
                selected ? "border-accent bg-accent-soft" : "border-line bg-surface",
              )}
            >
              <span
                aria-hidden
                className={cx(
                  "grid size-3.5 shrink-0 place-items-center rounded-full border",
                  selected ? "border-accent-solid" : "border-line-strong",
                )}
              >
                {selected ? <span className="size-1.5 rounded-full bg-accent-solid" /> : null}
              </span>
              <span className="min-w-0 flex-1 truncate text-[12.5px] text-ink">{agent.name}</span>
              {agent.found ? (
                <span className="shrink-0 rounded bg-ok-soft px-1.5 py-0.5 text-[10.5px] font-medium text-ok">Found</span>
              ) : (
                <span className="shrink-0 rounded bg-sunken px-1.5 py-0.5 text-[10.5px] font-medium text-subtle">
                  Not installed
                </span>
              )}
            </div>
          );
        })}
      </div>

      <div className="mt-3.5 flex items-start gap-2.5">
        <span aria-hidden className="relative mt-0.5 inline-flex h-5 w-9 shrink-0 items-center rounded-full bg-accent-solid">
          <span className="inline-block size-4 translate-x-[18px] rounded-full bg-white shadow" />
        </span>
        <span className="min-w-0">
          <span className="block text-[12.5px] font-semibold leading-snug text-ink">{SUMMARIES_LABEL}</span>
          <span className="mt-0.5 block text-[11.5px] leading-snug text-subtle">{SUMMARIES_NOTE}</span>
        </span>
      </div>

      <span
        data-demo="submit"
        className="mt-3.5 flex w-full items-center justify-center rounded-lg bg-accent-solid px-4 py-2 text-[13px] font-semibold text-accent-solid-ink shadow-sm"
      >
        {submitting ? "Saving…" : "Finish and save this page"}
      </span>
      <p className="mt-2.5 border-t border-line pt-2.5 text-[11.5px] text-subtle">
        You can change all of this later in Settings.
      </p>
    </div>
  );
}

/* ------------------------------------------------ The save it finishes */

function SaveScreen({ saved }: { saved: boolean }) {
  return (
    <div>
      <div className="flex items-center gap-2">
        <AgentMarkMark look={saved ? "saved-fresh" : "saving"} size={18} />
        <p className="text-[15px] font-semibold tracking-tight text-ink">{saved ? "Saved!" : "Saving..."}</p>
      </div>
      <p className="mt-1 truncate text-[13px] text-muted">{SAMPLE.title}</p>
      <div
        className={cx(
          "flex items-center justify-between gap-3 pt-2 transition-opacity duration-300 motion-reduce:transition-none",
          saved ? "opacity-100" : "opacity-0",
        )}
      >
        <p className="min-w-0 truncate font-mono text-[10.5px] text-subtle">ID: {SAMPLE.id}</p>
        <span className="shrink-0 text-[12px] font-medium text-accent">More options</span>
      </div>
    </div>
  );
}

/* ------------------------------------------------------ Terminal aside */

/** Floating window over the page while the visitor is outside Chrome. */
const ASIDE_WINDOW =
  "absolute inset-x-3 bottom-3 z-[5] overflow-hidden rounded-lg shadow-frame @min-[460px]:right-auto @min-[460px]:w-[360px] @min-[460px]:bottom-5 @min-[460px]:left-5";

/** The installer's last lines. The popup is closed by now: Terminal has focus. */
function MiniTerminal({ visible, lines }: { visible: boolean; lines: number }) {
  return (
    <div
      aria-hidden
      className={cx(
        ASIDE_WINDOW,
        "border border-term-line bg-term-bg font-mono text-[10.5px] leading-[1.6] text-term-ink",
        "transition-[opacity,transform] duration-300 ease-out motion-reduce:transition-none",
        visible ? "translate-y-0 opacity-100" : "pointer-events-none translate-y-1 opacity-0",
      )}
    >
      <div className="flex h-6 items-center gap-1 border-b border-term-line bg-term-chrome px-2.5">
        <span className="size-1.5 rounded-full bg-term-line" />
        <span className="size-1.5 rounded-full bg-term-line" />
        <span className="size-1.5 rounded-full bg-term-line" />
        <span className="ml-auto text-[9.5px] text-term-muted">Terminal</span>
      </div>
      <div className="px-3 py-2.5">
        <p className="truncate text-term-ink">
          <span className="text-term-muted">$ </span>
          {COMMAND}
        </p>
        {TERMINAL_LINES.map((line, i) => (
          <p
            key={line.text}
            className={cx(
              "transition-opacity duration-200 motion-reduce:transition-none",
              line.tone === "ok" ? "truncate text-term-ok" : "mt-1 font-semibold text-term-ink",
              i < lines ? "opacity-100" : "opacity-0",
            )}
            style={{ transitionDelay: i < lines ? `${i * 260}ms` : "0ms" }}
          >
            {line.text}
          </p>
        ))}
      </div>
    </div>
  );
}

const APP_STEPS = ["Install the agentmark command", "Register with Chrome", "Add the agent skill"] as const;

/** The Mac app's first screen (packages/app), for the download path. */
function MiniApp({ visible, done }: { visible: boolean; done: boolean }) {
  return (
    <div
      aria-hidden
      className={cx(
        ASIDE_WINDOW,
        "border border-line-strong bg-surface text-ink",
        "transition-[opacity,transform] duration-300 ease-out motion-reduce:transition-none",
        visible ? "translate-y-0 opacity-100" : "pointer-events-none translate-y-1 opacity-0",
      )}
    >
      <div className="flex h-6 items-center gap-1 border-b border-line bg-chrome px-2.5">
        <span className="size-1.5 rounded-full bg-line-strong" />
        <span className="size-1.5 rounded-full bg-line-strong" />
        <span className="size-1.5 rounded-full bg-line-strong" />
        <span className="ml-2 flex items-center gap-1.5 text-[10px] font-medium text-chrome-ink">
          <AgentMarkMark look="brand" size={11} />
          AgentMark
        </span>
      </div>
      <div className="px-3.5 py-3">
        <p className="text-[13px] font-semibold tracking-tight">Welcome to AgentMark</p>
        <ul className="mt-2 space-y-1.5 text-[11.5px]">
          {APP_STEPS.map((step, i) => (
            <li key={step} className="flex items-center gap-2">
              <span
                className={cx(
                  "grid size-3.5 shrink-0 place-items-center rounded-full border transition-colors duration-300 motion-reduce:transition-none",
                  done ? "border-ok bg-ok-soft text-ok" : "border-line-strong text-transparent",
                )}
                style={{ transitionDelay: done ? `${i * 260}ms` : "0ms" }}
              >
                <svg viewBox="0 0 16 16" className="size-2.5" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M3.5 8.5 6.5 11.5 12.5 4.5" />
                </svg>
              </span>
              <span className="text-muted">{step}</span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
