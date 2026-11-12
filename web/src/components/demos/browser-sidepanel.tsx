"use client";

import { SAMPLE, cx } from "./kit";
import { PulseDot } from "./browser-ui";
import { STATE_LABELS, STATE_TRANSITIONS, type BookmarkState, type DemoBookmark } from "./browser-bookmarks";

/*
 * Token-styled replicas of the extension's side panel
 * (packages/extension/src/sidepanel/): BookmarkList + BookmarkCard, and
 * BookmarkDetail with TagManager. Same structure and wording.
 */

export type Filter = BookmarkState | "all";

const FILTERS: Array<{ value: Filter; label: string }> = [
  { value: "all", label: "All" },
  { value: "inbox", label: "Inbox" },
  { value: "processed", label: "Processed" },
  { value: "archived", label: "Archived" },
];

const STATE_BADGE: Record<BookmarkState, string> = {
  inbox: "bg-accent-soft text-accent",
  processed: "border border-line-strong text-muted",
  archived: "bg-sunken text-subtle",
};

const VIEW_IN = "transition-[opacity,transform] duration-300 ease-out starting:opacity-0 motion-reduce:transition-none";
const CHIP = "inline-flex h-[22px] items-center gap-px rounded px-1.5 text-[11.5px]";
const USER_CHIP = "bg-accent-soft text-accent";
const SUGGESTED_CHIP = "border border-dashed border-line-strong text-muted";
const CHIP_IN = "transition-[opacity,transform] duration-300 ease-out starting:scale-90 starting:opacity-0 motion-reduce:transition-none";

function Label({ children }: { children: string }) {
  return <p className="mb-1 text-[11px] font-medium text-subtle">{children}</p>;
}

/* ------------------------------------------------------------------ List */

export function BookmarkListView({
  bookmarks,
  filter,
  onFilter,
  onSelect,
}: {
  bookmarks: DemoBookmark[];
  filter: Filter;
  onFilter: (f: Filter) => void;
  onSelect: (id: string) => void;
}) {
  const shown = filter === "all" ? bookmarks : bookmarks.filter((b) => b.state === filter);
  return (
    <div className={cx("flex h-full flex-col starting:-translate-x-2", VIEW_IN)}>
      <div className="flex shrink-0 border-b border-line" role="group" aria-label="Filter bookmarks">
        {FILTERS.map(({ value, label }) => (
          <button
            key={value}
            type="button"
            aria-pressed={filter === value}
            onClick={() => onFilter(value)}
            className={cx(
              "-mb-px flex-1 border-b-2 px-1 py-2 text-[11.5px] font-medium transition-colors",
              filter === value ? "border-accent text-accent" : "border-transparent text-subtle hover:text-ink",
            )}
          >
            {label}
          </button>
        ))}
      </div>
      <ul className="min-h-0 flex-1 space-y-2 overflow-y-auto p-2.5">
        {shown.length === 0 ? (
          <li className="mt-8 text-center text-[12.5px] text-subtle">No bookmarks found</li>
        ) : (
          shown.map((b) => (
            <li key={b.id}>
              <BookmarkCard bookmark={b} onSelect={onSelect} />
            </li>
          ))
        )}
      </ul>
    </div>
  );
}

function BookmarkCard({ bookmark: b, onSelect }: { bookmark: DemoBookmark; onSelect: (id: string) => void }) {
  return (
    <button
      type="button"
      data-card={b.id}
      onClick={() => onSelect(b.id)}
      className="w-full rounded-lg border border-line bg-surface p-3 text-left transition-colors hover:border-line-strong"
    >
      <span className="flex items-start justify-between gap-2">
        <span className="line-clamp-2 flex-1 text-[12.5px] font-medium leading-snug text-ink">{b.title}</span>
        <span className={cx("shrink-0 rounded px-1.5 py-0.5 text-[10.5px] font-medium", STATE_BADGE[b.state])}>
          {STATE_LABELS[b.state]}
        </span>
      </span>
      <span className="mt-1 flex items-center gap-1.5 text-[11px] text-subtle">
        {b.savedOn}
        {b.summary === null ? (
          <>
            <span aria-hidden>·</span>
            <PulseDot />
            <span>Summarising…</span>
          </>
        ) : null}
      </span>
      {b.userTags.length + b.suggestedTags.length > 0 ? (
        <span className="mt-2 flex flex-wrap gap-1">
          {b.userTags.map((t) => (
            <span key={`u-${t}`} className={cx(CHIP, USER_CHIP, CHIP_IN)}>
              {t}
            </span>
          ))}
          {b.suggestedTags.map((t) => (
            <span key={`s-${t}`} className={cx(CHIP, SUGGESTED_CHIP, CHIP_IN)}>
              {t}
              <span className="sr-only"> (suggested)</span>
            </span>
          ))}
        </span>
      ) : null}
    </button>
  );
}

/* ---------------------------------------------------------------- Detail */

