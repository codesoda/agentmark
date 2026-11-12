"use client";

import type { Ref } from "react";
import { AgentMarkMark, cx, type MarkLook } from "./kit";

/** The in-page button's states (packages/extension/src/content/save-button.ts). */
export type BadgeState = "idle" | "saving" | "saved-fresh" | "saved";

/** How long the hollow "saving" look shows in the demo. A real save takes about as long. */
export const BADGE_SAVING_MS = 900;
/** A fresh save shows its tick this long, then settles. Matches SAVED_TICK_MS. */
export const BADGE_TICK_MS = 5000;

/** The product's tooltip and accessible name for each state. */
export function badgeLabel(state: BadgeState, savedBefore: boolean) {
  if (state === "saving") return "Saving to AgentMark…";
  if (state === "idle") return "Save to AgentMark";
  return savedBefore ? "Saved to AgentMark — click to update" : "Saved to AgentMark";
}

/**
 * The per-post save button. A real button with the product's labels as its
 * name and tooltip. Saving keeps focus (aria-disabled, not disabled), so a
 * keyboard user is not dropped mid-save.
 */
export function SaveBadge({
  ref,
  state,
  savedBefore,
  describedBy,
  onSave,
}: {
  ref?: Ref<HTMLButtonElement>;
  state: BadgeState;
  /** Saved when the page loaded, so a click updates rather than saves. */
  savedBefore: boolean;
  describedBy?: string;
  onSave: () => void;
}) {
  const label = badgeLabel(state, savedBefore);
  const saving = state === "saving";
  const look: MarkLook = state;
  return (
    <span className="group/badge relative inline-flex">
      <button
        ref={ref}
        type="button"
        aria-label={label}
        aria-describedby={describedBy}
        aria-disabled={saving}
        onClick={() => {
          if (!saving) onSave();
        }}
        className={cx(
          "grid size-8 place-items-center rounded-md transition-colors hover:bg-sunken",
          saving ? "cursor-progress" : "cursor-pointer",
        )}
      >
        <AgentMarkMark look={look} size={20} />
      </button>
      <span
        aria-hidden
        className="pointer-events-none absolute bottom-full right-0 z-10 mb-1.5 whitespace-nowrap rounded-md bg-ink px-2 py-1 text-[11px] font-medium text-bg opacity-0 shadow-frame transition-[opacity,transform] duration-150 ease-out translate-y-0.5 group-hover/badge:translate-y-0 group-hover/badge:opacity-100 group-has-focus-visible/badge:translate-y-0 group-has-focus-visible/badge:opacity-100 motion-reduce:transition-none"
      >
        {label}
      </span>
    </span>
  );
}
