import { SAMPLE } from "./kit";

const MORE = ["The case for boring formats", "A folder is a database", "Exporting is a habit"];

/**
 * The sample essay laid out around the toolbar popup, which hangs from the
 * top-right corner. Wide: the popup sits over the sidebar. Narrow: it sits
 * over the cover, and the headline starts below it. Needs an `@container`
 * ancestor the width of the browser frame.
 */
export function HeroArticle() {
  return (
    <div className="h-full bg-surface" aria-hidden>
      <div className="flex items-center justify-between border-b border-line px-5 py-3 @min-[540px]:px-8">
        <span className="font-serif text-[13px] font-semibold tracking-tight text-ink">{SAMPLE.site}</span>
        <span className="hidden gap-4 text-[11px] text-subtle @min-[540px]:flex">
          <span>Essays</span>
          <span>Archive</span>
          <span>About</span>
        </span>
      </div>

      <div className="px-5 @min-[540px]:grid @min-[540px]:grid-cols-[minmax(0,1fr)_248px] @min-[540px]:gap-7 @min-[540px]:px-8">
        <Cover />
        <article className="pt-4 @min-[540px]:pt-9">
          <p className="text-[11px] font-medium uppercase tracking-[0.12em] text-subtle">Essay · 9 min read</p>
          <h3 className="mt-2 max-w-[18ch] font-serif text-[22px] font-semibold leading-tight tracking-tight text-ink @min-[540px]:text-[28px] @min-[700px]:text-[32px]">
            {SAMPLE.title}
          </h3>
          <div className="mt-4 space-y-3 font-serif text-[13px] leading-relaxed text-muted @min-[540px]:text-[14.5px]">
            <p>
              Every note app I have used is gone or changed beyond recognition. The notes are still here, because the
              last thing I did with each app was export to a folder of text files.
            </p>
            <p className="hidden @min-[540px]:block">
              This essay looks at three of those moves, what broke in each, and the one format that came through all of
              them unchanged.
            </p>
            <div className="space-y-2 pt-1">
              <div className="h-2 w-full rounded-full bg-sunken" />
              <div className="h-2 w-11/12 rounded-full bg-sunken" />
              <div className="h-2 w-4/5 rounded-full bg-sunken" />
            </div>
          </div>
        </article>
        <aside className="hidden pt-9 @min-[540px]:block">
          <p className="text-[11px] font-medium uppercase tracking-[0.12em] text-subtle">More essays</p>
          <ul className="mt-3 divide-y divide-line border-y border-line">
            {MORE.map((title) => (
              <li key={title} className="py-2.5 font-serif text-[13px] leading-snug text-muted">
                {title}
              </li>
            ))}
          </ul>
        </aside>
      </div>
    </div>
  );
}

/** Narrow layouts only: a quiet cover for the popup to sit on, so it never lands on the headline. */
function Cover() {
  return (
    <div className="mt-4 flex h-[116px] items-end rounded-lg bg-sunken px-4 pb-2 @min-[540px]:hidden">
      <span className="font-serif text-[44px] leading-none text-line-strong">&para;</span>
    </div>
  );
}
