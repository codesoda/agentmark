"use client";

import Link from "next/link";
import { useEffect, useRef, useState } from "react";
import { BrowserPopup } from "@/components/demos/browser-popup";
import { AgentMarkMark, usePrefersReducedMotion, type MarkLook } from "@/components/demos/kit";
import { SITE_TITLE } from "@/lib/site";

/** How long the save "runs" before it lands, and how long the popup stays. */
const SAVING_MS = 750;
const POPUP_MS = 3600;

/** Sample ID in the real format; nothing is saved by this page. */
const SAMPLE_ID = "am_01JCDX4Q8N3T7V2K9R6M5H1BZE";

type Phase = "rest" | "saving" | "saved-open" | "saved";

const LOOK: Record<Phase, MarkLook> = {
  rest: "brand",
  saving: "saving",
  "saved-open": "saved-fresh",
  saved: "saved",
};

/**
 * The header logo. A click plays what a real toolbar click does: the mark goes
 * hollow while the save runs, the popup shows the saved page, then the popup
 * closes and the mark stays in its plain saved look. The link still goes to
 * the top of the page.
 */
export function HeaderBrand() {
  const [phase, setPhase] = useState<Phase>("rest");
  const reduced = usePrefersReducedMotion();
  const timers = useRef<number[]>([]);

  const clearTimers = () => {
    timers.current.forEach((t) => window.clearTimeout(t));
    timers.current = [];
  };
  useEffect(() => clearTimers, []);

  const play = () => {
    clearTimers();
    const savingFor = reduced ? 0 : SAVING_MS;
    setPhase(reduced ? "saved-open" : "saving");
    if (!reduced) timers.current.push(window.setTimeout(() => setPhase("saved-open"), savingFor));
    timers.current.push(window.setTimeout(() => setPhase("saved"), savingFor + POPUP_MS));
  };

  const open = phase === "saving" || phase === "saved-open";

  return (
    <div
      className="relative"
      onKeyDown={(e) => {
        if (e.key === "Escape" && open) {
          clearTimers();
          setPhase("saved");
        }
      }}
    >
      <Link
        href="/#top"
        onClick={play}
        className="flex items-center gap-2.5 rounded-md text-[15px] font-semibold tracking-tight text-ink"
      >
        <AgentMarkMark look={LOOK[phase]} size={22} />
        AgentMark
      </Link>
      <div className="absolute left-0 top-full z-50 pt-3 *:origin-top-left">
        <BrowserPopup open={open} status={phase === "saving" ? "saving" : "saved"} title={SITE_TITLE} id={SAMPLE_ID} />
      </div>
      <p className="sr-only" aria-live="polite">
        {phase === "saving" ? "Saving…" : phase === "saved-open" ? "Saved. This is a demonstration; nothing was saved." : ""}
      </p>
    </div>
  );
}
