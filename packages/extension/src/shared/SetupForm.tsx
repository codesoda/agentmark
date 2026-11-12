/**
 * Screen C (docs/setup-spec.md §4): the app is installed but not set up.
 * Two choices and a switch, saved through `save_settings`. In the popup the
 * answers live in the setup draft too, because the popup closes whenever the
 * folder dialog takes focus (contract §4.4).
 */

import { useCallback, useEffect, useId, useRef, useState } from "react";
import AgentMarkMark from "./AgentMarkMark";
import type { AppState } from "./connection";
import { sendGetSetupState, sendSaveSettings } from "./runtime";
import { AgentChoice, FolderField, Toggle } from "./settings-controls";
import { errorFor, primaryButtonClass, SUMMARIES_LABEL, SUMMARIES_NOTE, SUMMARIES_OFF_NOTE } from "./settings-shared";
import { clearSetupDraft, loadSetupDraft, updateSetupDraft, type SetupDraft } from "./setup-draft";
import type { FieldError, RuntimeResponse, SetupState } from "./types";

export interface SetupFormProps {
  /** "Finish and save this page" (or "this post") in the popup, "Finish setup" elsewhere. */
  submitLabel: string;
  /** Keep answers in the session draft (popup only). */
  useDraft?: boolean;
  /** Setup is saved (or turned out to be done already). */
  onDone: (state: SetupState) => void;
  /** Let a failed request move the surface's app state; returns the new state or null. */
  onResult: (response: RuntimeResponse, setupMessage?: boolean) => AppState | null;
}

interface Answers {
  storage_path: string;
  default_agent: string;
  enrichment_enabled: boolean;
}

type Load = { status: "loading" } | { status: "error"; message: string } | { status: "loaded"; state: SetupState };

function joinNames(names: string[]): string {
  if (names.length <= 1) return names.join("");
  return `${names.slice(0, -1).join(", ")} or ${names[names.length - 1]}`;
}

