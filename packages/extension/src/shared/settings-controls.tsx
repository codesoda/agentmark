/**
 * Form controls shared by setup (popup and side panel, screen C) and the
 * Settings view (side panel, screen D).
 */

import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { sendChooseFolder } from "./runtime";
import { focusRing, inputClass } from "./settings-shared";
import type { AgentInfo, RuntimeResponse } from "./types";

export function FieldMessage({ id, error }: { id: string; error?: string }) {
  if (!error) return null;
  return (
    <p id={id} className="mt-1 text-xs leading-snug text-red-600" data-testid="field-error">
      {error}
    </p>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  description,
  disabled = false,
  testId,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  description?: ReactNode;
  disabled?: boolean;
  testId?: string;
}) {
  const id = useId();
  return (
    <div className="flex items-start gap-2.5">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-labelledby={`${id}-label`}
        aria-describedby={description ? `${id}-desc` : undefined}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`relative mt-0.5 inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${focusRing} ${
          checked ? "bg-indigo-600" : "bg-gray-300"
        }`}
        data-testid={testId}
      >
        <span
          aria-hidden="true"
          className={`inline-block h-4 w-4 rounded-full bg-white shadow transition-transform ${checked ? "translate-x-[18px]" : "translate-x-0.5"}`}
        />
      </button>
      <div className="min-w-0">
        <span
          id={`${id}-label`}
          className="block cursor-default text-[13px] font-semibold leading-snug text-gray-900"
          onClick={() => !disabled && onChange(!checked)}
        >
          {label}
        </span>
        {description && (
          <span id={`${id}-desc`} className="mt-0.5 block text-xs leading-snug text-gray-500">
            {description}
          </span>
        )}
      </div>
    </div>
  );
}

/** Radio rows for the agents the CLI knows, with a Found / Not installed badge each. */
export function AgentChoice({
  agents,
  value,
  onChange,
  labelledBy,
  error,
}: {
  agents: AgentInfo[];
  value: string;
  onChange: (id: string) => void;
  labelledBy: string;
  error?: string;
}) {
  const name = useId();
  const errorId = `${name}-error`;
  return (
    <div>
      <div role="radiogroup" aria-labelledby={labelledBy} aria-describedby={error ? errorId : undefined} className="space-y-1.5">
        {agents.map((agent) => {
          const selected = agent.id === value;
          return (
            <label
              key={agent.id}
              className={`flex cursor-pointer items-center gap-2.5 rounded-lg border px-3 py-2 transition-colors focus-within:ring-2 focus-within:ring-indigo-500 focus-within:ring-offset-1 ${
                selected ? "border-indigo-500 bg-indigo-50/70" : "border-gray-200 bg-white hover:bg-gray-50"
              }`}
              data-testid={`agent-${agent.id}`}
            >
              <input
                type="radio"
                name={name}
                value={agent.id}
                checked={selected}
                onChange={() => onChange(agent.id)}
                className="h-3.5 w-3.5 shrink-0 accent-indigo-600 focus:outline-none"
              />
              <span className="min-w-0 flex-1 truncate text-[13px] text-gray-900">{agent.name}</span>
              {agent.found ? (
                <span className="shrink-0 rounded bg-green-50 px-1.5 py-0.5 text-[11px] font-medium text-green-700 ring-1 ring-inset ring-green-600/15">
                  Found
                </span>
              ) : (
                <span className="shrink-0 rounded bg-gray-100 px-1.5 py-0.5 text-[11px] font-medium text-gray-500">
                  Not installed
                </span>
              )}
            </label>
          );
        })}
      </div>
      <FieldMessage id={errorId} error={error} />
    </div>
  );
}

/**
 * A folder path with a "Choose…" button that opens the system dialog through
 * the app. When no dialog is available the reason shows and the path is typed.
 * `onResult` receives failed requests so the surface can move its app state.
 */
export function FolderField({
  value,
  onChange,
  labelledBy,
  error,
  onResult,
  testId = "folder-input",
}: {
  value: string;
  onChange: (value: string) => void;
  labelledBy: string;
  error?: string;
  onResult?: (response: RuntimeResponse) => void;
  testId?: string;
}) {
  const id = useId();
  const input = useRef<HTMLInputElement>(null);
  const [choosing, setChoosing] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const live = useRef(true);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);

  const choose = async () => {
    setChoosing(true);
    setNote(null);
    const response = await sendChooseFolder(value.trim() || undefined);
    if (!live.current) return;
    setChoosing(false);
    if (!response.success) {
      onResult?.(response);
      setNote(response.error);
      return;
    }
    if (response.data.type !== "choose_folder_result") {
      setNote(`Unexpected response type: ${response.data.type}`);
      return;
    }
    const result = response.data;
    if (result.outcome === "chosen" && result.path) {
      onChange(result.path_display ?? result.path);
    } else if (result.outcome === "unavailable") {
      setNote(`${result.reason ?? "No folder dialog is available."} Type the path instead.`);
      input.current?.focus();
    }
  };

  const describedBy = [error ? `${id}-error` : null, note ? `${id}-note` : null].filter(Boolean).join(" ") || undefined;

  return (
    <div>
      <div className="flex gap-1.5">
        <input
          ref={input}
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          aria-labelledby={labelledBy}
          aria-invalid={error ? true : undefined}
          aria-describedby={describedBy}
          spellCheck={false}
          autoComplete="off"
          className={`${inputClass} ${error ? "border-red-400" : "border-gray-300 focus:border-indigo-500"}`}
          data-testid={testId}
        />
        <button
          type="button"
          onClick={() => void choose()}
          disabled={choosing}
          className={`shrink-0 rounded-lg border border-gray-300 bg-white px-2.5 text-[13px] text-gray-700 shadow-sm hover:bg-gray-50 disabled:cursor-wait disabled:opacity-60 ${focusRing}`}
          data-testid="choose-folder"
        >
          {choosing ? "Choosing…" : "Choose…"}
        </button>
      </div>
      <FieldMessage id={`${id}-error`} error={error} />
      {note && (
        <p id={`${id}-note`} className="mt-1 text-xs leading-snug text-gray-500" role="status" data-testid="folder-note">
          {note}
        </p>
      )}
    </div>
  );
}

/** A small uppercase section heading, as in the Settings mockup. */
export function SectionLabel({ id, children }: { id?: string; children: ReactNode }) {
  return (
    <h2 id={id} className="text-[11px] font-semibold uppercase tracking-wider text-gray-500">
      {children}
    </h2>
  );
}
