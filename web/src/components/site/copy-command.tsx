"use client";

import { useEffect, useRef, useState } from "react";

/**
 * A shell command with a copy button. The status is announced politely to
 * screen readers. Falls back to selecting the text if the clipboard API is
 * not available.
 */
export function CopyCommand({
  command,
  label = "Copy command",
  size = "md",
  wrap = false,
}: {
  command: string;
  label?: string;
  size?: "md" | "lg";
  /** Let a long command wrap at "/" and spaces instead of scrolling. */
  wrap?: boolean;
}) {
  const [copied, setCopied] = useState<"idle" | "ok" | "fail">("idle");
  const codeRef = useRef<HTMLElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
  }, []);

  async function copy() {
    try {
      await navigator.clipboard.writeText(command);
      setCopied("ok");
    } catch {
      const el = codeRef.current;
      if (el) {
        const range = document.createRange();
        range.selectNodeContents(el);
        const sel = window.getSelection();
        sel?.removeAllRanges();
        sel?.addRange(range);
      }
      setCopied("fail");
    }
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setCopied("idle"), 2000);
  }

  const lg = size === "lg";
  return (
    <div
      className={
        "group flex w-full min-w-0 items-stretch overflow-hidden rounded-lg border border-line-strong bg-surface " +
        (lg ? "text-[13px] sm:text-[14px]" : "text-[12.5px] sm:text-[13px]")
      }
    >
      <div
        className={
          "flex min-w-0 flex-1 items-start gap-3 " +
          (wrap
            ? ""
            : "overflow-x-auto [scrollbar-width:none] [mask-image:linear-gradient(to_right,black_calc(100%-2.5rem),transparent)] ") +
          (lg ? "px-4 py-3.5" : "px-3.5 py-2.5")
        }
        {...(wrap ? {} : { tabIndex: 0, role: "region", "aria-label": "Command" })}
      >
        <span className="select-none font-mono text-subtle" aria-hidden>
          $
        </span>
        <code ref={codeRef} className={"font-mono text-ink " + (wrap ? "min-w-0 flex-1 leading-relaxed" : "whitespace-nowrap pr-8")}>
          {wrap ? withBreaks(command) : command}
        </code>
      </div>
      <button
        type="button"
        onClick={copy}
        className="relative flex shrink-0 items-center gap-1.5 border-l border-line-strong bg-raised px-3.5 text-[12.5px] font-medium text-muted transition-colors hover:bg-sunken hover:text-ink focus-visible:-outline-offset-2"
        aria-label={label}
      >
        {copied === "ok" ? <CheckIcon /> : <CopyIcon />}
        <span className="hidden sm:inline">{copied === "ok" ? "Copied" : copied === "fail" ? "Selected" : "Copy"}</span>
      </button>
      <span className="sr-only" role="status" aria-live="polite">
        {copied === "ok" ? "Copied to clipboard" : copied === "fail" ? "Copy failed. The command is selected; press Command or Control plus C." : ""}
      </span>
    </div>
  );
}

/**
 * Lines break after each "/" and at spaces, like the extension's command
 * block, never at a hyphen inside a flag such as `--extension-id`. Only a
 * piece too long to keep whole may break inside.
 */
function withBreaks(command: string) {
  const pieces = command.match(/[^/\s]*(?:\/+|\s+|$)/g)?.filter((piece) => piece !== "") ?? [command];
  return pieces.map((piece, i) => (
    <span key={i}>
      {i > 0 ? <wbr /> : null}
      {piece.length <= 20 ? (
        <span className="whitespace-nowrap">{piece}</span>
      ) : (
        // A long host name may also break after a dot, and anywhere as a last resort.
        <span className="[overflow-wrap:anywhere]">
          {piece.split(/(?<=\.)/).map((part, j) => (
            <span key={j}>
              {j > 0 ? <wbr /> : null}
              {part}
            </span>
          ))}
        </span>
      )}
    </span>
  ));
}

function CopyIcon() {
  return (
    <svg viewBox="0 0 16 16" className="size-3.5" fill="none" stroke="currentColor" strokeWidth="1.4" aria-hidden>
      <rect x="5.25" y="5.25" width="8" height="8" rx="1.5" />
      <path d="M10.75 5.25V3.75a1.5 1.5 0 0 0-1.5-1.5h-5.5a1.5 1.5 0 0 0-1.5 1.5v5.5a1.5 1.5 0 0 0 1.5 1.5h1.5" />
    </svg>
  );
}

function CheckIcon() {
  return (
    <svg viewBox="0 0 16 16" className="size-3.5 text-accent" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M3.5 8.5 6.5 11.5 12.5 4.5" />
    </svg>
  );
}
