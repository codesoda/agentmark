/**
 * Copy shared with the extension's setup and Settings screens. Same questions,
 * same wording (packages/extension/src/shared/settings-shared.ts and
 * src/sidepanel/SettingsView.tsx). The app cannot import from the extension,
 * so these are copies: change both together.
 */

export const SUMMARIES_LABEL = "Write a summary and tags after each save";

/** What the summaries switch really does (contract §5.4). */
export const SUMMARIES_NOTE =
  "Sends the page text to your agent's provider. Your agent also turns LinkedIn, X, Hacker News and Reddit pages into readable text.";

export const SUMMARIES_OFF_NOTE = "With this off, those pages keep the text taken straight from the page.";

export const FOLDER_QUESTION = "Where should bookmarks live?";
export const FOLDER_HINT = "A normal folder. One sub-folder per saved page.";
export const AGENT_QUESTION = "Which agent writes summaries?";
export const AGENT_HINT = "AgentMark uses one you already have.";
/** Shown as a warning once the folder field differs from the saved folder (contract §5.4). */
export const FOLDER_MOVE_NOTE =
  "Bookmarks you already saved stay in the old folder. AgentMark cannot open or edit them until you move them into the new folder.";
export const CAPTURE_HEADING = "Capture the page from the browser on";
export const CAPTURE_NOTE = "On these sites AgentMark saves what your browser shows instead of fetching the page itself.";
export const PROMPT_PLACEHOLDER = 'Optional. For example: "Keep summaries under 60 words."';

export function joinNames(names: string[]): string {
  if (names.length <= 1) return names.join("");
  return `${names.slice(0, -1).join(", ")} or ${names[names.length - 1]}`;
}
