"use client";

import { useRef, useState, type Ref } from "react";
import { AgentMarkMark, BrowserFrame, SAMPLE, cx, useInView, usePrefersReducedMotion, type MarkLook } from "./kit";
import { HeroArticle } from "./browser-article";
import { BrowserCursor, CURSOR_HIDDEN, pointAt, type CursorState } from "./browser-cursor";
import { BrowserPopup } from "./browser-popup";
import { useStory } from "./browser-story";
import { LiveRegion, ReplayButton } from "./browser-ui";

/*
 * Beats, in ms from the start of a run:
 * 0 idle page · 1 pointer glides to the toolbar icon · 2 press ·
 * 3 popup opens "Saving…", icon hollow light blue · 4 "Saved", icon blue tick ·
 * 5 the background write-up lands · 6 the tick's 5 s hold ends, icon plain blue.
 */
const TIMES = [0, 600, 1150, 1300, 2300, 3700, 7300] as const;
const POPUP_OPEN = 3;
const SAVED = 4;
const WRITEUP = 5;

function iconLook(beat: number, last: number): MarkLook {
  if (beat < POPUP_OPEN) return "idle";
  if (beat < SAVED) return "saving";
  return beat < last ? "saved-fresh" : "saved";
}

export default function HeroSaveDemo() {
  const { ref, inView } = useInView<HTMLDivElement>({ threshold: 0.4 });
  const reduced = usePrefersReducedMotion();
  const iconRef = useRef<HTMLButtonElement>(null);
  const [cursor, setCursor] = useState<CursorState>(CURSOR_HIDDEN);
  const [message, setMessage] = useState("");

  const story = useStory({
    times: TIMES,
    inView,
    reduced,
    onBeat: (b) => {
      const root = ref.current;
      const icon = iconRef.current;
      if (b === 0 && root && icon) {
        const target = pointAt(root, icon, { x: 2, y: 3 });
        setCursor({ x: target.x - 170, y: target.y + 150, visible: false, pressed: false });
      }
      if (b === 1 && root && icon) setCursor({ ...pointAt(root, icon, { x: 2, y: 3 }), visible: true, pressed: false });
      if (b === 2) setCursor((c) => ({ ...c, pressed: true }));
      if (b === POPUP_OPEN) setCursor((c) => ({ ...c, pressed: false }));
      if (b === SAVED) {
        setCursor((c) => ({ ...c, visible: false }));
        setMessage("Page saved.");
      }
      if (b === WRITEUP) setMessage("Summary and suggested tags added in the background.");
    },
  });
  const { beat, last, running } = story;
  const look = iconLook(beat, last);
  const settled = beat === last;

  const save = () => {
    if (running) return;
    setMessage(reduced ? "Page saved. Summary and suggested tags added in the background." : "Saving…");
    // The visitor is the pointer: skip the autoplay's glide and press.
    story.play(POPUP_OPEN);
  };

  return (
    <div
      ref={ref}
      className="@container relative h-[380px] w-full sm:h-[460px] xl:h-[500px]"
      role="group"
      aria-label="Demo: saving a page with the AgentMark toolbar button"
    >
      <BrowserFrame
        url={SAMPLE.displayUrl}
        tabTitle={SAMPLE.title}
        extension={<ToolbarButton ref={iconRef} look={look} busy={running} onClick={save} />}
        overlay={<BrowserPopup open={beat >= POPUP_OPEN} status={beat >= SAVED ? "saved" : "saving"} writeupReady={beat >= WRITEUP} />}
      >
        <HeroArticle />
      </BrowserFrame>
      {settled ? (
        // Sits in the empty end of the tab strip: demo chrome, not part of the page.
        <ReplayButton onClick={() => story.play(0)} label="Replay the save demo" className="absolute right-2.5 top-1.5 z-20" />
      ) : null}
      <BrowserCursor state={cursor} />
      <LiveRegion message={message} />
    </div>
  );
}

function ToolbarButton({
  ref,
  look,
  busy,
  onClick,
}: {
  ref: Ref<HTMLButtonElement>;
  look: MarkLook;
  busy: boolean;
  onClick: () => void;
}) {
  const saved = look === "saved" || look === "saved-fresh";
  return (
    <button
      ref={ref}
      type="button"
      onClick={onClick}
      aria-disabled={busy}
      aria-label={saved ? "AgentMark: page saved. Save again" : "AgentMark: save this page"}
      className={cx(
        "grid size-7 place-items-center rounded-md transition-colors hover:bg-sunken",
        busy ? "cursor-default" : "cursor-pointer",
      )}
    >
      <AgentMarkMark look={look} size={16} />
    </button>
  );
}
