"use client";

import { useId, type ReactNode } from "react";

export type Post = {
  key: string;
  initials: string;
  name: string;
  meta: string;
  body: string;
  /** Bookmark title the save uses until the agent writes a better one. */
  title: string;
  /** Bookmark ID the popup shows once saved. */
  id: string;
  link?: { host: string; title: string };
  /** Only signed-in members can see it: a plain fetch would get a login page. */
  membersOnly?: boolean;
  /** Already saved when the page loaded. */
  savedBefore?: boolean;
  replies: number;
  shares: number;
};

function Glyph({ d }: { d: string }) {
  return (
    <svg viewBox="0 0 16 16" className="size-3.5" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d={d} />
    </svg>
  );
}

const REPLY = "M3 3.5h10v7H7.5L4.5 13v-2.5H3z";
const SHARE = "M8 2.5v7.5M5 5.5l3-3 3 3M3.5 9.5v4h9v-4";
const LOCK = "M5 7V5.25a3 3 0 0 1 6 0V7M4 7h8v6H4z";

/** A post in a generic feed, with the save badge at the end of its action row. */
export function FeedPost({ post, children }: { post: Post; children: (describedBy: string) => ReactNode }) {
  const nameId = useId();
  return (
    <article className="rounded-lg border border-line bg-surface px-3.5 pt-3 @min-[400px]:px-4" aria-labelledby={nameId}>
      <header className="flex items-start gap-2.5">
        <span
          className="grid size-8 shrink-0 place-items-center rounded-full bg-raised text-[10.5px] font-semibold text-muted ring-1 ring-line"
          aria-hidden
        >
          {post.initials}
        </span>
        <div className="min-w-0 flex-1">
          <p id={nameId} className="truncate text-[13px] font-semibold leading-tight text-ink">
            {post.name}
          </p>
          <p className="mt-0.5 flex items-center gap-1 truncate text-[11.5px] leading-tight text-subtle">
            {post.membersOnly ? (
              <span className="inline-flex shrink-0 items-center gap-1 rounded bg-sunken px-1 py-px text-[10.5px] font-medium text-muted">
                <Glyph d={LOCK} />
                Members only
              </span>
            ) : null}
            <span className="truncate">{post.meta}</span>
          </p>
        </div>
      </header>
      <p className="mt-2 text-[12.5px] leading-normal text-muted @min-[400px]:text-[13px]">{post.body}</p>
      {post.link ? (
        <div className="mt-2 hidden items-baseline gap-2 rounded-md border border-line bg-raised px-2.5 py-1.5 @min-[400px]:flex">
          <p className="min-w-0 truncate text-[12px] font-medium text-ink">{post.link.title}</p>
          <p className="ml-auto shrink-0 font-mono text-[10.5px] text-subtle">{post.link.host}</p>
        </div>
      ) : null}
      <div className="mt-1 flex items-center gap-4 text-[11.5px] text-subtle">
        <span className="flex items-center gap-1.5">
          <Glyph d={REPLY} />
          {post.replies}
          <span className="sr-only">replies</span>
        </span>
        <span className="flex items-center gap-1.5">
          <Glyph d={SHARE} />
          {post.shares}
          <span className="sr-only">shares</span>
        </span>
        <span className="-mr-1.5 ml-auto py-1">{children(nameId)}</span>
      </div>
    </article>
  );
}
