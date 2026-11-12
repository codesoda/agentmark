/** Class names, copy and helpers shared by the setup and Settings forms. */

import type { FieldError, SettingsField } from "./types";

export const focusRing = "focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1";

export const inputClass =
  "block w-full min-w-0 rounded-lg border bg-white px-2.5 py-1.5 font-mono text-[12.5px] text-gray-900 placeholder:font-sans placeholder:text-gray-400 focus:outline-none focus:ring-2 focus:ring-indigo-500/30";

export const primaryButtonClass =
  "flex w-full items-center justify-center rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-60 " +
  "focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-2";

/**
 * What the summaries switch really does (contract §5.4): turning it off also
 * stops the agent writing the readable copy of pages captured from the browser.
 */
export const SUMMARIES_LABEL = "Write a summary and tags after each save";
export const SUMMARIES_NOTE =
  "Sends the page text to your agent's provider. Your agent also turns LinkedIn, X, Hacker News and Reddit pages into readable text.";
/** What turning the switch off does for those pages; shown with the note on setup and Settings. */
export const SUMMARIES_OFF_NOTE = "With this off, those pages keep the text taken straight from the page.";

/**
 * What changing the storage folder does (contract §5.4). Shown as a warning
 * once the folder field differs from the saved folder.
 */
export const FOLDER_MOVE_NOTE =
  "Bookmarks you already saved stay in the old folder. AgentMark cannot open or edit them until you move them into the new folder.";

/** The first error for `field` (any index unless given). */
export function errorFor(errors: FieldError[], field: SettingsField, index?: number | null): FieldError | undefined {
  return errors.find((e) => e.field === field && (index === undefined || e.index === index));
}

