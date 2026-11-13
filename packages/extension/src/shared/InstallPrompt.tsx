import type { ReactNode } from "react";
import AgentMarkMark from "./AgentMarkMark";
import AppMissing, { WaitingForApp } from "./AppMissing";
import CopyCommand from "./CopyCommand";
import { SETUP_GUIDE_URL } from "./constants";
import { registerCommand, type ConnectionProblem } from "./connection";
import { runtimeExtensionId } from "./usePlatformInfo";

interface InstallPromptProps {
  problem: ConnectionProblem;
  /** "Check again" for a missing or unregistered app (the surface also polls), else "Try again". */
  onRetry: () => void;
  /** The raw error, shown collapsed for troubleshooting. */
  detail?: string;
}

const focusRing = "focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1";

/**
 * Tells the user how to get the extension talking to the AgentMark app, one
 * variant per connection problem. Used by the popup and side panel. A missing
 * app gets the platform list (screens A and B).
 */
export default function InstallPrompt({ problem, onRetry, detail }: InstallPromptProps) {
  if (problem === "not_installed") {
    return (
      <div className="min-w-0" data-testid="install-prompt" data-problem={problem}>
        <AppMissing onCheckAgain={onRetry} detail={detail} />
      </div>
    );
  }

  const register = registerCommand(runtimeExtensionId());
  const reloadStep = (
    <>
      Reload this extension at <span className="font-mono text-[12px]">chrome://extensions</span>, then try again.
    </>
  );

  let heading: string;
  let intro: string;
  let steps: ReactNode[];
  switch (problem) {
    case "forbidden":
      heading = "Connect this extension";
      intro = "The AgentMark app is installed, but it is set up for a different copy of this extension.";
      steps = [
        <>Open Terminal and paste this:<CopyCommand command={register} label="connect command" /></>,
        <>
          Come back here. This updates by itself. If it does not, reload this extension at{" "}
          <span className="font-mono text-[12px]">chrome://extensions</span>.
        </>,
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
  const crashed = problem === "host_crashed";
  const polls = problem === "forbidden";

  return (
    <div
      className={`min-w-0 ${crashed ? "rounded-lg border border-amber-200 bg-amber-50 p-3" : polls ? "" : "rounded-lg border border-gray-200 bg-white p-3"}`}
      data-testid="install-prompt"
      data-problem={problem}
    >
      <div className="flex items-center gap-2">
        {!crashed && <AgentMarkMark look="idle" size={18} />}
        <h2 className={`font-semibold text-gray-900 ${polls ? "text-[15px]" : "text-sm"}`}>{heading}</h2>
      </div>
      <p className={`mt-1 text-xs leading-snug ${crashed ? "text-amber-900" : "text-gray-600"}`}>{intro}</p>
      <ol className="mt-2.5 space-y-2.5">
        {steps.map((step, i) => (
          <li key={i} className="flex min-w-0 gap-2 text-xs leading-snug text-gray-800">
            <span
              aria-hidden="true"
              className={`flex h-[18px] w-[18px] shrink-0 items-center justify-center rounded-full text-[11px] font-semibold ${
                crashed ? "bg-amber-100 text-amber-800" : "bg-indigo-50 text-indigo-600"
              }`}
            >
              {i + 1}
            </span>
            <div className="min-w-0 flex-1 pt-px">{step}</div>
          </li>
        ))}
      </ol>
      {polls ? (
        <WaitingForApp onCheckAgain={onRetry} />
      ) : (
        <div className="mt-3 flex items-center gap-3">
          <button
            type="button"
            onClick={onRetry}
            className={`rounded-md bg-indigo-600 px-3 py-1 text-xs font-medium text-white hover:bg-indigo-700 ${focusRing}`}
            data-testid="install-retry"
          >
            Try again
          </button>
          <a
            href={SETUP_GUIDE_URL}
            target="_blank"
            rel="noreferrer"
            className={`ml-auto rounded text-xs text-indigo-600 hover:text-indigo-800 ${focusRing}`}
            data-testid="setup-guide-link"
          >
            Setup guide
          </a>
        </div>
      )}
      {polls ? (
        <div className="mt-3 flex items-center justify-between gap-3 text-[11px] text-gray-400">
          {detail ? (
            <details className="min-w-0">
              <summary className="cursor-pointer rounded hover:text-gray-600">Error details</summary>
              <p className="mt-1 break-words" data-testid="install-prompt-detail">{detail}</p>
            </details>
          ) : (
            <span />
          )}
          <a
            href={SETUP_GUIDE_URL}
            target="_blank"
            rel="noreferrer"
            className={`shrink-0 self-start rounded hover:text-gray-600 ${focusRing}`}
            data-testid="setup-guide-link"
          >
            Setup guide
          </a>
        </div>
      ) : (
        detail && (
          <details className="mt-2 text-[11px] text-gray-400">
            <summary className="cursor-pointer">Error details</summary>
            <p className="mt-1 break-words" data-testid="install-prompt-detail">{detail}</p>
          </details>
        )
      )}
    </div>
  );
}
