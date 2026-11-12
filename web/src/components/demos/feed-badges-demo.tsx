"use client";

import { useRef, useState } from "react";
import { BrowserFrame, SAMPLE, useInView, usePrefersReducedMotion } from "./kit";
import { BrowserCursor, CURSOR_HIDDEN, pointAt, type CursorState } from "./browser-cursor";
import { BrowserPopup, type PopupStatus } from "./browser-popup";
import { BADGE_SAVING_MS, BADGE_TICK_MS, SaveBadge, type BadgeState } from "./browser-save-badge";
import { useStory, useTimeouts } from "./browser-story";
import { LiveRegion } from "./browser-ui";
import { FeedPost, type Post } from "./browser-feed-post";

const POSTS: Post[] = [
  {
    key: "guild",
    initials: "IH",
    name: "Ines Halvorsen",
    meta: "Build Guild · 1h",
    membersOnly: true,
    body: "Notes from Tuesday's incremental builds session: the cache key rules we agreed on.",
    title: "Ines Halvorsen: notes from the incremental builds session",
    id: "am_01JCDY2B7K9M4P6R8T0V3X5Z7Q",
    replies: 14,
    shares: 3,
  },
  {
    key: "mara",
    initials: "MO",
    name: "Mara Okafor",
    meta: "Platform engineer · 3h",
    body: "The best case for plain files I have read this year. Every migration I have done ended this way.",
    link: { host: "notes.example.com", title: SAMPLE.title },
    title: "Mara Okafor: the best case for plain files",
    id: "am_01JCD9V5N2Q8S4W7Y1A3C6E9GK",
    savedBefore: true,
    replies: 32,
    shares: 11,
  },
  {
    key: "jonas",
    initials: "JT",
    name: "Jonas Tveit",
    meta: "Writes about databases · 5h",
    body: "Full-text search in SQLite is better than you remember. A thread on FTS5 ranking, tokenizers and the traps.",
    title: "Jonas Tveit: full-text search in SQLite",
    id: "am_01JCDZ8H3J5K7M9N2P4Q6R8S0T",
    replies: 21,
    shares: 6,
  },
  {
    key: "rina",
    initials: "RA",
    name: "Rina Achterberg",
    meta: "Design lead · 1d",
    body: "Our rules for status colours. One colour, one meaning, everywhere in the product.",
    title: "Rina Achterberg: one colour, one meaning",
    id: "am_01JCB4F6G8H0J2K4M6N8P0Q2R4",
    savedBefore: true,
    replies: 8,
    shares: 2,
  },
];

/** The post that saves itself once, so a passive reader sees the sequence. */
const AUTO = "guild";

/*
 * Autoplay beats, in ms: 0 idle · 1 pointer glides to the badge · 2 press ·
 * 3 saving (hollow light blue) and the popup opens · 4 saved, tick ·
 * 5 popup closes (the product's 2 s auto-close) · 6 tick hold ends, plain blue.
 */
const TIMES = [0, 700, 1250, 1400, 1400 + BADGE_SAVING_MS, 3300 + BADGE_SAVING_MS, 1400 + BADGE_SAVING_MS + BADGE_TICK_MS] as const;
const POPUP_CLOSE_MS = 2000;

function autoBadge(beat: number): BadgeState {
  if (beat < 3) return "idle";
  if (beat === 3) return "saving";
  return beat < TIMES.length - 1 ? "saved-fresh" : "saved";
}

type Popup = { open: boolean; status: PopupStatus; post: Post };

