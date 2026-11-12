"use client";

import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import {
  BUNDLE,
  SAMPLE_BUNDLE_DIR as BUNDLE_DIR,
  SAMPLE_DATE_DIR as DATE_DIR,
  bundleDirName,
  dayDir,
  type BundleFile,
  type BundleFileName,
} from "./files-bundle";
import { BACKUPS, CLI_DESIGN, DOCS, FTS_GUIDE } from "./files-library";
import { highlight, type HighlightedLine, type Tone } from "./files-highlight";
import { WindowFrame, cx, useInView, usePrefersReducedMotion } from "./kit";

/**
 * On disk: one bundle's files in an editor-style window. The file list is a
 * tablist with manual activation (arrows move focus, Enter or Space opens).
 * When it first comes into view it opens a few files on its own, then rests
 * on bookmark.md, which is also the server-rendered state.
 */

const RESTING: BundleFileName = "bookmark.md";

/** Autoplay: open these files in turn, `at` ms after the demo comes into view. */
const TOUR: ReadonlyArray<{ at: number; file: BundleFileName }> = [
  { at: 900, file: "article.md" },
  { at: 3600, file: "events.jsonl" },
  { at: 6100, file: RESTING },
];

/** Scroll panes clip the global focus ring (drawn outside, unlayered CSS), so draw it inside. */
const INSET_FOCUS = { outlineOffset: -2 } as const;

const TONE_CLASS: Record<Tone, string> = {
  plain: "text-ink",
  prose: "text-muted",
  strong: "font-semibold text-ink",
  key: "text-accent",
  string: "text-ink",
  literal: "text-muted",
  punct: "text-subtle",
  mark: "text-accent",
  dim: "text-subtle",
  quote: "italic text-muted",
};

/** Neighbours from the same library the terminal demo lists, so the tree reads as a real storage folder. */
const EARLIER_DAYS = [CLI_DESIGN, BACKUPS, DOCS].map(dayDir).sort();
const SIBLINGS = [bundleDirName(FTS_GUIDE)];

const PREVIEWS: FilePreview[] = BUNDLE.map((file) => ({
  ...file,
  lines: highlight(file.content, file.language),
  size: formatSize(new TextEncoder().encode(file.content).length),
}));

export default function BundleViewerDemo() {
  const [selected, setSelected] = useState<BundleFileName>(RESTING);
  const reduced = usePrefersReducedMotion();
  const { ref, inView } = useInView({ threshold: 0.45 });
  const timers = useRef<number[]>([]);
  const tourDone = useRef(false);

  const stopTour = useCallback(() => {
    tourDone.current = true;
    timers.current.forEach(clearTimeout);
    timers.current = [];
  }, []);

  useEffect(() => {
    if (!inView || reduced || tourDone.current) return;
    timers.current = TOUR.map(({ at, file }, i) =>
      window.setTimeout(() => {
        setSelected(file);
        if (i === TOUR.length - 1) tourDone.current = true;
      }, at),
    );
    // Leaving the viewport mid-tour cancels it; it starts over next time.
    return () => {
      timers.current.forEach(clearTimeout);
      timers.current = [];
    };
  }, [inView, reduced]);

  const open = (name: BundleFileName) => {
    stopTour();
    setSelected(name);
  };

  const current = PREVIEWS.find((f) => f.name === selected) ?? PREVIEWS[0];

  return (
    <div
      ref={ref}
      role="group"
      aria-label="Example bundle for one saved page. Choose a file to preview it."
      className="h-[460px] w-full sm:h-[480px]"
    >
      <WindowFrame title={BUNDLE_DIR}>
        <div className="flex h-full flex-col sm:flex-row" onPointerDown={stopTour}>
          <FileTree selected={selected} onOpen={open} onInteract={stopTour} />
          <Preview file={current} />
        </div>
      </WindowFrame>
    </div>
  );
}