export default function SetupForm({ submitLabel, useDraft = false, onDone, onResult }: SetupFormProps) {
  const ids = useId();
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const [answers, setAnswers] = useState<Answers | null>(null);
  /** The user picked an agent here (or in an earlier popup of this setup). */
  const [agentPicked, setAgentPicked] = useState(false);
  const [errors, setErrors] = useState<FieldError[]>([]);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const live = useRef(true);
  // Callers may pass new callbacks each render; reading them through refs keeps the load to one.
  const onDoneRef = useRef(onDone);
  const onResultRef = useRef(onResult);
  onDoneRef.current = onDone;
  onResultRef.current = onResult;

  const fetchState = useCallback(async () => {
    setLoad({ status: "loading" });
    const [response, draft] = await Promise.all([
      sendGetSetupState(),
      useDraft ? loadSetupDraft() : Promise.resolve<SetupDraft | null>(null),
    ]);
    if (!live.current) return;
    if (!response.success) {
      if (!onResultRef.current(response, true)) setLoad({ status: "error", message: response.error });
      return;
    }
    if (response.data.type !== "setup_state_result") {
      setLoad({ status: "error", message: `Unexpected response type: ${response.data.type}` });
      return;
    }
    const state = response.data.state;
    if (state.initialized) {
      onDoneRef.current(state);
      return;
    }
    setAnswers({
      storage_path: draft?.storage_path ?? state.settings.storage_path_display,
      default_agent: draft?.default_agent ?? state.settings.default_agent,
      enrichment_enabled: draft?.enrichment_enabled ?? state.settings.enrichment_enabled,
    });
    setAgentPicked(draft?.default_agent != null);
    setLoad({ status: "loaded", state });
  }, [useDraft]);

  useEffect(() => {
    live.current = true;
    void fetchState();
    return () => {
      live.current = false;
    };
  }, [fetchState]);

  const change = (patch: Partial<Answers>) => {
    setAnswers((prev) => (prev ? { ...prev, ...patch } : prev));
    if (useDraft) void updateSetupDraft(patch);
  };

  const submit = async () => {
    if (!answers || submitting) return;
    setSubmitting(true);
    setSubmitError(null);
    const response = await sendSaveSettings({
      storage_path: answers.storage_path,
      default_agent: answers.default_agent,
      enrichment_enabled: answers.enrichment_enabled,
    });
    if (!live.current) return;
    setSubmitting(false);
    if (!response.success) {
      if (!onResultRef.current(response, true)) setSubmitError(response.error);
      return;
    }
    if (response.data.type === "settings_invalid") {
      setErrors(response.data.errors);
      return;
    }
    if (response.data.type !== "save_settings_result") {
      setSubmitError(`Unexpected response type: ${response.data.type}`);
      return;
    }
    setErrors([]);
    if (useDraft) await clearSetupDraft();
    onDoneRef.current(response.data.state);
  };

  const header = (
    <div className="flex items-center gap-2">
      <AgentMarkMark look="idle" />
      <h1 className="text-[15px] font-semibold text-gray-900">Set up AgentMark</h1>
    </div>
  );

  if (load.status === "loading" || !answers) {
    return (
      <section data-testid="setup-loading" aria-busy="true">
        {header}
        {load.status === "error" ? (
          <div data-testid="setup-load-error">
            <p className="mt-2 text-[13px] text-red-700">{load.message}</p>
            <button
              type="button"
              onClick={() => void fetchState()}
              className="mt-3 rounded-md bg-indigo-600 px-3 py-1 text-xs font-medium text-white hover:bg-indigo-700 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500"
            >
              Try again
            </button>
          </div>
        ) : (
          <p className="mt-2 text-[13px] text-gray-500">Reading your settings…</p>
        )}
      </section>
    );
  }

  const state = (load as Extract<Load, { status: "loaded" }>).state;
  const noAgent = !state.agents.some((agent) => agent.found);
  const shownFields = new Set(["storage_path", "default_agent", "enrichment_enabled"]);
  const otherErrors = errors.filter((e) => !shownFields.has(e.field));

  return (
    <form
      data-testid="setup-form"
      aria-labelledby={`${ids}-heading`}
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div className="flex items-center gap-2">
        <AgentMarkMark look="idle" />
        <h1 id={`${ids}-heading`} className="text-[15px] font-semibold text-gray-900">
          Set up AgentMark
        </h1>
      </div>
      <p className="mt-1.5 text-[13px] leading-snug text-gray-600">
        The app is installed ({state.version}). Two choices and you're done.
      </p>
      {state.config_problem && (
        <p className="mt-2 rounded-lg bg-amber-50 px-2.5 py-2 text-xs leading-snug text-amber-900" data-testid="config-problem">
          Your settings file could not be read, so AgentMark will keep a copy of it and write a new one.{" "}
          <span className="break-words text-amber-800/80">({state.config_problem})</span>
        </p>
      )}

      <div className="mt-4">
        <h2 id={`${ids}-folder`} className="text-[13px] font-semibold text-gray-900">
          Where should bookmarks live?
        </h2>
        <p className="mb-1.5 text-xs text-gray-500">A normal folder. One sub-folder per saved page.</p>
        <FolderField
          value={answers.storage_path}
          onChange={(storage_path) => change({ storage_path })}
          labelledBy={`${ids}-folder`}
          error={errorFor(errors, "storage_path")?.message}
          onResult={(r) => onResult(r, true)}
        />
      </div>

      <div className="mt-4">
        <h2 id={`${ids}-agent`} className="text-[13px] font-semibold text-gray-900">
          Which agent writes summaries?
        </h2>
        <p className="mb-1.5 text-xs text-gray-500">AgentMark uses one you already have.</p>
        <AgentChoice
          agents={state.agents}
          // With no agent found, no row shows as picked until the user picks
          // one; the stored default is still sent (contract §4.5).
          value={noAgent && !agentPicked ? "" : answers.default_agent}
          onChange={(default_agent) => {
            setAgentPicked(true);
            change({ default_agent });
          }}
          labelledBy={`${ids}-agent`}
          error={errorFor(errors, "default_agent")?.message}
        />
        {noAgent && (
          <p className="mt-1.5 text-xs leading-snug text-gray-500" data-testid="no-agent">
            No agent was found on this computer, so summaries start off. Install{" "}
            {joinNames(state.agents.map((a) => a.name))} to turn them on later.
          </p>
        )}
      </div>

      <div className="mt-4">
        <Toggle
          checked={answers.enrichment_enabled}
          onChange={(enrichment_enabled) => change({ enrichment_enabled })}
          label={SUMMARIES_LABEL}
          description={`${SUMMARIES_NOTE} ${SUMMARIES_OFF_NOTE}`}
          testId="summaries-switch"
        />
        {errorFor(errors, "enrichment_enabled") && (
          <p className="mt-1 text-xs text-red-600" data-testid="field-error">
            {errorFor(errors, "enrichment_enabled")!.message}
          </p>
        )}
      </div>

      {(otherErrors.length > 0 || submitError) && (
        <div className="mt-3 rounded-lg bg-red-50 px-2.5 py-2 text-xs text-red-700" role="alert" data-testid="setup-error">
          {submitError && <p>{submitError}</p>}
          {otherErrors.map((e, i) => (
            <p key={i}>{e.message}</p>
          ))}
        </div>
      )}

      <button type="submit" disabled={submitting} className={`mt-4 ${primaryButtonClass}`} data-testid="setup-submit">
        {submitting ? "Saving…" : submitLabel}
      </button>
      <p className="mt-2.5 border-t border-gray-100 pt-2.5 text-xs text-gray-500">
        You can change all of this later in Settings.
      </p>
    </form>
  );
}
