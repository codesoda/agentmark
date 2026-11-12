"use client";

import { AgentMarkMark, SAMPLE, cx } from "./kit";
import { Collapse, PulseDot } from "./browser-ui";

export type PopupStatus = "saving" | "saved";

/**
 * The toolbar popup's quick-save view (packages/extension/src/popup/Popup.tsx):
 * "Saving…" while the save runs, then "Saved" with the title, the bookmark ID
 * and "More options". The "In the background" strip is the demo's window on
 * the write-up that runs after the save; the popup itself never waits on it.
 */
export function BrowserPopup({
  open,
  status,
  title = SAMPLE.title,
  id = SAMPLE.id,
  writeupReady,
}: {
  open: boolean;
  status: PopupStatus;
  title?: string;
  id?: string;
  /** Omit to leave out the background strip. */
  writeupReady?: boolean;
}) {
  const saved = status === "saved";
  return (
    <div
      className={cx(
        "w-[236px] origin-top-right rounded-lg border border-line-strong bg-surface shadow-frame @min-[540px]:w-[272px]",
        "transition-[opacity,transform] duration-200 ease-out motion-reduce:transition-none",
        open ? "translate-y-0 scale-100 opacity-100" : "pointer-events-none -translate-y-1 scale-[0.98] opacity-0",
      )}
      aria-hidden={!open}
      inert={!open}
    >
      <div className="p-3.5 @min-[540px]:p-4">
        <div className="flex items-center gap-2">
          <AgentMarkMark look={saved ? "saved-fresh" : "saving"} size={18} />
          <p className="text-[15px] font-semibold tracking-tight text-ink">{saved ? "Saved" : "Saving…"}</p>
        </div>
        <p className="mt-1 truncate text-[13px] text-muted">{title}</p>

        <Collapse open={saved}>
          <div className="flex items-center justify-between gap-3 pt-2">
            <p className="min-w-0 truncate font-mono text-[10.5px] text-subtle">ID: {id}</p>
            <span className="shrink-0 text-[12px] font-medium text-accent">More options</span>
          </div>
          {writeupReady === undefined ? null : <Writeup ready={writeupReady} />}
        </Collapse>
      </div>
    </div>
  );
}

function Writeup({ ready }: { ready: boolean }) {
  return (
    <div className="mt-3 border-t border-line pt-3">
      <div className="flex items-center justify-between gap-2 text-[11px]">
        <p className="font-medium text-subtle">In the background</p>
        <p className="flex items-center gap-1.5 text-subtle">
          {ready ? (
            <span>Summary ready</span>
          ) : (
            <>
              <PulseDot />
              <span>Summarising…</span>
            </>
          )}
        </p>
      </div>

      {/* Skeleton and text share one cell, so nothing moves when the text lands.
          Short frames (below sm) drop it so the popup stays clear of the headline. */}
      <div className="mt-1.5 hidden *:col-start-1 *:row-start-1 sm:grid">
        <p
          className={cx(
            "line-clamp-2 text-[12px] leading-[1.45] text-muted transition-opacity duration-300 motion-reduce:transition-none",
            ready ? "opacity-100" : "opacity-0",
          )}
        >
          {SAMPLE.summary}
        </p>
        <div
          className={cx(
            "flex flex-col justify-center gap-2 transition-opacity duration-200 motion-reduce:transition-none",
            ready ? "opacity-0" : "opacity-100",
          )}
          aria-hidden
        >
          <span className="h-1.5 w-full rounded-full bg-line" />
          <span className="h-1.5 w-3/4 rounded-full bg-line" />
        </div>
      </div>

      <ul className="mt-2.5 flex flex-wrap gap-1" aria-label="Suggested tags">
        {SAMPLE.suggestedTags.map((tag, i) => (
          <li
            key={tag}
            className={cx(
              "rounded border border-dashed border-line-strong px-1.5 py-px text-[11px] text-muted transition-[opacity,transform] duration-300 ease-out motion-reduce:transition-none",
              ready ? "translate-y-0 opacity-100" : "translate-y-0.5 opacity-0",
            )}
            style={{ transitionDelay: ready ? `${150 + i * 70}ms` : "0ms" }}
          >
            {tag}
          </li>
        ))}
      </ul>
    </div>
  );
}
