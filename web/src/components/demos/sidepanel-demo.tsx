"use client";

import { useEffect, useRef, useState } from "react";
import { AgentMarkMark, BrowserFrame, SAMPLE, useInView, usePrefersReducedMotion } from "./kit";
import { MockArticle } from "./kit/mock-article";
import { AUTO_ACCEPT, NEW_ID, PENDING, SETTLED, type BookmarkState, type DemoBookmark } from "./browser-bookmarks";
import { BrowserCursor, CURSOR_HIDDEN, pointAt, type CursorState } from "./browser-cursor";
import { BookmarkDetailView, BookmarkListView, type Filter } from "./browser-sidepanel";
import { useStory } from "./browser-story";
import { LiveRegion, ReplayButton } from "./browser-ui";

/*
 * Beats, in ms: 0 just saved, in the list with "Summarising…" ·
 * 1 pointer glides to it · 2 press · 3 detail view, summary still writing ·
 * 4 summary lands · 5 suggested tags arrive · 6 pointer to the first
 * suggestion's ✓ · 7 press · 8 accepted: it is now a user tag.
 */
const TIMES = [0, 800, 1350, 1500, 3300, 3800, 5000, 5550, 5700] as const;

type View = { kind: "list" } | { kind: "detail"; id: string };

const SETTLED_VIEW: View = { kind: "detail", id: NEW_ID };

