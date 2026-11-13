/**
 * Form controls for the Setup and Settings screens. Same behaviour and
 * wording as the extension's `settings-controls.tsx`; the folder button opens
 * the native dialog through the app instead of the native host.
 */

import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { api, errorText } from "../api";
import type { AgentInfo } from "../types";
import { focusRing, inputBad, inputBase, inputOk, secondaryButton } from "./ui";

export function FieldMessage({ id, error }: { id: string; error?: string }) {
  if (!error) return null;
  return (
    <p id={id} className="mt-1.5 text-xs leading-snug text-red-600 dark:text-red-400" data-testid="field-error">
      {error}
    </p>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  description,
  testId,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  description?: ReactNode;
  testId?: string;
}) {
  const id = useId();
  return (
    <div className="flex items-start gap-3">
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-labelledby={`${id}-label`}
        aria-describedby={description ? `${id}-desc` : undefined}
        onClick={() => onChange(!checked)}
        className={`relative mt-px inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full transition-colors ${focusRing} ${
          checked ? "bg-accent" : "bg-neutral-300 dark:bg-white/20"
        }`}
        data-testid={testId}
      >
        <span
          aria-hidden="true"
          className={`inline-block h-[18px] w-[18px] rounded-full bg-white shadow-sm shadow-black/20 transition-transform ${
            checked ? "translate-x-[18px]" : "translate-x-[2px]"
          }`}
        />
      </button>
      <div className="min-w-0">
        <span
          id={`${id}-label`}
          className="block cursor-default text-[13px] font-medium leading-snug text-neutral-900 dark:text-neutral-100"
          onClick={() => onChange(!checked)}
        >
          {label}
        </span>
        {description && (
          <span id={`${id}-desc`} className="mt-0.5 block text-xs leading-snug text-neutral-500 dark:text-neutral-400">
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
      <div
        role="radiogroup"
        aria-labelledby={labelledBy}
        aria-describedby={error ? errorId : undefined}
        className="space-y-1.5"
      >
        {agents.map((agent) => {
          const selected = agent.id === value;
          return (
            <label
              key={agent.id}
              className={`flex h-9 cursor-pointer items-center gap-2.5 rounded-lg border px-3 transition-colors focus-within:ring-[3px] focus-within:ring-accent/20 ${
                selected
                  ? "border-accent/70 bg-accent-soft/70 dark:border-indigo-400/60 dark:bg-indigo-400/10"
                  : "border-neutral-200 bg-white hover:bg-neutral-50 dark:border-white/10 dark:bg-white/[0.03] dark:hover:bg-white/[0.06]"
              }`}
              data-testid={`agent-${agent.id}`}
            >
              <input
                type="radio"
                name={name}
                value={agent.id}
                checked={selected}
                onChange={() => onChange(agent.id)}
                className="h-3.5 w-3.5 shrink-0 accent-accent focus:outline-none"
              />
              <span className="min-w-0 flex-1 truncate text-[13px] text-neutral-900 dark:text-neutral-100">{agent.name}</span>
              {agent.found ? (
                <span className="shrink-0 rounded-md bg-emerald-50 px-1.5 py-0.5 text-2xs font-medium text-emerald-700 ring-1 ring-inset ring-emerald-600/15 dark:bg-emerald-400/10 dark:text-emerald-300 dark:ring-emerald-400/20">
                  Found
                </span>
              ) : (
                <span className="shrink-0 rounded-md bg-neutral-100 px-1.5 py-0.5 text-2xs font-medium text-neutral-500 dark:bg-white/[0.06] dark:text-neutral-400">
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
 * A folder path with a "Choose…" button that opens the system folder dialog.
 * When no dialog is available the reason shows and the path is typed.
 */
export function FolderField({
  value,
  onChange,
  labelledBy,
  error,
}: {
  value: string;
  onChange: (value: string) => void;
  labelledBy: string;
  error?: string;
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
    try {
      const result = await api.chooseFolder(value.trim() || undefined);
      if (!live.current) return;
      if (result.outcome === "chosen" && result.path) {
        onChange(result.path_display ?? result.path);
      } else if (result.outcome === "unavailable") {
        setNote(`${result.reason ?? "No folder dialog is available."} Type the path instead.`);
        input.current?.focus();
      }
    } catch (err) {
      if (live.current) setNote(`${errorText(err)} Type the path instead.`);
    } finally {
      if (live.current) setChoosing(false);
    }
  };

  const describedBy = [error ? `${id}-error` : null, note ? `${id}-note` : null].filter(Boolean).join(" ") || undefined;

  return (
    <div>
      <div className="flex gap-2">
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
          autoCorrect="off"
          autoCapitalize="off"
          className={`${inputBase} h-8 font-mono ${error ? inputBad : inputOk}`}
          data-testid="folder-input"
        />
        <button type="button" onClick={() => void choose()} disabled={choosing} className={secondaryButton} data-testid="choose-folder">
          {choosing ? "Choosing…" : "Choose…"}
        </button>
      </div>
      <FieldMessage id={`${id}-error`} error={error} />
      {note && (
        <p id={`${id}-note`} className="mt-1.5 text-xs leading-snug text-neutral-500 dark:text-neutral-400" role="status" data-testid="folder-note">
          {note}
        </p>
      )}
    </div>
  );
}

/** A small uppercase section heading, as in the Settings mockup. */
export function SectionLabel({ id, children }: { id?: string; children: ReactNode }) {
  return (
    <h2 id={id} className="text-2xs font-semibold uppercase tracking-[0.06em] text-neutral-500 dark:text-neutral-400">
      {children}
    </h2>
  );
}

/** A spinner sized for a status row. */
export function Spinner({ className = "h-4 w-4" }: { className?: string }) {
  return (
    <svg className={`animate-spin ${className}`} viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="8" r="6.25" stroke="currentColor" strokeOpacity="0.2" strokeWidth="1.5" />
      <path d="M14.25 8A6.25 6.25 0 0 0 8 1.75" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );
}