export default function FeedBadgesDemo() {
  const { ref, inView } = useInView<HTMLDivElement>({ threshold: 0.4 });
  const reduced = usePrefersReducedMotion();
  const autoBadgeRef = useRef<HTMLButtonElement>(null);
  const [cursor, setCursor] = useState<CursorState>(CURSOR_HIDDEN);
  // Visitor saves override the autoplay and the page-load state, per post.
  const [clicked, setClicked] = useState<Partial<Record<string, BadgeState>>>({});
  const [popup, setPopup] = useState<Popup>({ open: false, status: "saved", post: POSTS[0] });
  const [message, setMessage] = useState("");
  const gens = useRef<Record<string, number>>({});
  const timers = useTimeouts();

  const story = useStory({
    times: TIMES,
    inView,
    reduced,
    onBeat: (b) => {
      const root = ref.current;
      const badge = autoBadgeRef.current;
      const auto = POSTS[0];
      if (b === 0 && root && badge) {
        const p = pointAt(root, badge, { x: 1, y: 2 });
        setCursor({ x: p.x - 120, y: p.y + 110, visible: false, pressed: false });
      }
      if (b === 1 && root && badge) setCursor({ ...pointAt(root, badge, { x: 1, y: 2 }), visible: true, pressed: false });
      if (b === 2) setCursor((c) => ({ ...c, pressed: true }));
      if (b === 3) {
        setCursor((c) => ({ ...c, pressed: false, visible: false }));
        setPopup({ open: true, status: "saving", post: auto });
      }
      if (b === 4) {
        setPopup({ open: true, status: "saved", post: auto });
        setMessage(`Saved to AgentMark: ${auto.title}`);
      }
      if (b === 5) setPopup((p) => ({ ...p, open: false }));
      if (b === TIMES.length - 1) {
        setCursor((c) => ({ ...c, visible: false }));
        setPopup((p) => ({ ...p, open: false }));
      }
    },
  });

  const stateOf = (post: Post): BadgeState =>
    clicked[post.key] ?? (post.key === AUTO ? autoBadge(story.beat) : post.savedBefore ? "saved" : "idle");

  const save = (post: Post) => {
    if (post.key === AUTO && story.running) story.settle();
    const gen = (gens.current[post.key] ?? 0) + 1;
    gens.current[post.key] = gen;
    const set = (s: BadgeState) => {
      if (gens.current[post.key] === gen) setClicked((c) => ({ ...c, [post.key]: s }));
    };
    if (reduced) {
      set("saved");
      setMessage(`Saved to AgentMark: ${post.title}`);
      return;
    }
    set("saving");
    setPopup({ open: true, status: "saving", post });
    timers.after(BADGE_SAVING_MS, () => {
      set("saved-fresh");
      setPopup((p) => (p.post.key === post.key ? { ...p, status: "saved" } : p));
      setMessage(`Saved to AgentMark: ${post.title}`);
    });
    timers.after(BADGE_SAVING_MS + POPUP_CLOSE_MS, () =>
      setPopup((p) => (p.post.key === post.key ? { ...p, open: false } : p)),
    );
    timers.after(BADGE_SAVING_MS + BADGE_TICK_MS, () => set("saved"));
  };

  return (
    <div
      ref={ref}
      className="@container relative h-[440px] w-full sm:h-[480px]"
      role="group"
      aria-label="Demo: a social feed with an AgentMark save button on every post"
    >
      <BrowserFrame
        url="feed.example/home"
        tabTitle="Home · feed.example"
        markLook="idle"
        bodyClassName="bg-sunken"
        overlay={
          // A badge click really opens the toolbar popup. Narrow frames skip it: it would hide the post being saved.
          <div className="hidden @min-[540px]:block">
            <BrowserPopup open={popup.open} status={popup.status} title={popup.post.title} id={popup.post.id} />
          </div>
        }
      >
        {/* The feed scrolls, like a feed: the later posts are there to click. */}
        <div className="h-full overflow-y-auto overscroll-contain [scrollbar-width:thin]">
          <FeedHeader />
          <div className="mx-auto flex max-w-[30rem] flex-col gap-2.5 px-3 pb-12 pt-3 @min-[400px]:px-4">
            {POSTS.map((post) => {
              const state = stateOf(post);
              return (
                <FeedPost key={post.key} post={post}>
                  {(describedBy) => (
                    <SaveBadge
                      ref={post.key === AUTO ? autoBadgeRef : undefined}
                      state={state}
                      savedBefore={Boolean(post.savedBefore) && clicked[post.key] === undefined}
                      describedBy={describedBy}
                      onSave={() => save(post)}
                    />
                  )}
                </FeedPost>
              );
            })}
          </div>
        </div>
        <div className="pointer-events-none absolute inset-x-0 bottom-0 h-8 bg-linear-to-t from-sunken to-transparent @min-[400px]:h-14" aria-hidden />
      </BrowserFrame>
      <BrowserCursor state={cursor} />
      <LiveRegion message={message} />
    </div>
  );
}

/** The signed-in feed's own top bar: generic, not any real site's. */
function FeedHeader() {
  return (
    <div className="flex h-9 items-center justify-between border-b border-line bg-surface px-4" aria-hidden>
      <span className="text-[13px] font-semibold tracking-tight text-ink">feed.example</span>
      <span className="flex items-center gap-2 text-[11px] text-subtle">
        Signed in
        <span className="grid size-6 place-items-center rounded-full bg-raised text-[10px] font-semibold text-muted ring-1 ring-line">
          YO
        </span>
      </span>
    </div>
  );
}
