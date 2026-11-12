import type { ReactNode } from "react";
import { AgentMarkMark, type MarkLook } from "./agentmark-mark";
import { cx } from "./cx";

function TrafficLights({ tone = "chrome" }: { tone?: "chrome" | "term" }) {
  const dot = tone === "term" ? "bg-term-line" : "bg-line-strong";
  return (
    <div className="flex shrink-0 items-center gap-1.5" aria-hidden>
      <span className={cx("size-2.5 rounded-full", dot)} />
      <span className={cx("size-2.5 rounded-full", dot)} />
      <span className={cx("size-2.5 rounded-full", dot)} />
    </div>
  );
}

function Icon({ d, className }: { d: string; className?: string }) {
  return (
    <svg viewBox="0 0 16 16" className={cx("size-3.5", className)} fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d={d} />
    </svg>
  );
}

/**
 * A neutral browser window. Not a copy of any real browser's chrome.
 * Fills its parent's height; give the parent a fixed height.
 */
export function BrowserFrame({
  url,
  tabTitle,
  markLook = "idle",
  extension,
  overlay,
  children,
  className,
  bodyClassName,
  label,
}: {
  /** Shown in the address field, without the scheme. */
  url: string;
  tabTitle: string;
  /** Look of the AgentMark toolbar icon. Ignored when `extension` is given. */
  markLook?: MarkLook;
  /** Replaces the toolbar icon, e.g. an interactive button. */
  extension?: ReactNode;
  /** Absolutely positioned layer over the page, anchored top-right under the toolbar (popup, side panel). */
  overlay?: ReactNode;
  children?: ReactNode;
  className?: string;
  bodyClassName?: string;
  /** Accessible description of the whole mock. */
  label?: string;
}) {
  return (
    <div
      className={cx(
        "flex h-full w-full flex-col overflow-hidden rounded-xl border border-line-strong bg-surface shadow-frame",
        className,
      )}
      role={label ? "img" : undefined}
      aria-label={label}
    >
      <div className="flex h-9 shrink-0 items-end gap-3 bg-chrome px-3.5">
        <div className="flex h-9 items-center">
          <TrafficLights />
        </div>
        <div className="flex h-7 min-w-0 max-w-[14rem] items-center gap-2 rounded-t-lg bg-surface px-3 text-[11px] font-medium text-muted">
          <span className="size-2.5 shrink-0 rounded-sm bg-line-strong" aria-hidden />
          <span className="truncate">{tabTitle}</span>
        </div>
      </div>
      <div className="flex h-10 shrink-0 items-center gap-2 border-b border-line bg-surface px-3 text-chrome-ink">
        <Icon d="M10 3.5 5.5 8l4.5 4.5" />
        <Icon d="M6 3.5 10.5 8 6 12.5" className="hidden opacity-50 sm:block" />
        <Icon d="M12.5 8a4.5 4.5 0 1 1-1.3-3.2M12.5 3v2.5H10" className="hidden sm:block" />
        <div className="flex h-7 min-w-0 flex-1 items-center gap-2 rounded-full bg-sunken px-3 text-[12px] text-muted">
          <Icon d="M5 7V5.5a3 3 0 0 1 6 0V7M4.5 7h7v5.5h-7z" className="size-3 shrink-0" />
          <span className="truncate font-mono text-[11.5px] tracking-tight">{url}</span>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <span className="grid size-7 place-items-center" aria-hidden>
            <Icon d="M6.5 2.5h3v2a1 1 0 0 0 2 0h0v0h2v3h-1.5a1.25 1.25 0 0 0 0 2.5H13.5v3.5h-3.5v-1.5a1.25 1.25 0 0 0-2.5 0v1.5H3.5V10h1.5a1.25 1.25 0 0 0 0-2.5H3.5v-3h3z" className="opacity-60" />
          </span>
          {extension ?? (
            <span className="grid size-7 place-items-center rounded-md">
              <AgentMarkMark look={markLook} size={16} />
            </span>
          )}
        </div>
      </div>
      <div className={cx("relative min-h-0 flex-1 overflow-hidden", bodyClassName)}>
        {children}
        {overlay ? <div className="absolute right-2 top-1.5 z-10 sm:right-3">{overlay}</div> : null}
      </div>
    </div>
  );
}

/** A plain app window: title bar plus content. Fills its parent's height. */
export function WindowFrame({
  title,
  children,
  className,
  bodyClassName,
  actions,
  label,
}: {
  title?: ReactNode;
  children?: ReactNode;
  className?: string;
  bodyClassName?: string;
  /** Right side of the title bar. */
  actions?: ReactNode;
  label?: string;
}) {
  return (
    <div
      className={cx(
        "flex h-full w-full flex-col overflow-hidden rounded-xl border border-line-strong bg-surface shadow-frame",
        className,
      )}
      role={label ? "img" : undefined}
      aria-label={label}
    >
      <div className="relative flex h-9 shrink-0 items-center gap-3 border-b border-line bg-chrome px-3.5">
        <TrafficLights />
        {title ? (
          <div className="pointer-events-none absolute inset-x-16 truncate text-center text-[11.5px] font-medium text-chrome-ink">
            {title}
          </div>
        ) : null}
        <div className="ml-auto flex items-center gap-2">{actions}</div>
      </div>
      <div className={cx("relative min-h-0 flex-1 overflow-hidden", bodyClassName)}>{children}</div>
    </div>
  );
}

/** A terminal window. Dark in both themes. Fills its parent's height. */
export function TerminalFrame({
  title = "zsh",
  children,
  className,
  bodyClassName,
  label,
}: {
  title?: ReactNode;
  children?: ReactNode;
  className?: string;
  bodyClassName?: string;
  label?: string;
}) {
  return (
    <div
      className={cx(
        "flex h-full w-full flex-col overflow-hidden rounded-xl border border-term-line bg-term-bg text-term-ink shadow-frame",
        className,
      )}
      role={label ? "img" : undefined}
      aria-label={label}
    >
      <div className="relative flex h-9 shrink-0 items-center border-b border-term-line bg-term-chrome px-3.5">
        <TrafficLights tone="term" />
        <div className="pointer-events-none absolute inset-x-16 truncate text-center font-mono text-[11px] text-term-muted">
          {title}
        </div>
      </div>
      <div
        className={cx(
          "relative min-h-0 flex-1 overflow-hidden p-4 font-mono text-[12.5px] leading-[1.65] sm:p-5 sm:text-[13px]",
          bodyClassName,
        )}
      >
        {children}
      </div>
    </div>
  );
}
