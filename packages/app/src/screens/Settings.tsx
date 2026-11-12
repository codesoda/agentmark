/**
 * Settings (spec §5): the extension's Settings fields minus the browser
 * permission switch. "Save changes" sends only what changed (contract §5.2).
 */

import { useCallback, useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { api, errorText } from "../api";
import { AgentChoice, FieldMessage, FolderField, SectionLabel, Toggle } from "../components/controls";
import Screen from "../components/Screen";
import { card, focusRing, inputBad, inputBase, inputOk, primaryButton, secondaryButton } from "../components/ui";
import {
  CAPTURE_HEADING,
  CAPTURE_NOTE,
  FOLDER_MOVE_NOTE,
  PROMPT_PLACEHOLDER,
  SUMMARIES_LABEL,
  SUMMARIES_NOTE,
  SUMMARIES_OFF_NOTE,
} from "../copy";
import { isLiveConnection, lastSeenText } from "../connection";
import { buildChanges, errorFor, formFromSettings, type SettingsFormValues } from "../settings-form";
import type { ConnectionRecord, FieldError, SetupState } from "../types";

type SaveStatus = { status: "idle" } | { status: "saving" } | { status: "saved" } | { status: "error"; message: string };

const SAVED_MS = 2500;

export default function Settings({
  connection,
  onSaved,
}: {
  connection: ConnectionRecord | null;
  onSaved: (state: SetupState) => void;
}) {
  const ids = useId();
  const [state, setState] = useState<SetupState | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [form, setForm] = useState<SettingsFormValues | null>(null);
  const [errors, setErrors] = useState<FieldError[]>([]);
  const [save, setSave] = useState<SaveStatus>({ status: "idle" });
  const [newSite, setNewSite] = useState("");
  const savedTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const load = useCallback(async () => {
    setLoadError(null);
    try {
      const next = await api.getSetupState();
      setState(next);
      setForm(formFromSettings(next.settings));
    } catch (err) {
      setLoadError(errorText(err));
    }
  }, []);

  useEffect(() => {
    void load();
    return () => {
      if (savedTimer.current) clearTimeout(savedTimer.current);
    };
  }, [load]);

  if (loadError) {
    return (
      <Screen testId="settings-error">
        <p className="text-[13px] text-red-600 dark:text-red-400">{loadError}</p>
        <button type="button" onClick={() => void load()} className={`${secondaryButton} mt-3`}>
          Try again
        </button>
      </Screen>
    );
  }

  if (!state || !form) {
    return (
      <Screen testId="settings-loading">
        <p className="text-[13px] text-neutral-500 dark:text-neutral-400">Loading settings…</p>
      </Screen>
    );
  }

  const changes = buildChanges(state.settings, form);
  const dirty = Object.keys(changes).length > 0;
  const update = (patch: Partial<SettingsFormValues>) => {
    setForm((prev) => (prev ? { ...prev, ...patch } : prev));
    if (save.status === "saved" || save.status === "error") setSave({ status: "idle" });
  };

  const addSite = () => {
    const site = newSite.trim();
    if (!site) return;
    if (!form.snapshot_sites.includes(site)) update({ snapshot_sites: [...form.snapshot_sites, site] });
    setNewSite("");
  };

  const removeSite = (index: number) => {
    update({ snapshot_sites: form.snapshot_sites.filter((_, i) => i !== index) });
    // Index-based errors no longer line up with the list.
    setErrors((prev) => prev.filter((e) => e.field !== "snapshot_sites"));
  };

  const submit = async () => {
    if (!dirty || save.status === "saving") return;
    setSave({ status: "saving" });
    try {
      const result = await api.saveSettings(changes);
      if (result.type === "invalid") {
        setErrors(result.errors);
        setSave({ status: "error", message: "Some settings need fixing. See the highlighted fields." });
        return;
      }
      setErrors([]);
      setState(result.state);
      setForm(formFromSettings(result.state.settings));
      onSaved(result.state);
      setSave({ status: "saved" });
      if (savedTimer.current) clearTimeout(savedTimer.current);
      savedTimer.current = setTimeout(() => setSave((s) => (s.status === "saved" ? { status: "idle" } : s)), SAVED_MS);
    } catch (err) {
      setSave({ status: "error", message: errorText(err) });
    }
  };

  const siteErrors = errors.filter((e) => e.field === "snapshot_sites");
  const listError = siteErrors.find((e) => e.index === null);
  const promptError = errorFor(errors, "system_prompt")?.message;
  const formId = `${ids}-form`;

  return (
    <Screen
      testId="settings"
      footer={
        <div className="flex items-center gap-3">
          <p
            className={`min-w-0 flex-1 text-xs ${save.status === "error" ? "text-red-600 dark:text-red-400" : save.status === "saved" ? "text-emerald-700 dark:text-emerald-300" : "text-neutral-500 dark:text-neutral-400"}`}
            aria-live="polite"
            data-testid="save-status"
          >
            {save.status === "saved" ? "Saved" : save.status === "error" ? save.message : dirty ? "Unsaved changes" : "No unsaved changes"}
          </p>
          <button type="submit" form={formId} disabled={!dirty || save.status === "saving"} className={`${primaryButton} min-w-32`} data-testid="save-settings">
            {save.status === "saving" ? "Saving…" : "Save changes"}
          </button>
        </div>
      }
    >
      <form
        id={formId}
        className="space-y-7"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <section aria-labelledby={`${ids}-app`}>
          <SectionLabel id={`${ids}-app`}>App</SectionLabel>
          <dl className={`${card} mt-2 divide-y divide-neutral-100 text-[13px] dark:divide-white/[0.06]`}>
            <Row label="Version">
              <span className="font-mono text-xs" data-testid="app-version">
                agentmark {state.version}
              </span>
            </Row>
            <Row label="Chrome extension">
              <span data-testid="extension-status" className={isLiveConnection(connection) ? "text-emerald-700 dark:text-emerald-300" : ""}>
                {!connection
                  ? "Not connected yet"
                  : isLiveConnection(connection)
                    ? `Connected${connection.extension_version ? ` · ${connection.extension_version}` : ""}`
                    : lastSeenText(connection)}
              </span>
            </Row>
            <Row label="Bookmarks">
              <span className="tabular-nums" data-testid="bookmark-count">
                {state.bookmark_count === null ? "Unknown" : state.bookmark_count.toLocaleString()}
              </span>
            </Row>
          </dl>
        </section>

        <section aria-labelledby={`${ids}-storage`}>
          <SectionLabel id={`${ids}-storage`}>Storage</SectionLabel>
          <div className="mt-2">
            <FolderField
              value={form.storage_path}
              onChange={(storage_path) => update({ storage_path })}
              labelledBy={`${ids}-storage`}
              error={errorFor(errors, "storage_path")?.message}
            />
            {changes.storage_path !== undefined ? (
              <p
                className="mt-1.5 rounded-lg bg-amber-50 px-3 py-2 text-xs leading-snug text-amber-900 dark:bg-amber-400/10 dark:text-amber-200"
                role="status"
                data-testid="folder-move-note"
                data-tone="warning"
              >
                {FOLDER_MOVE_NOTE}
              </p>
            ) : (
              <p className="mt-1.5 text-xs text-neutral-500 dark:text-neutral-400" data-testid="folder-move-note" data-tone="hint">
                {FOLDER_MOVE_NOTE}
              </p>
            )}
          </div>
        </section>

        <section aria-labelledby={`${ids}-summaries`} className="space-y-4">
          <SectionLabel id={`${ids}-summaries`}>Summaries</SectionLabel>
          <Toggle
            checked={form.enrichment_enabled}
            onChange={(enrichment_enabled) => update({ enrichment_enabled })}
            label={SUMMARIES_LABEL}
            description={`${SUMMARIES_NOTE} ${SUMMARIES_OFF_NOTE}`}
            testId="summaries-switch"
          />
          <div>
            <p id={`${ids}-agent`} className="mb-2 text-[13px] font-medium text-neutral-900 dark:text-neutral-100">
              Agent
            </p>
            <AgentChoice
              agents={state.agents}
              value={form.default_agent}
              onChange={(default_agent) => update({ default_agent })}
              labelledBy={`${ids}-agent`}
              error={errorFor(errors, "default_agent")?.message}
            />
          </div>
          <div>
            <label htmlFor={`${ids}-prompt`} className="mb-2 block text-[13px] font-medium text-neutral-900 dark:text-neutral-100">
              Instructions for summaries
            </label>
            <textarea
              id={`${ids}-prompt`}
              value={form.system_prompt}
              onChange={(e) => update({ system_prompt: e.target.value })}
              rows={3}
              placeholder={PROMPT_PLACEHOLDER}
              aria-invalid={promptError ? true : undefined}
              aria-describedby={promptError ? `${ids}-prompt-error` : undefined}
              className={`${inputBase} resize-none py-2 text-[13px] leading-snug ${promptError ? inputBad : inputOk}`}
              data-testid="prompt-input"
            />
            <FieldMessage id={`${ids}-prompt-error`} error={promptError} />
          </div>
        </section>

        <section aria-labelledby={`${ids}-capture`}>
          <SectionLabel id={`${ids}-capture`}>{CAPTURE_HEADING}</SectionLabel>
          <p className="mt-1.5 text-xs leading-snug text-neutral-500 dark:text-neutral-400">{CAPTURE_NOTE}</p>
          <ul className="mt-2.5 flex flex-wrap gap-1.5" aria-label="Sites" data-testid="site-chips">
            {state.builtin_snapshot_sites.map((site) => (
              <li
                key={`builtin-${site}`}
                className="rounded-md border border-neutral-200 bg-neutral-100/70 px-1.5 py-0.5 font-mono text-[11.5px] text-neutral-600 dark:border-white/[0.08] dark:bg-white/[0.05] dark:text-neutral-400"
                title="Built in"
                data-testid="builtin-site"
              >
                {site}
              </li>
            ))}
            {form.snapshot_sites.map((site, i) => {
              const error = errorFor(errors, "snapshot_sites", i);
              return (
                <li
                  key={`user-${site}-${i}`}
                  className={`inline-flex items-center gap-0.5 rounded-md border py-0.5 pl-1.5 pr-0.5 font-mono text-[11.5px] ${
                    error
                      ? "border-red-300 bg-red-50 text-red-700 dark:border-red-400/40 dark:bg-red-500/10 dark:text-red-300"
                      : "border-accent/30 bg-white text-neutral-800 dark:border-indigo-400/30 dark:bg-indigo-400/[0.08] dark:text-neutral-100"
                  }`}
                  data-testid="user-site"
                >
                  {site}
                  <button
                    type="button"
                    onClick={() => removeSite(i)}
                    aria-label={`Remove ${site}`}
                    className={`ml-0.5 rounded px-1 text-neutral-400 hover:bg-neutral-100 hover:text-neutral-700 dark:hover:bg-white/10 dark:hover:text-neutral-200 ${focusRing}`}
                  >
                    ×
                  </button>
                </li>
              );
            })}
          </ul>
          <div className="mt-2.5 flex gap-2">
            <input
              type="text"
              value={newSite}
              onChange={(e) => setNewSite(e.target.value)}
              onKeyDown={(e: KeyboardEvent<HTMLInputElement>) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  addSite();
                }
              }}
              placeholder="example.com/app/"
              aria-label="Add a site"
              spellCheck={false}
              autoComplete="off"
              autoCorrect="off"
              autoCapitalize="off"
              className={`${inputBase} h-8 font-mono ${inputOk}`}
              data-testid="new-site-input"
            />
            <button type="button" onClick={addSite} disabled={!newSite.trim()} className={secondaryButton} data-testid="add-site">
              Add site
            </button>
          </div>
          {siteErrors.length > 0 && (
            <ul className="mt-1.5 space-y-0.5" data-testid="site-errors">
              {listError && <li className="text-xs text-red-600 dark:text-red-400">{listError.message}</li>}
              {siteErrors
                .filter((e) => e.index !== null)
                .map((e) => (
                  <li key={e.index} className="text-xs text-red-600 dark:text-red-400">
                    {e.message}
                  </li>
                ))}
            </ul>
          )}
        </section>
      </form>
    </Screen>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex h-9 items-center justify-between gap-3 px-3.5">
      <dt className="text-neutral-600 dark:text-neutral-400">{label}</dt>
      <dd className="text-neutral-800 dark:text-neutral-200">{children}</dd>
    </div>
  );
}
