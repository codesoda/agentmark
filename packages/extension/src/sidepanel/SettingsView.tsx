/**
 * Screen D (docs/setup-spec.md §4): AgentMark's settings, in the side panel.
 * Loads `get_setup_state`; "Save changes" sends only the fields that changed
 * (absent = unchanged, contract §5.2). The toolbar permission switch acts at
 * once: Chrome only shows its prompt from inside the click.
 */

import { useCallback, useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import type { AppState } from "../shared/connection";
import { sendGetSetupState, sendSaveSettings } from "../shared/runtime";
import { AgentChoice, FieldMessage, FolderField, SectionLabel, Toggle } from "../shared/settings-controls";
import {
  errorFor,
  focusRing,
  FOLDER_MOVE_NOTE,
  inputClass,
  primaryButtonClass,
  SUMMARIES_LABEL,
  SUMMARIES_NOTE,
  SUMMARIES_OFF_NOTE,
} from "../shared/settings-shared";
import type { FieldError, RuntimeResponse, SetupState } from "../shared/types";
import { buildChanges, formFromSettings, type SettingsFormValues } from "./settings-form";

type SaveStatus = { status: "idle" } | { status: "saving" } | { status: "saved" } | { status: "error"; message: string };

const SAVED_MS = 2500;

interface SettingsViewProps {
  /** Let a failed request move the side panel's app state; returns the new state or null. */
  onResult: (response: RuntimeResponse, setupMessage?: boolean) => AppState | null;
}

export default function SettingsView({ onResult }: SettingsViewProps) {
  const ids = useId();
  const [state, setState] = useState<SetupState | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [form, setForm] = useState<SettingsFormValues | null>(null);
  const [errors, setErrors] = useState<FieldError[]>([]);
  const [save, setSave] = useState<SaveStatus>({ status: "idle" });
  const [newSite, setNewSite] = useState("");
  const savedTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onResultRef = useRef(onResult);
  onResultRef.current = onResult;

  const load = useCallback(async () => {
    setLoadError(null);
    const response = await sendGetSetupState();
    if (!response.success) {
      if (!onResultRef.current(response, true)) setLoadError(response.error);
      return;
    }
    if (response.data.type !== "setup_state_result") {
      setLoadError(`Unexpected response type: ${response.data.type}`);
      return;
    }
    setState(response.data.state);
    setForm(formFromSettings(response.data.state.settings));
  }, []);

  useEffect(() => {
    void load();
    return () => {
      if (savedTimer.current) clearTimeout(savedTimer.current);
    };
  }, [load]);

  if (loadError) {
    return (
      <div className="p-4" data-testid="settings-error">
        <p className="text-sm text-red-600">{loadError}</p>
        <button
          type="button"
          onClick={() => void load()}
          className={`mt-3 rounded-md bg-indigo-600 px-3 py-1 text-xs font-medium text-white hover:bg-indigo-700 ${focusRing}`}
        >
          Try again
        </button>
      </div>
    );
  }

  if (!state || !form) {
    return (
      <div className="flex flex-1 items-center justify-center p-4" data-testid="settings-loading">
        <p className="text-sm text-gray-500">Loading settings…</p>
      </div>
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
    const response = await sendSaveSettings(changes);
    if (!response.success) {
      onResultRef.current(response, true);
      setSave({ status: "error", message: response.error });
      return;
    }
    if (response.data.type === "settings_invalid") {
      setErrors(response.data.errors);
      setSave({ status: "error", message: "Some settings need fixing. See the highlighted fields." });
      return;
    }
    if (response.data.type !== "save_settings_result") {
      setSave({ status: "error", message: `Unexpected response type: ${response.data.type}` });
      return;
    }
    setErrors([]);
    setState(response.data.state);
    setForm(formFromSettings(response.data.state.settings));
    setSave({ status: "saved" });
    if (savedTimer.current) clearTimeout(savedTimer.current);
    savedTimer.current = setTimeout(() => setSave((s) => (s.status === "saved" ? { status: "idle" } : s)), SAVED_MS);
  };

  const siteErrors = errors.filter((e) => e.field === "snapshot_sites");
  const listError = siteErrors.find((e) => e.index === null);

  return (
    <form
      className="flex min-h-0 flex-1 flex-col"
      data-testid="settings-view"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div className="flex-1 space-y-6 overflow-y-auto px-4 py-4">
        <section aria-labelledby={`${ids}-app`}>
          <SectionLabel id={`${ids}-app`}>App</SectionLabel>
          <dl className="mt-2 space-y-1.5 text-[13px]">
            <div className="flex items-center justify-between gap-3">
              <dt className="text-gray-700">Connected</dt>
              <dd className="font-mono text-xs text-gray-600" data-testid="app-version">agentmark {state.version}</dd>
            </div>
            <div className="flex items-center justify-between gap-3">
              <dt className="text-gray-700">Bookmarks</dt>
              <dd className="font-mono text-xs text-gray-600" data-testid="bookmark-count">
                {state.bookmark_count === null ? "Unknown" : state.bookmark_count.toLocaleString()}
              </dd>
            </div>
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
              onResult={(r) => onResultRef.current(r, true)}
            />
            {changes.storage_path !== undefined ? (
              <p
                className="mt-1.5 rounded-lg bg-amber-50 px-2.5 py-2 text-xs leading-snug text-amber-900"
                role="status"
                data-testid="folder-move-note"
                data-tone="warning"
              >
                {FOLDER_MOVE_NOTE}
              </p>
            ) : (
              <p className="mt-1.5 text-xs text-gray-500" data-testid="folder-move-note" data-tone="hint">
                {FOLDER_MOVE_NOTE}
              </p>
            )}
          </div>
        </section>

        <section aria-labelledby={`${ids}-summaries`} className="space-y-3">
          <SectionLabel id={`${ids}-summaries`}>Summaries</SectionLabel>
          <Toggle
            checked={form.enrichment_enabled}
            onChange={(enrichment_enabled) => update({ enrichment_enabled })}
            label={SUMMARIES_LABEL}
            description={`${SUMMARIES_NOTE} ${SUMMARIES_OFF_NOTE}`}
            testId="summaries-switch"
          />
          <div>
            <p id={`${ids}-agent`} className="mb-1.5 text-[13px] font-semibold text-gray-900">
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
            <label htmlFor={`${ids}-prompt`} className="mb-1.5 block text-[13px] font-semibold text-gray-900">
              Instructions for summaries
            </label>
            <textarea
              id={`${ids}-prompt`}
              value={form.system_prompt}
              onChange={(e) => update({ system_prompt: e.target.value })}
              rows={3}
              placeholder='Optional. For example: "Keep summaries under 60 words."'
              aria-invalid={errorFor(errors, "system_prompt") ? true : undefined}
              aria-describedby={errorFor(errors, "system_prompt") ? `${ids}-prompt-error` : undefined}
              className={`${inputClass} resize-y font-sans text-[13px] ${errorFor(errors, "system_prompt") ? "border-red-400" : "border-gray-300 focus:border-indigo-500"}`}
              data-testid="prompt-input"
            />
            <FieldMessage id={`${ids}-prompt-error`} error={errorFor(errors, "system_prompt")?.message} />
          </div>
        </section>

        <section aria-labelledby={`${ids}-capture`}>
          <SectionLabel id={`${ids}-capture`}>Capture the page from the browser on</SectionLabel>
          <p className="mt-1 text-xs leading-snug text-gray-500">
            On these sites AgentMark saves what your browser shows instead of fetching the page itself.
          </p>
          <ul className="mt-2 flex flex-wrap gap-1.5" aria-label="Sites" data-testid="site-chips">
            {state.builtin_snapshot_sites.map((site) => (
              <li
                key={`builtin-${site}`}
                className="rounded-md border border-gray-200 bg-gray-50 px-1.5 py-0.5 font-mono text-[11.5px] text-gray-600"
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
                    error ? "border-red-300 bg-red-50 text-red-700" : "border-indigo-200 bg-white text-gray-800"
                  }`}
                  data-testid="user-site"
                >
                  {site}
                  <button
                    type="button"
                    onClick={() => removeSite(i)}
                    aria-label={`Remove ${site}`}
                    className={`ml-0.5 rounded px-1 text-gray-400 hover:bg-gray-100 hover:text-gray-700 ${focusRing}`}
                  >
                    ×
                  </button>
                </li>
              );
            })}
          </ul>
          <div className="mt-2 flex gap-1.5">
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
              className={`${inputClass} border-gray-300 focus:border-indigo-500`}
              data-testid="new-site-input"
            />
            <button
              type="button"
              onClick={addSite}
              disabled={!newSite.trim()}
              className={`shrink-0 rounded-lg border border-gray-300 bg-white px-2.5 text-[13px] text-gray-700 shadow-sm hover:bg-gray-50 disabled:opacity-50 ${focusRing}`}
              data-testid="add-site"
            >
              Add site
            </button>
          </div>
          {siteErrors.length > 0 && (
            <ul className="mt-1.5 space-y-0.5" data-testid="site-errors">
              {listError && <li className="text-xs text-red-600">{listError.message}</li>}
              {siteErrors
                .filter((e) => e.index !== null)
                .map((e) => (
                  <li key={e.index} className="text-xs text-red-600">
                    {e.message}
                  </li>
                ))}
            </ul>
          )}
        </section>

        <section aria-labelledby={`${ids}-toolbar`}>
          <SectionLabel id={`${ids}-toolbar`}>Toolbar icon</SectionLabel>
          <div className="mt-2">
            <TabsPermissionToggle />
          </div>
        </section>
      </div>

      <div className="sticky bottom-0 border-t border-gray-200 bg-white/95 px-4 py-3 backdrop-blur">
        <button
          type="submit"
          disabled={!dirty || save.status === "saving"}
          className={primaryButtonClass}
          data-testid="save-settings"
        >
          {save.status === "saving" ? "Saving…" : "Save changes"}
        </button>
        <p
          className={`mt-1.5 min-h-[1rem] text-center text-xs ${save.status === "error" ? "text-red-600" : "text-gray-500"}`}
          aria-live="polite"
          data-testid="save-status"
        >
          {save.status === "saved" ? "Saved" : save.status === "error" ? save.message : dirty ? "" : "No unsaved changes"}
        </p>
      </div>
    </form>
  );
}

const TABS: chrome.permissions.Permissions = { permissions: ["tabs"] };

/**
 * The optional `tabs` permission. `permissions.request` must run inside the
 * click, so this switch acts at once instead of waiting for "Save changes".
 */
export function TabsPermissionToggle() {
  const [granted, setGranted] = useState<boolean | null>(null);
  const [note, setNote] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    const sync = () => {
      Promise.resolve(chrome.permissions.contains(TABS))
        .then((has) => live && setGranted(has === true))
        .catch(() => live && setGranted(false));
    };
    sync();
    const onChange = (p: chrome.permissions.Permissions) => {
      if (p.permissions?.includes("tabs")) sync();
    };
    // @types/chrome omits removeListener on these two events; Chrome has it.
    type PermissionsEvent = chrome.events.Event<(p: chrome.permissions.Permissions) => void>;
    const events = [chrome.permissions.onAdded, chrome.permissions.onRemoved] as unknown as Array<PermissionsEvent | undefined>;
    for (const event of events) event?.addListener(onChange);
    return () => {
      live = false;
      for (const event of events) event?.removeListener(onChange);
    };
  }, []);

  const change = (next: boolean) => {
    setNote(null);
    // Called straight from the click handler: no awaits before the request.
    const pending = next ? chrome.permissions.request(TABS) : chrome.permissions.remove(TABS);
    Promise.resolve(pending)
      .then((ok) => {
        if (next) {
          setGranted(ok === true);
          if (ok !== true) setNote("Chrome did not grant the permission, so the icon only changes when you save.");
        } else {
          setGranted(ok !== true);
        }
      })
      .catch((err: unknown) => setNote(err instanceof Error ? err.message : String(err)));
  };

  return (
    <div data-testid="tabs-permission">
      <Toggle
        checked={granted === true}
        onChange={change}
        disabled={granted === null}
        label="Show when the current page is already saved"
        description="On every site. Chrome will ask for permission."
        testId="tabs-permission-switch"
      />
      <p className="mt-1.5 text-xs leading-snug text-gray-500">
        Without it, the icon turns blue when you save, and on saved LinkedIn, X, Hacker News and Reddit pages.
      </p>
      {note && (
        <p className="mt-1 text-xs text-amber-700" role="status" data-testid="tabs-permission-note">
          {note}
        </p>
      )}
    </div>
  );
}