export default function SidepanelDemo() {
  const { ref, inView } = useInView<HTMLDivElement>({ threshold: 0.4 });
  const reduced = usePrefersReducedMotion();
  const [bookmarks, setBookmarks] = useState<DemoBookmark[]>(SETTLED);
  const [view, setView] = useState<View>(SETTLED_VIEW);
  const [filter, setFilter] = useState<Filter>("all");
  const [cursor, setCursor] = useState<CursorState>(CURSOR_HIDDEN);
  const [message, setMessage] = useState("");
  // Where focus goes after a visitor changes view. Autoplay never moves focus.
  const focusNext = useRef<string | null>(null);
  // Set while a visitor action cuts a run short: finish the data, keep their view.
  const keepView = useRef(false);

  const cursorTo = (selector: string) => {
    const root = ref.current;
    const target = root?.querySelector(selector);
    if (root && target) setCursor({ ...pointAt(root, target), visible: true, pressed: false });
  };

  const story = useStory({
    times: TIMES,
    inView,
    reduced,
    onBeat: (b) => {
      if (b === 0) {
        setBookmarks(PENDING);
        setView({ kind: "list" });
        setFilter("all");
        setCursor(CURSOR_HIDDEN);
      }
      if (b === 1) cursorTo(`[data-card="${NEW_ID}"]`);
      if (b === 2 || b === 7) setCursor((c) => ({ ...c, pressed: true }));
      if (b === 3) {
        setView({ kind: "detail", id: NEW_ID });
        setCursor((c) => ({ ...c, pressed: false, visible: false }));
      }
      if (b === 4) updateNew((x) => ({ ...x, summary: SAMPLE.summary }));
      if (b === 5) {
        updateNew((x) => ({ ...x, suggestedTags: [...SAMPLE.suggestedTags] }));
        setMessage(`Summary written. ${SAMPLE.suggestedTags.length} suggested tags.`);
      }
      if (b === 6) cursorTo(`[data-accept="${AUTO_ACCEPT}"]`);
      if (b === TIMES.length - 1) {
        setBookmarks(SETTLED);
        if (!keepView.current) setView(SETTLED_VIEW);
        setCursor((c) => ({ ...c, pressed: false, visible: false }));
      }
    },
  });

  function updateNew(fn: (b: DemoBookmark) => DemoBookmark) {
    setBookmarks((list) => list.map((x) => (x.id === NEW_ID ? fn(x) : x)));
  }

  /** A visitor action: finish any run first, so the two never fight. */
  const act = (fn: () => void) => {
    if (story.running) {
      keepView.current = true;
      story.settle();
      keepView.current = false;
    }
    fn();
  };

  const update = (id: string, fn: (b: DemoBookmark) => DemoBookmark) =>
    setBookmarks((list) => list.map((x) => (x.id === id ? fn(x) : x)));

  useEffect(() => {
    const key = focusNext.current;
    if (!key) return;
    focusNext.current = null;
    ref.current?.querySelector<HTMLElement>(key)?.focus();
  }, [view, ref]);

  const open = view.kind === "detail" ? bookmarks.find((b) => b.id === view.id) : undefined;
  const settled = story.beat === story.last;

  return (
    <div
      ref={ref}
      className="@container relative h-[460px] w-full sm:h-[500px]"
      role="group"
      aria-label="Demo: the AgentMark side panel, where the summary and suggested tags arrive after a save"
    >
      <BrowserFrame url={SAMPLE.displayUrl} tabTitle={SAMPLE.title} markLook="saved">
        <div className="flex h-full">
          <div className="hidden min-w-0 flex-1 @min-[560px]:block" aria-hidden>
            <MockArticle compact />
          </div>
          <aside
            aria-label="AgentMark side panel"
            className="flex w-full min-w-0 flex-col border-line bg-raised @min-[560px]:w-[320px] @min-[560px]:shrink-0 @min-[560px]:border-l"
          >
            <div className="flex h-10 shrink-0 items-center gap-2 border-b border-line px-3.5">
              <AgentMarkMark look="brand" size={16} />
              <span className="text-[13px] font-semibold text-ink">AgentMark</span>
              {settled ? (
                <ReplayButton onClick={() => story.play(0)} label="Replay the side panel demo" className="ml-auto" />
              ) : null}
            </div>
            <div className="relative min-h-0 flex-1">
              {open ? (
                <BookmarkDetailView
                  key={`detail-${open.id}`}
                  bookmark={open}
                  onBack={() =>
                    act(() => {
                      focusNext.current = `[data-card="${open.id}"]`;
                      setView({ kind: "list" });
                      setMessage("Bookmark list");
                    })
                  }
                  onAccept={(tag) =>
                    act(() => {
                      update(open.id, (x) => ({
                        ...x,
                        userTags: x.userTags.includes(tag) ? x.userTags : [...x.userTags, tag],
                        suggestedTags: x.suggestedTags.filter((t) => t !== tag),
                      }));
                      setMessage(`Accepted tag ${tag}`);
                    })
                  }
                  onReject={(tag) =>
                    act(() => {
                      update(open.id, (x) => ({ ...x, suggestedTags: x.suggestedTags.filter((t) => t !== tag) }));
                      setMessage(`Rejected tag ${tag}`);
                    })
                  }
                  onRemove={(tag) =>
                    act(() => {
                      update(open.id, (x) => ({ ...x, userTags: x.userTags.filter((t) => t !== tag) }));
                      setMessage(`Removed tag ${tag}`);
                    })
                  }
                  onAdvance={(next: BookmarkState) =>
                    act(() => {
                      update(open.id, (x) => ({ ...x, state: next }));
                      setMessage(next === "processed" ? "Marked processed" : "Archived");
                    })
                  }
                />
              ) : (
                <BookmarkListView
                  key="list"
                  bookmarks={bookmarks}
                  filter={filter}
                  onFilter={(f) => act(() => setFilter(f))}
                  onSelect={(id) =>
                    act(() => {
                      focusNext.current = "[data-back]";
                      setView({ kind: "detail", id });
                      setMessage(bookmarks.find((b) => b.id === id)?.title ?? "");
                    })
                  }
                />
              )}
              {/* Content below the fold fades out instead of being cut mid-control. */}
              <div className="pointer-events-none absolute inset-x-0 bottom-0 h-4 bg-linear-to-t from-raised to-transparent @min-[560px]:h-10" aria-hidden />
            </div>
          </aside>
        </div>
      </BrowserFrame>
      <BrowserCursor state={cursor} />
      <LiveRegion message={message} />
    </div>
  );
}
