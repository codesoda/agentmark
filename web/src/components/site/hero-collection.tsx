import type { ReactNode } from "react";

/*
 * The hero's "Local collection" card, the same picture as the share image
 * (src/app/opengraph-image.tsx). The three logos use each site's own brand
 * colours, like the AgentMark mark; everything else uses theme tokens.
 */

const BOOKMARK =
  "M9.057 5.795H14.943A1.67 1.67 0 0 1 16.614 7.466V17.695Q16.614 18.205 16.105 18.205H15.834Q15.58 18.205 15.401 18.026L12 14.625L8.599 18.026Q8.42 18.205 8.166 18.205H7.895Q7.386 18.205 7.386 17.695V7.466A1.67 1.67 0 0 1 9.057 5.795Z";

function Logo({ children }: { children: ReactNode }) {
  return (
    <svg viewBox="0 0 24 24" width="36" height="36" aria-hidden className="shrink-0">
      {children}
    </svg>
  );
}

const SAVED_ITEMS: Array<{ site: string; title: string; tag: string; logo: ReactNode }> = [
  {
    site: "LinkedIn · Post",
    title: "A post to revisit",
    tag: "agents",
    logo: (
      <Logo>
        <rect x="2" y="2" width="20" height="20" rx="4.5" fill="#0A66C2" />
        <circle cx="7.4" cy="8" r="1.35" fill="#fff" />
        <path
          d="M6.1 10.1h2.6v7.8H6.1zM10.1 10.1h2.5v1.1c.6-.9 1.5-1.4 2.8-1.4 2.3 0 3.4 1.4 3.4 4v4.1h-2.6v-3.7c0-1.2-.5-1.9-1.6-1.9-1.2 0-1.9.8-1.9 2v3.6h-2.6z"
          fill="#fff"
        />
      </Logo>
    ),
  },
  {
    site: "YouTube · Video",
    title: "A video to revisit",
    tag: "captions",
    logo: (
      <Logo>
        <rect x="2" y="4.5" width="20" height="15" rx="4.5" fill="#FF0033" />
        <path d="M10 8.5v7l5.8-3.5z" fill="#fff" />
      </Logo>
    ),
  },
  {
    site: "Reddit · Page",
    title: "A thread worth keeping",
    tag: "ideas",
    logo: (
      <Logo>
        <rect x="2" y="2" width="20" height="20" rx="5" fill="#FF4500" />
        <circle cx="6.2" cy="13" r="1.4" fill="#fff" />
        <circle cx="17.8" cy="13" r="1.4" fill="#fff" />
        <circle cx="12" cy="13.2" r="6" fill="#fff" />
        <path d="M12 7.2 13.4 4l2.8.7" stroke="#fff" strokeWidth="1.3" strokeLinecap="round" fill="none" />
        <circle cx="17" cy="5" r="1.25" fill="#fff" />
        <circle cx="9.7" cy="12.3" r="0.7" fill="#FF4500" />
        <circle cx="14.3" cy="12.3" r="0.7" fill="#FF4500" />
        <path d="M9.7 15Q12 17 14.3 15" stroke="#FF4500" strokeWidth="1.2" strokeLinecap="round" fill="none" />
      </Logo>
    ),
  },
];

export function HeroCollection() {
  return (
    <div
      role="img"
      aria-label="A local collection of saved items: a LinkedIn post, a YouTube video and a Reddit thread, each with a tag."
      className="w-full max-w-[30rem] rounded-[22px] border border-line-strong bg-surface p-5 shadow-frame sm:p-6"
    >
      <div className="flex items-center justify-between gap-4">
        <div>
          <p className="font-mono text-[11px] font-semibold uppercase tracking-[0.14em] text-subtle">Your saved items</p>
          <p className="mt-1 text-[19px] font-semibold tracking-[-0.01em] text-ink">Local collection</p>
        </div>
        <span className="grid size-[42px] shrink-0 place-items-center rounded-xl bg-accent-soft">
          <svg viewBox="1.5 1.5 21 21" width="25" height="25" aria-hidden className="text-accent">
            <path d={BOOKMARK} fill="currentColor" />
          </svg>
        </span>
      </div>
      <ul className="mt-4 space-y-3.5">
        {SAVED_ITEMS.map((item) => (
          <li key={item.site} className="flex items-center gap-3 rounded-xl border border-line bg-raised px-3.5 py-3">
            {item.logo}
            <div className="min-w-0 flex-1">
              <p className="truncate font-mono text-[11px] uppercase text-subtle">{item.site}</p>
              <p className="mt-0.5 truncate text-[15px] font-semibold text-ink">{item.title}</p>
            </div>
            <span className="shrink-0 rounded-lg bg-accent-soft px-2 py-1 text-[12px] text-accent">{item.tag}</span>
          </li>
        ))}
      </ul>
      <div className="mt-4 flex justify-between gap-4 font-mono text-[11px] uppercase tracking-[0.06em] text-subtle">
        <span>Full-text search</span>
        <span>AI-suggested tags</span>
      </div>
    </div>
  );
}
