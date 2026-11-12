import { SAMPLE } from "./sample";

/** A calm, generic article page used inside BrowserFrame demos. */
export function MockArticle({ compact = false }: { compact?: boolean }) {
  return (
    <div className="h-full bg-surface">
      <div className="flex items-center justify-between border-b border-line px-5 py-3 sm:px-8">
        <span className="font-serif text-[13px] font-semibold tracking-tight text-ink">{SAMPLE.site}</span>
        <span className={(compact ? "hidden" : "hidden sm:flex") + " gap-4 text-[11px] text-subtle"}>
          <span>Essays</span>
          <span>Archive</span>
          <span>About</span>
        </span>
      </div>
      <article className={compact ? "px-5 pt-5" : "max-w-xl px-5 pt-8 sm:px-10 sm:pt-12"}>
        <p className="text-[11px] font-medium uppercase tracking-[0.12em] text-subtle">Essay · 9 min read</p>
        <h3
          className={
            "mt-2 font-serif font-semibold leading-tight tracking-tight text-ink " +
            (compact ? "text-[20px]" : "max-w-[20ch] text-[22px] sm:text-[30px]")
          }
        >
          {SAMPLE.title}
        </h3>
        <div
          className={
            "mt-4 space-y-3 font-serif leading-relaxed text-muted " + (compact ? "text-[12.5px]" : "text-[13px] sm:text-[15px]")
          }
        >
          <p>
            Every note app I have used is gone or changed beyond recognition. The notes are still here, because the
            last thing I did with each app was export to a folder of text files.
          </p>
          <p className={compact ? "hidden" : "hidden sm:block"}>
            This essay looks at three of those moves, what broke in each, and the one format that came through all of
            them unchanged.
          </p>
          <div className="space-y-2 pt-1" aria-hidden>
            <div className="h-2 w-full rounded-full bg-sunken" />
            <div className="h-2 w-11/12 rounded-full bg-sunken" />
            <div className="h-2 w-4/5 rounded-full bg-sunken" />
          </div>
        </div>
      </article>
    </div>
  );
}