function FileTree({
  selected,
  onOpen,
  onInteract,
}: {
  selected: BundleFileName;
  onOpen: (name: BundleFileName) => void;
  onInteract: () => void;
}) {
  const listRef = useRef<HTMLDivElement>(null);

  // Keep the open file visible in the narrow horizontal strip. Scrolls only
  // the strip: scrollIntoView could move the page during autoplay.
  useEffect(() => {
    const list = listRef.current;
    const tab = list?.querySelector<HTMLElement>('[aria-selected="true"]');
    if (!list || !tab || list.scrollWidth <= list.clientWidth) return;
    const left = tab.offsetLeft - list.offsetLeft;
    if (left < list.scrollLeft || left + tab.offsetWidth > list.scrollLeft + list.clientWidth) {
      list.scrollTo({ left: left - 8, behavior: "smooth" });
    }
  }, [selected]);

  // Below sm the file list is a sideways strip. Fade whichever edge has more
  // files past it, so a phone visitor can tell the strip scrolls.
  const [more, setMore] = useState({ before: false, after: false });
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const measure = () => {
      const max = list.scrollWidth - list.clientWidth;
      const next = { before: max > 1 && list.scrollLeft > 1, after: max > 1 && list.scrollLeft < max - 1 };
      setMore((prev) => (prev.before === next.before && prev.after === next.after ? prev : next));
    };
    measure();
    list.addEventListener("scroll", measure, { passive: true });
    const ro = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    ro?.observe(list);
    return () => {
      list.removeEventListener("scroll", measure);
      ro?.disconnect();
    };
  }, []);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const tabs = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]'));
    const index = tabs.findIndex((t) => t === document.activeElement);
    const last = tabs.length - 1;
    const next: Record<string, number> = {
      ArrowDown: index === last ? 0 : index + 1,
      ArrowRight: index === last ? 0 : index + 1,
      ArrowUp: index <= 0 ? last : index - 1,
      ArrowLeft: index <= 0 ? last : index - 1,
      Home: 0,
      End: last,
    };
    if (!(e.key in next)) return;
    e.preventDefault();
    onInteract();
    tabs[next[e.key]].focus();
  };

  return (
    <div className="shrink-0 border-b border-line bg-raised sm:w-[12.5rem] sm:border-b-0 sm:border-r">
      <div className="px-3 pt-2.5 font-mono text-[11px] leading-[22px] text-subtle sm:pt-3">
        {EARLIER_DAYS.map((day) => (
          <Folder key={day} name={day} className="hidden sm:flex" />
        ))}
        <Folder name={DATE_DIR} open className="hidden sm:flex" />
        {SIBLINGS.map((dir) => (
          <Folder key={dir} name={dir} className="hidden sm:flex sm:pl-3.5" />
        ))}
        <Folder name={BUNDLE_DIR} open current className="sm:pl-3.5" />
      </div>
      <div className="relative">
        <div
          ref={listRef}
          role="tablist"
          aria-label="Files in the bundle"
          onKeyDown={onKeyDown}
          className="flex gap-1 overflow-x-auto px-2 pt-1.5 pb-2.5 [scrollbar-width:none] sm:flex-col sm:gap-px sm:overflow-visible sm:pt-0.5 sm:pl-[2.6rem]"
        >
          {BUNDLE.map((file) => {
            const isSelected = file.name === selected;
            return (
              <button
                key={file.name}
                type="button"
                role="tab"
                id={tabId(file.name)}
                aria-selected={isSelected}
                aria-controls="bundle-file-panel"
                tabIndex={isSelected ? 0 : -1}
                onClick={() => onOpen(file.name)}
                className={cx(
                  "flex shrink-0 items-center gap-2 rounded-md px-2.5 py-1.5 text-left font-mono text-[11.5px] whitespace-nowrap transition-colors duration-150 motion-reduce:transition-none sm:py-[5px]",
                  isSelected
                    ? "bg-accent-soft text-accent"
                    : "text-muted hover:bg-sunken hover:text-ink",
                )}
              >
                <FileGlyph />
                {file.name}
              </button>
            );
          })}
        </div>
        <EdgeFade side="left" show={more.before} />
        <EdgeFade side="right" show={more.after} />
      </div>
    </div>
  );
}

