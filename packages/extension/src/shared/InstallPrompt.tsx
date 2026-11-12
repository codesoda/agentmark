import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  INSTALL_COMMAND,
  SETUP_GUIDE_URL,
  registerCommand,
  type ConnectionProblem,
} from "./connection";

interface InstallPromptProps {
  problem: ConnectionProblem;
  onRetry: () => void;
  /** The raw error, shown collapsed for troubleshooting. */
  detail?: string;
}

const COPIED_MS = 1500;

function extensionId(): string | undefined {
  return typeof chrome !== "undefined" ? chrome.runtime?.id : undefined;
}

/**
 * Tells the user how to get the extension talking to the AgentMark CLI,
 * one variant per connection problem. Used by the popup and side panel.
 */
export default function InstallPrompt({ problem, onRetry, detail }: InstallPromptProps) {
  const register = registerCommand(extensionId());
  const reloadStep = (
    <>
      Reload this extension at <span className="font-mono">chrome://extensions</span>, then try again.
    </>
  );

  let heading: string;
  let intro: string;
  let steps: ReactNode[];
  switch (problem) {
    case "not_installed":
      heading = "Install the AgentMark app";
      intro = "This extension needs the AgentMark command-line app on this computer, and it was not found.";
      steps = [
        <>Install the app:<CopyCommand command={INSTALL_COMMAND} label="install command" /></>,
        <>Connect this extension:<CopyCommand command={register} label="connect command" /></>,
        reloadStep,
      ];
      break;
    case "forbidden":
      heading = "Connect this extension to AgentMark";
      intro = "The AgentMark app is installed, but it is set up for a different copy of this extension.";
      steps = [
        <>Connect this extension:<CopyCommand command={register} label="connect command" /></>,
        reloadStep,
      ];
      break;
    case "worker_unreachable":
      heading = "Reload AgentMark";
      intro = "The extension's background service stopped responding.";
      steps = [reloadStep];
      break;
    case "host_crashed":
      heading = "Couldn't reach AgentMark";
      intro = "The connection to the AgentMark app was lost.";
      steps = [
        reloadStep,
        <>If it still fails, connect this extension again and reload:<CopyCommand command={register} label="connect command" /></>,
      ];
      break;
  }

  // A lost connection is a real failure; the other kinds are setup steps.
  const tone =
    problem === "host_crashed"
      ? "border-amber-200 bg-amber-50 text-amber-900"
      : "border-indigo-100 bg-indigo-50/60 text-gray-700";

  return (
    <div
      className={`min-w-0 rounded-md border p-3 text-xs ${tone}`}
      data-testid="install-prompt"
      data-problem={problem}
    >
      <h2 className="text-sm font-semibold text-gray-900">{heading}</h2>
      <p className="mt-1">{intro}</p>
      <ol className="mt-2 list-decimal space-y-2 pl-4">
        {steps.map((step, i) => (
          <li key={i} className="min-w-0">{step}</li>
        ))}
      </ol>
      <div className="mt-3 flex items-center gap-3">
        <button
          type="button"
          onClick={onRetry}
          className="rounded bg-indigo-600 px-3 py-1 text-xs font-medium text-white hover:bg-indigo-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1"
          data-testid="install-retry"
        >
          Try again
        </button>
        <a
          href={SETUP_GUIDE_URL}
          target="_blank"
          rel="noreferrer"
          className="text-indigo-600 hover:text-indigo-800"
          data-testid="setup-guide-link"
        >
          Setup guide
        </a>
      </div>
      {detail && (
        <details className="mt-2 text-[11px] text-gray-400">
          <summary className="cursor-pointer">Error details</summary>
          <p className="mt-1 break-words" data-testid="install-prompt-detail">{detail}</p>
        </details>
      )}
    </div>
  );
}

/** A command in a wrapping code block with a copy button. */
function CopyCommand({ command, label }: { command: string; label: string }) {
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
    <div className="mt-1 flex items-start gap-1 rounded bg-white/80 p-1.5 ring-1 ring-gray-200">
      <code className="min-w-0 flex-1 select-all whitespace-pre-wrap break-all font-mono text-[11px] text-gray-800">
        {command}
      </code>
      <button
        type="button"
        onClick={() => void copy()}
        aria-label={`Copy ${label}`}
        className="shrink-0 rounded px-1.5 py-0.5 text-[11px] text-indigo-600 hover:bg-indigo-50 hover:text-indigo-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
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
