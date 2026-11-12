import { useEffect, useRef, useState } from "react";
import { commandPieces } from "./command-pieces";

const COPIED_MS = 1500;

/**
 * A terminal command in a block that wraps (never scrolls the page sideways)
 * with a copy button. The command stays selectable when the clipboard fails.
 * Lines break after `/` and at spaces; only a piece wider than the block is
 * split mid-word.
 */
export default function CopyCommand({ command, label }: { command: string; label: string }) {
  const [status, setStatus] = useState<"idle" | "copied" | "failed">("idle");
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => () => {
    if (timer.current) clearTimeout(timer.current);
  }, []);

  const copy = async () => {
    let next: "copied" | "failed" = "copied";
    try {
      await navigator.clipboard.writeText(command);
    } catch {
      // Clipboard denied or unavailable; the command stays selectable.
      next = "failed";
    }
    setStatus(next);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setStatus("idle"), COPIED_MS);
  };

  return (
    <div className="mt-1.5 flex min-w-0 items-start gap-2 rounded-lg border border-gray-200 bg-gray-50 py-2 pl-2.5 pr-1.5">
      <code
        className="min-w-0 flex-1 select-all whitespace-pre-wrap font-mono text-[11.5px] leading-[1.45] text-gray-800 [overflow-wrap:anywhere]"
        data-testid="command-text"
      >
        {commandPieces(command).map((piece, i) => (
          <span key={i}>
            {i > 0 && <wbr />}
            {piece}
          </span>
        ))}
      </code>
      <button
        type="button"
        onClick={() => void copy()}
        aria-label={`Copy ${label}`}
        className="shrink-0 rounded-md border border-gray-200 bg-white px-2 py-0.5 text-[11px] font-medium text-gray-700 shadow-sm hover:bg-gray-100 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
        data-testid="copy-command"
      >
        {status === "copied" ? "Copied" : "Copy"}
      </button>
      <span className="sr-only" aria-live="polite">
        {status === "copied" ? "Copied" : status === "failed" ? "Copy failed. Select the command to copy it." : ""}
      </span>
    </div>
  );
}
