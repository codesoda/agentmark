import { SAMPLE } from "./kit";

/*
 * Side panel sample data. The newest bookmark is the shared sample essay;
 * the others are pages the feed demo shows as saved, so the page tells one
 * story. Field names follow the extension's BookmarkSummary/BookmarkDetail.
 */

export type BookmarkState = "inbox" | "processed" | "archived";

export type DemoBookmark = {
  id: string;
  title: string;
  savedOn: string;
  state: BookmarkState;
  /** Null until the background write-up lands. */
  summary: string | null;
  userTags: string[];
  suggestedTags: string[];
};

export const STATE_LABELS: Record<BookmarkState, string> = {
  inbox: "Inbox",
  processed: "Processed",
  archived: "Archived",
};

/** The side panel's "next state" button (BookmarkDetail.tsx). */
export const STATE_TRANSITIONS: Record<BookmarkState, { next: BookmarkState; label: string } | null> = {
  inbox: { next: "processed", label: "Mark Processed" },
  processed: { next: "archived", label: "Archive" },
  archived: null,
};

export const NEW_ID = SAMPLE.id;
/** The suggested tag the autoplay accepts. */
export const AUTO_ACCEPT = SAMPLE.suggestedTags[0];

const OLDER: DemoBookmark[] = [
  {
    id: "am_01JCDY2B7K9M4P6R8T0V3X5Z7Q",
    title: "Ines Halvorsen: notes from the incremental builds session",
    savedOn: "Nov 12, 2026",
    state: "inbox",
    summary:
      "Members-only notes from a build guild session. Sets out rules for cache keys, why timestamps were dropped from them, and lists the questions left open for the next meeting.",
    userTags: ["builds"],
    suggestedTags: [],
  },
  {
    id: "am_01JCD9V5N2Q8S4W7Y1A3C6E9GK",
    title: "Mara Okafor: the best case for plain files",
    savedOn: "Nov 11, 2026",
    state: "processed",
    summary: "A short post that recommends an essay on plain-text notes, with a thread of readers describing their own app migrations.",
    userTags: ["plain-text"],
    suggestedTags: [],
  },
  {
    id: "am_01JCB4F6G8H0J2K4M6N8P0Q2R4",
    title: "Rina Achterberg: one colour, one meaning",
    savedOn: "Nov 9, 2026",
    state: "processed",
    summary: "A design lead's rules for status colours: each colour has one meaning, used the same way in every part of the product.",
    userTags: ["design-systems"],
    suggestedTags: ["colour"],
  },
];

/** Just saved: no summary or suggestions yet. */
export const PENDING: DemoBookmark[] = [
  {
    id: NEW_ID,
    title: SAMPLE.title,
    savedOn: "Nov 12, 2026",
    state: "inbox",
    summary: null,
    userTags: [...SAMPLE.userTags],
    suggestedTags: [],
  },
  ...OLDER,
];

/** Final state: write-up landed, one suggestion accepted. Also the server render. */
export const SETTLED: DemoBookmark[] = [
  {
    ...PENDING[0],
    summary: SAMPLE.summary,
    userTags: [...SAMPLE.userTags, AUTO_ACCEPT],
    suggestedTags: SAMPLE.suggestedTags.filter((t) => t !== AUTO_ACCEPT),
  },
  ...OLDER,
];
