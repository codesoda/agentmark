/**
 * Setup (spec §5): the same two questions and switch as the extension's
 * screen C, with the native folder dialog. Saves through `apply_settings`.
 */

import { useCallback, useEffect, useId, useRef, useState } from "react";
import { api, errorText } from "../api";
import { AgentChoice, FolderField, Toggle } from "../components/controls";
import Screen from "../components/Screen";
import { heading, lede, linkButton, primaryButton } from "../components/ui";
import {
  AGENT_HINT,
  AGENT_QUESTION,
  FOLDER_HINT,
  FOLDER_QUESTION,
  joinNames,
  SUMMARIES_LABEL,
  SUMMARIES_NOTE,
  SUMMARIES_OFF_NOTE,
} from "../copy";
import { errorFor } from "../settings-form";
import type { FieldError, SetupState } from "../types";

interface Answers {
  storage_path: string;
  default_agent: string;
  enrichment_enabled: boolean;
}

type Load = { status: "loading" } | { status: "error"; message: string } | { status: "loaded"; state: SetupState };

const SHOWN_FIELDS = new Set(["storage_path", "default_agent", "enrichment_enabled"]);

export default function Setup({ onDone, onBack }: { onDone: (state: SetupState) => void; onBack: () => void }) {
  const ids = useId();
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const [answers, setAnswers] = useState<Answers | null>(null);
  const [errors, setErrors] = useState<FieldError[]>([]);
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const live = useRef(true);

  const fetchState = useCallback(async () => {
    setLoad({ status: "loading" });
    try {
      const state = await api.getSetupState();
      if (!live.current) return;
      setAnswers({
        storage_path: state.settings.storage_path_display,
        default_agent: state.settings.default_agent,
        enrichment_enabled: state.settings.enrichment_enabled,
      });
      setLoad({ status: "loaded", state });
    } catch (err) {
      if (live.current) setLoad({ status: "error", message: errorText(err) });
    }
  }, []);

  useEffect(() => {
    live.current = true;
    void fetchState();
    return () => {
      live.current = false;
    };
  }, [fetchState]);

  const change = (patch: Partial<Answers>) => setAnswers((prev) => (prev ? { ...prev, ...patch } : prev));

  const submit = async () => {
    if (!answers || submitting) return;
    setSubmitting(true);
    setSubmitError(null);
    try {
      const result = await api.saveSettings({
        storage_path: answers.storage_path,
        default_agent: answers.default_agent,
        enrichment_enabled: answers.enrichment_enabled,
      });
      if (!live.current) return;
      if (result.type === "invalid") {
        setErrors(result.errors);
        return;
      }
      setErrors([]);
      onDone(result.state);
    } catch (err) {
      if (live.current) setSubmitError(errorText(err));
    } finally {
      if (live.current) setSubmitting(false);
    }
  };

  if (load.status !== "loaded" || !answers) {
    return (
      <Screen testId="setup-loading">
        <h1 className={heading}>Set up AgentMark</h1>
        {load.status === "error" ? (
          <div data-testid="setup-load-error">
            <p className="mt-2 text-[13px] text-red-700 dark:text-red-300">{load.message}</p>
            <button type="button" onClick={() => void fetchState()} className={`${linkButton} mt-2`}>
              Try again
            </button>
          </div>
        ) : (
          <p className={lede}>Reading your settings…</p>
        )}
      </Screen>
    );
  }

  const state = load.state;
  const noAgent = !state.agents.some((agent) => agent.found);
  const otherErrors = errors.filter((e) => !SHOWN_FIELDS.has(e.field));
  const formId = `${ids}-form`;

  return (
    <Screen
      testId="setup"
      footer={
        <div>
          <button type="submit" form={formId} disabled={submitting} className={`${primaryButton} w-full`} data-testid="setup-submit">
            {submitting ? "Saving…" : "Finish setup"}
          </button>
          <p className="mt-2 text-center text-xs text-neutral-500 dark:text-neutral-400">You can change all of this later in Settings.</p>
        </div>
      }
    >
      <form
        id={formId}
        aria-labelledby={`${ids}-heading`}
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <button type="button" onClick={onBack} className={`${linkButton} -ml-0.5 text-xs`}>
          ‹ Back
        </button>
        <h1 id={`${ids}-heading`} className={`mt-2 ${heading}`}>
          Set up AgentMark
        </h1>
        <p className={lede}>Two choices and you're done.</p>
        {state.config_problem && (
          <p
            className="mt-3 rounded-lg bg-amber-50 px-3 py-2.5 text-xs leading-snug text-amber-900 dark:bg-amber-400/10 dark:text-amber-200"
            data-testid="config-problem"
          >
            Your settings file could not be read, so AgentMark will keep a copy of it and write a new one.{" "}
            <span className="break-words opacity-75">({state.config_problem})</span>
          </p>
        )}

        <section className="mt-6">
          <h2 id={`${ids}-folder`} className="text-[13px] font-semibold text-neutral-900 dark:text-neutral-100">
            {FOLDER_QUESTION}
          </h2>
          <p className="mb-2 mt-0.5 text-xs text-neutral-500 dark:text-neutral-400">{FOLDER_HINT}</p>
          <FolderField
            value={answers.storage_path}
            onChange={(storage_path) => change({ storage_path })}
            labelledBy={`${ids}-folder`}
            error={errorFor(errors, "storage_path")?.message}
          />
        </section>

        <section className="mt-6">
          <h2 id={`${ids}-agent`} className="text-[13px] font-semibold text-neutral-900 dark:text-neutral-100">
            {AGENT_QUESTION}
          </h2>
          <p className="mb-2 mt-0.5 text-xs text-neutral-500 dark:text-neutral-400">{AGENT_HINT}</p>
          <AgentChoice
            agents={state.agents}
            value={answers.default_agent}
            onChange={(default_agent) => change({ default_agent })}
            labelledBy={`${ids}-agent`}
            error={errorFor(errors, "default_agent")?.message}
          />
          {noAgent && (
            <p className="mt-2 text-xs leading-snug text-neutral-500 dark:text-neutral-400" data-testid="no-agent">
              No agent was found on this computer, so summaries start off. Install {joinNames(state.agents.map((a) => a.name))} to turn
              them on later.
            </p>
          )}
        </section>

        <section className="mt-6">
          <Toggle
            checked={answers.enrichment_enabled}
            onChange={(enrichment_enabled) => change({ enrichment_enabled })}
            label={SUMMARIES_LABEL}
            description={`${SUMMARIES_NOTE} ${SUMMARIES_OFF_NOTE}`}
            testId="summaries-switch"
          />
          {errorFor(errors, "enrichment_enabled") && (
            <p className="mt-1.5 text-xs text-red-600 dark:text-red-400" data-testid="field-error">
              {errorFor(errors, "enrichment_enabled")!.message}
            </p>
          )}
        </section>

        {(otherErrors.length > 0 || submitError) && (
          <div className="mt-5 rounded-lg bg-red-50 px-3 py-2.5 text-xs text-red-700 dark:bg-red-500/10 dark:text-red-300" role="alert" data-testid="setup-error">
            {submitError && <p>{submitError}</p>}
            {otherErrors.map((e, i) => (
              <p key={i}>{e.message}</p>
            ))}
          </div>
        )}
      </form>
    </Screen>
  );
}
