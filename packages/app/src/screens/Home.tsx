/**
 * Welcome and install progress (spec §5): the three launch steps with ticks,
 * then the Chrome extension's connection. On later launches the same screen
 * says AgentMark is ready.
 */

import { api } from "../api";
import AgentMarkMark from "../components/AgentMarkMark";
import type { View } from "../components/Header";
import Screen from "../components/Screen";
import { StatusRow, type RowState } from "../components/StatusRow";
import { card, heading, lede, linkButton, primaryButton, secondaryButton } from "../components/ui";
import { Spinner } from "../components/controls";
import { STEP_ORDER, STEP_TITLES, type StepState } from "../steps";
import { isLiveConnection, lastSeenText } from "../connection";
import type { ConnectionRecord, SetupState, StepId } from "../types";

const RUNNING_SUMMARY: Record<StepId, string> = {
  cli: "Copying it to ~/.agentmark/bin…",
  native_host: "Letting Chrome start AgentMark…",
  skill: "Teaching your agents to search your bookmarks…",
};

function rowState(step: StepState): RowState {
  return typeof step === "string" ? step : step.status;
}

type Phase = "working" | "attention" | "setup" | "extension" | "ready";

function phaseOf(steps: Record<StepId, StepState>, state: SetupState | null, connection: ConnectionRecord | null): Phase {
  const values = STEP_ORDER.map((id) => steps[id]);
  if (values.some((s) => typeof s === "string") || !state) return "working";
  if (values.some((s) => typeof s !== "string" && s.status === "failed")) return "attention";
  if (!state.initialized) return "setup";
  if (!connection) return "extension";
  return "ready";
}

const HERO: Record<Phase, { title: string; text: string }> = {
  working: { title: "Welcome to AgentMark", text: "Getting this computer ready. This takes a few seconds." },
  attention: { title: "Something needs attention", text: "A step did not finish. Try it again, or see the details below." },
  setup: { title: "Welcome to AgentMark", text: "The app is installed. Next, two choices: where bookmarks live, and which agent writes summaries." },
  extension: { title: "Almost there", text: "Add the Chrome extension, then save pages from Chrome." },
  ready: { title: "AgentMark is ready", text: "Save pages from Chrome. Your agent can search and read them." },
};

export default function Home({
  steps,
  state,
  stateError,
  connection,
  onRetry,
  onReloadState,
  onNavigate,
}: {
  steps: Record<StepId, StepState>;
  state: SetupState | null;
  stateError: string | null;
  connection: ConnectionRecord | null;
  onRetry: (id: StepId) => void;
  onReloadState: () => void;
  onNavigate: (view: View) => void;
}) {
  const phase = stateError ? "attention" : phaseOf(steps, state, connection);
  const hero = HERO[phase];

  const footer =
    phase === "working" ? (
      <button type="button" disabled className={`${primaryButton} w-full`} data-testid="primary-action">
        <Spinner className="h-3.5 w-3.5" />
        Getting AgentMark ready…
      </button>
    ) : phase === "ready" ? (
      <div className="flex items-center justify-end gap-2">
        <button type="button" className={secondaryButton} onClick={() => onNavigate("settings")} data-testid="open-settings">
          Settings
        </button>
        <button type="button" className={`${primaryButton} min-w-24`} onClick={() => void api.closeWindow()} data-testid="primary-action">
          Done
        </button>
      </div>
    ) : state && !state.initialized ? (
      <button type="button" className={`${primaryButton} w-full`} onClick={() => onNavigate("setup")} data-testid="primary-action">
        Set up AgentMark
      </button>
    ) : state && !connection ? (
      <button type="button" className={`${primaryButton} w-full`} onClick={() => onNavigate("extension")} data-testid="primary-action">
        Add the Chrome extension
      </button>
    ) : null;

  const host = steps.native_host;
  const hostWaiting = typeof host !== "string" && host.status === "waiting";

  return (
    <Screen footer={footer} testId="home">
      <div className="flex flex-col items-start">
        <AgentMarkMark size={40} />
        <h1 className={`mt-4 ${heading}`} data-testid="home-title">
          {hero.title}
        </h1>
        <p className={lede}>{hero.text}</p>
      </div>

      <ul className={`${card} mt-5 divide-y divide-neutral-100 dark:divide-white/[0.06]`} aria-label="Install steps">
        {STEP_ORDER.map((id) => {
          const step = steps[id];
          const report = typeof step === "string" ? null : step;
          const retry =
            report?.status === "failed" ? (
              <button type="button" className={`${secondaryButton} h-7 px-2.5 text-xs`} onClick={() => onRetry(id)} data-testid={`retry-${id}`}>
                Try again
              </button>
            ) : id === "native_host" && hostWaiting && state?.initialized ? (
              <button type="button" className={`${linkButton} text-xs`} onClick={() => onNavigate("extension")}>
                Add extension
              </button>
            ) : null;
          return (
            <StatusRow
              key={id}
              state={rowState(step)}
              title={STEP_TITLES[id]}
              summary={report ? report.summary : step === "running" ? RUNNING_SUMMARY[id] : null}
              detail={report?.detail}
              action={retry}
              testId={`step-${id}`}
            />
          );
        })}
        <StatusRow
          state={isLiveConnection(connection) ? "done" : "pending"}
          title="Chrome extension"
          summary={
            !connection
              ? "Not connected yet."
              : isLiveConnection(connection)
                ? `Connected${connection.extension_version ? ` · version ${connection.extension_version}` : ""}`
                : `${lastSeenText(connection)}.`
          }
          action={
            !connection && state?.initialized ? (
              <button type="button" className={`${linkButton} text-xs`} onClick={() => onNavigate("extension")}>
                Add extension
              </button>
            ) : null
          }
          testId="step-extension"
        />
      </ul>

      {stateError && (
        <div className="mt-4 rounded-lg bg-red-50 px-3 py-2.5 text-xs text-red-700 dark:bg-red-500/10 dark:text-red-300" role="alert" data-testid="state-error">
          <p>{stateError}</p>
          <button type="button" className={`${linkButton} mt-1 text-xs`} onClick={onReloadState}>
            Try again
          </button>
        </div>
      )}

      {phase === "ready" && state && (
        <dl className="mt-5 grid grid-cols-2 gap-3" data-testid="ready-summary">
          <div className={`${card} px-4 py-3`}>
            <dt className="text-2xs font-medium uppercase tracking-[0.06em] text-neutral-500 dark:text-neutral-400">Bookmarks</dt>
            <dd className="mt-1 text-[17px] font-semibold tabular-nums text-neutral-900 dark:text-neutral-100">
              {state.bookmark_count === null ? "–" : state.bookmark_count.toLocaleString()}
            </dd>
          </div>
          <div className={`${card} min-w-0 px-4 py-3`}>
            <dt className="text-2xs font-medium uppercase tracking-[0.06em] text-neutral-500 dark:text-neutral-400">Folder</dt>
            <dd className="mt-1.5 truncate font-mono text-xs text-neutral-800 dark:text-neutral-200" title={state.settings.storage_path}>
              {state.settings.storage_path_display}
            </dd>
          </div>
        </dl>
      )}
    </Screen>
  );
}
