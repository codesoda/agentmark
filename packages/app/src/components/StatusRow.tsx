import type { ReactNode } from "react";
import { Spinner } from "./controls";

export type RowState = "pending" | "running" | "done" | "warning" | "waiting" | "failed";

const ICON_LABEL: Record<RowState, string> = {
  pending: "Not started",
  running: "In progress",
  done: "Done",
  warning: "Done, with a note",
  waiting: "Waiting",
  failed: "Failed",
};

export function StatusIcon({ state }: { state: RowState }) {
  const label = ICON_LABEL[state];
  if (state === "running") {
    return (
      <span role="img" aria-label={label} className="flex h-5 w-5 items-center justify-center text-neutral-400 dark:text-neutral-500">
        <Spinner />
      </span>
    );
  }
  if (state === "pending") {
    return (
      <span role="img" aria-label={label} className="flex h-5 w-5 items-center justify-center">
        <span className="h-[15px] w-[15px] rounded-full border-[1.5px] border-dashed border-neutral-300 dark:border-neutral-600" />
      </span>
    );
  }
  const tone =
    state === "done"
      ? "bg-emerald-500 dark:bg-emerald-500/90"
      : state === "failed"
        ? "bg-red-500"
        : "bg-amber-500 dark:bg-amber-400";
  return (
    <span role="img" aria-label={label} className={`flex h-5 w-5 items-center justify-center rounded-full ${tone}`}>
      <svg viewBox="0 0 16 16" className="h-3 w-3 text-white" fill="none" aria-hidden="true">
        {state === "done" && (
          <path d="M4 8.4 6.6 11 12 5.4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
        )}
        {state === "failed" && <path d="M5.2 5.2l5.6 5.6M10.8 5.2l-5.6 5.6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />}
        {(state === "warning" || state === "waiting") && (
          <>
            <path d="M8 4.2v4.6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            <circle cx="8" cy="11.6" r="1.1" fill="currentColor" />
          </>
        )}
      </svg>
    </span>
  );
}

/** One row of a status card: icon, title, a sentence, an optional detail and action. */
export function StatusRow({
  state,
  title,
  summary,
  detail,
  action,
  testId,
}: {
  state: RowState;
  title: string;
  summary?: string | null;
  detail?: string | null;
  action?: ReactNode;
  testId?: string;
}) {
  const tone =
    state === "failed"
      ? "text-red-700 dark:text-red-300"
      : state === "waiting" || state === "warning"
        ? "text-amber-700 dark:text-amber-300"
        : "text-neutral-600 dark:text-neutral-400";
  return (
    <li className="flex items-start gap-3 px-4 py-3" data-testid={testId} data-state={state}>
      <div className="pt-px">
        <StatusIcon state={state} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-start justify-between gap-3">
          <p className="pt-px text-[13px] font-medium leading-5 text-neutral-900 dark:text-neutral-100">{title}</p>
          {action && <div className="-my-0.5 shrink-0">{action}</div>}
        </div>
        {summary && <p className={`mt-0.5 text-xs leading-snug ${tone}`}>{summary}</p>}
        {detail && (
          <p className="mt-0.5 break-words text-xs leading-snug text-neutral-500 dark:text-neutral-500" data-testid="row-detail">
            {detail}
          </p>
        )}
      </div>
    </li>
  );
}