export function BookmarkDetailView({
  bookmark: b,
  onBack,
  onAccept,
  onReject,
  onRemove,
  onAdvance,
}: {
  bookmark: DemoBookmark;
  onBack: () => void;
  onAccept: (tag: string) => void;
  onReject: (tag: string) => void;
  onRemove: (tag: string) => void;
  onAdvance: (next: BookmarkState) => void;
}) {
  const transition = STATE_TRANSITIONS[b.state];
  const actionable = b.suggestedTags.filter((t) => !b.userTags.includes(t));
  return (
    <div className={cx("flex h-full flex-col starting:translate-x-2", VIEW_IN)}>
      {/* The action sits in a footer under the scroll area, so the frame never cuts it off. */}
      <div className="relative min-h-0 flex-1">
        <div className="h-full space-y-3 overflow-y-auto px-4 py-3.5 @min-[560px]:space-y-3.5">
          <button type="button" data-back onClick={onBack} className="text-[12px] font-medium text-accent hover:underline">
            &larr; Back to list
          </button>

          <p className="line-clamp-2 text-[13.5px] font-medium leading-snug text-accent">{b.title}</p>

          <Summary text={b.summary} />

          <div>
            <Label>Tags</Label>
            <ul className="flex min-h-[32px] flex-wrap items-center gap-1 rounded-md border border-line bg-surface px-1.5 py-1" aria-label="Tags">
              {b.userTags.map((t) => (
                <li key={t} className={cx(CHIP, USER_CHIP, CHIP_IN)}>
                  {t}
                  <ChipButton label={`Remove tag ${t}`} onClick={() => onRemove(t)} className="text-accent/70 hover:text-accent">
                    ×
                  </ChipButton>
                </li>
              ))}
            </ul>
          </div>

          {actionable.length > 0 ? (
            <div className={CHIP_IN}>
              <Label>Suggested tags</Label>
              <ul className="flex flex-wrap gap-1" aria-label="Suggested tags">
                {actionable.map((t, i) => (
                  <li key={t} className={cx(CHIP, SUGGESTED_CHIP, CHIP_IN)} style={{ transitionDelay: `${i * 70}ms` }}>
                    {t}
                    <ChipButton label={`Accept tag ${t}`} onClick={() => onAccept(t)} accept={t} className="text-accent hover:bg-accent-soft">
                      ✓
                    </ChipButton>
                    <ChipButton label={`Reject tag ${t}`} onClick={() => onReject(t)} className="text-subtle hover:bg-sunken hover:text-ink">
                      ×
                    </ChipButton>
                  </li>
                ))}
              </ul>
            </div>
          ) : null}

          <p className="flex flex-wrap gap-x-3 text-[11px] text-subtle">
            <span>{b.savedOn}</span>
            <span>Extension</span>
            <span>{STATE_LABELS[b.state]}</span>
          </p>
        </div>
        {/* Content below the fold fades out instead of being cut mid-line. */}
        <div className="pointer-events-none absolute inset-x-0 bottom-0 h-4 bg-linear-to-t from-raised to-transparent" aria-hidden />
      </div>

      {transition ? (
        <div className="shrink-0 border-t border-line px-4 py-2.5">
          <button
            type="button"
            onClick={() => onAdvance(transition.next)}
            className="w-full rounded-md bg-accent-solid px-3 py-1.5 text-[12px] font-medium text-accent-solid-ink transition-opacity hover:opacity-90"
          >
            {transition.label}
          </button>
        </div>
      ) : null}
    </div>
  );
}

function ChipButton({
  label,
  onClick,
  accept,
  className,
  children,
}: {
  label: string;
  onClick: () => void;
  /** Marks the accept button so the autoplay pointer can find it. */
  accept?: string;
  className?: string;
  children: string;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      data-accept={accept}
      className={cx("-mr-0.5 grid size-4 place-items-center rounded text-[11.5px] leading-none transition-colors", className)}
    >
      <span aria-hidden>{children}</span>
    </button>
  );
}

/** Summary, or a quiet placeholder while the write-up runs. The skeleton overlays the reserved text, so nothing jumps. */
function Summary({ text }: { text: string | null }) {
  const ready = text !== null;
  return (
    <div>
      <div className="mb-1 flex items-center justify-between gap-2">
        <p className="text-[11px] font-medium text-subtle">Summary</p>
        {ready ? null : (
          <p className="flex items-center gap-1.5 text-[11px] text-subtle">
            <PulseDot />
            Writing in the background
          </p>
        )}
      </div>
      <div className="relative">
        <p
          className={cx(
            "text-[11.5px] leading-[1.6] text-muted transition-opacity duration-500 ease-out motion-reduce:transition-none",
            ready ? "opacity-100" : "opacity-0",
          )}
          aria-hidden={!ready}
        >
          {/* Before it lands, the sample text only reserves the space. */}
          {text ?? SAMPLE.summary}
        </p>
        <div
          className={cx("absolute inset-0 space-y-2.5 overflow-hidden pt-1.5 transition-opacity duration-300 motion-reduce:transition-none", ready ? "opacity-0" : "opacity-100")}
          aria-hidden
        >
          {["w-full", "w-11/12", "w-full", "w-4/5", "w-2/3"].map((w, i) => (
            <span key={i} className={cx("block h-2 rounded-full bg-line", w)} />
          ))}
        </div>
      </div>
    </div>
  );
}
