"use client";

import { cx } from "./kit";

export type CursorState = {
  x: number;
  y: number;
  visible: boolean;
  pressed: boolean;
};

export const CURSOR_HIDDEN: CursorState = { x: 0, y: 0, visible: false, pressed: false };

/** Centre of `target` in the coordinate space of `container` (which must be positioned). */
export function pointAt(container: Element, target: Element, nudge = { x: 0, y: 0 }) {
  const c = container.getBoundingClientRect();
  const t = target.getBoundingClientRect();
  return { x: t.left - c.left + t.width / 2 + nudge.x, y: t.top - c.top + t.height / 2 + nudge.y };
}

/**
 * A pointer that glides to a point and presses. Decorative: autoplay only,
 * the visitor's own pointer does the real clicking.
 */
export function BrowserCursor({ state }: { state: CursorState }) {
  return (
    <div
      aria-hidden
      className={cx(
        "pointer-events-none absolute left-0 top-0 z-30 ease-[cubic-bezier(0.45,0,0.2,1)] motion-reduce:hidden",
        // While hidden it jumps, so the next run starts from its own spot.
        state.visible ? "opacity-100 transition-[opacity,transform] duration-[450ms]" : "opacity-0 transition-opacity duration-200",
      )}
      style={{ transform: `translate(${state.x - 2}px, ${state.y - 1.5}px)` }}
    >
      <svg
        width="16"
        height="20"
        viewBox="0 0 16 20"
        className={cx(
          "origin-top-left drop-shadow-sm transition-transform duration-150 ease-out",
          state.pressed && "scale-[0.86]",
        )}
      >
        <path
          d="M2 1.5v14.2l3.6-3.4 2.5 5.6 2.5-1.1-2.5-5.5h5z"
          className="fill-ink stroke-surface"
          strokeWidth="1.25"
          strokeLinejoin="round"
        />
      </svg>
    </div>
  );
}
