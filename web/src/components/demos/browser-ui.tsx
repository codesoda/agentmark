"use client";

import type { ReactNode } from "react";
import { cx } from "./kit";

/** Small "run it again" control. Only rendered once a run has finished. */
export function ReplayButton({ onClick, label, className }: { onClick: () => void; label: string; className?: string }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      className={cx(
        "inline-flex h-6 items-center gap-1.5 rounded-full border border-line bg-surface px-1.5 text-[11px] @min-[540px]:px-2.5 font-medium text-muted transition-colors hover:border-line-strong hover:text-ink starting:opacity-0 motion-safe:transition-[opacity,color,border-color] motion-safe:duration-300",
        className,
      )}
    >
      <svg viewBox="0 0 16 16" className="size-3" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
        <path d="M2.75 8a5.25 5.25 0 1 0 1.6-3.77M2.75 2.5v3h3" />
      </svg>
      <span className="hidden @min-[540px]:inline">Replay</span>
    </button>
  );
}

/** A soft "working" dot. Holds still with reduced motion. */
export function PulseDot({ className }: { className?: string }) {
  return (
    <span className={cx("relative inline-flex size-1.5 shrink-0", className)} aria-hidden>
      <span className="absolute inset-0 rounded-full bg-accent opacity-60 motion-safe:animate-ping" />
      <span className="relative size-1.5 rounded-full bg-accent" />
    </span>
  );
}

/** Polite status for screen readers. Only changes on meaningful beats. */
export function LiveRegion({ message }: { message: string }) {
  return (
    <p className="sr-only" aria-live="polite" aria-atomic="true">
      {message}
    </p>
  );
}

/** Height-animating wrapper: collapses to nothing without measuring. */
export function Collapse({ open, children, className }: { open: boolean; children: ReactNode; className?: string }) {
  return (
    <div
      className={cx(
        "grid transition-[grid-template-rows,opacity] duration-300 ease-out motion-reduce:transition-none",
        open ? "grid-rows-[1fr] opacity-100" : "grid-rows-[0fr] opacity-0",
        className,
      )}
      aria-hidden={!open}
      inert={!open}
    >
      <div className="min-h-0 overflow-hidden">{children}</div>
    </div>
  );
}