/** A soft edge on the phone file strip, with a chevron, shown while files sit past that edge. */
function EdgeFade({ side, show }: { side: "left" | "right"; show: boolean }) {
  const right = side === "right";
  return (
    <div
      aria-hidden
      className={cx(
        "pointer-events-none absolute inset-y-0 flex w-10 items-center from-raised from-35% to-transparent transition-opacity duration-200 motion-reduce:transition-none sm:hidden",
        right ? "right-0 justify-end bg-linear-to-l pr-1" : "left-0 justify-start bg-linear-to-r pl-1",
        show ? "opacity-100" : "opacity-0",
      )}
    >
      <svg viewBox="0 0 16 16" className={cx("size-3 text-subtle", !right && "rotate-180")} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
        <path d="m6 4 4 4-4 4" />
      </svg>
    </div>
  );
}

type FilePreview = BundleFile & { lines: HighlightedLine[]; size: string };

function Preview({ file }: { file: FilePreview }) {
  const { lines, size } = file;
  const gutter = String(lines.length).length;
  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col bg-surface">
      <div className="shrink-0 border-b border-line px-4 py-2.5 sm:px-5">
        <p className="flex items-baseline gap-3 font-mono text-[11.5px]">
          <span className="text-ink">{file.name}</span>
          <span className="ml-auto text-subtle">{size}</span>
        </p>
        <p className="mt-0.5 truncate text-[12.5px] text-muted">{file.caption}</p>
      </div>
      <div
        key={file.name}
        id="bundle-file-panel"
        role="tabpanel"
        aria-labelledby={tabId(file.name)}
        tabIndex={0}
        style={INSET_FOCUS}
        className="min-h-0 flex-1 overflow-auto py-3 font-mono text-[11.5px] leading-[1.7] transition-opacity duration-200 ease-out starting:opacity-0 motion-reduce:transition-none sm:text-[12px]"
      >
        <div className={cx(file.language === "markdown" ? "w-full" : "w-max min-w-full")}>
          {lines.map(({ tokens, wrap }, i) => (
            <div key={i} className="flex">
              <span
                aria-hidden
                className="sticky left-0 shrink-0 bg-surface pr-3 pl-3 text-right text-subtle/70 select-none sm:pl-4"
                style={{ width: `${gutter + 3.5}ch` }}
              >
                {i + 1}
              </span>
              <span className={cx("min-w-0 pr-5", wrap ? "whitespace-pre-wrap break-words" : "whitespace-pre")}>
                {tokens.length === 0
                  ? "\u200b"
                  : tokens.map((t, j) => (
                      <span key={j} className={TONE_CLASS[t.tone]}>
                        {t.text}
                      </span>
                    ))}
              </span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function tabId(name: BundleFileName) {
  return `bundle-file-${name.replace(".", "-")}`;
}

function formatSize(bytes: number) {
  return bytes < 1000 ? `${bytes} B` : `${(bytes / 1000).toFixed(1)} KB`;
}

function FileGlyph() {
  return (
    <svg viewBox="0 0 16 16" className="size-3.5 shrink-0 opacity-70" fill="none" stroke="currentColor" strokeWidth="1.3" aria-hidden>
      <path d="M4 1.75h5.25L12 4.5v9.75H4z M9 1.75V4.75h3" strokeLinejoin="round" />
    </svg>
  );
}

/** A folder row in the tree. Only the open bundle's files are interactive. */
function Folder({
  name,
  open = false,
  current = false,
  className,
}: {
  name: string;
  open?: boolean;
  current?: boolean;
  className?: string;
}) {
  return (
    <p className={cx("flex min-w-0 items-center gap-1", current && "text-muted", className)} title={name}>
      <svg viewBox="0 0 16 16" className={cx("size-3 shrink-0 opacity-70", open && "rotate-90")} fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
        <path d="m6 4 4 4-4 4" />
      </svg>
      <svg viewBox="0 0 16 16" className="size-3.5 shrink-0" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round" aria-hidden>
        <path d={open ? "M1.75 12.5V3h4l1.25 1.25H12.5V7 M1.75 12.5h11l1.5-5.5H4.5z" : "M1.75 3h4l1.25 1.25h7.25v8.25H1.75z"} />
      </svg>
      <span className="ml-0.5 truncate">{name}</span>
    </p>
  );
}
