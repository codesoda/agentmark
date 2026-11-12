/**
 * The app state for the popup and the side panel (docs/setup-contract.md §4):
 * probes with `status` on mount, re-probes every `POLL_INTERVAL_MS` (after
 * the previous probe settles, never overlapping) while the app is missing or
 * registered for another extension, and stops on any other state or unmount.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  appStateFromResult,
  appStatusFromProbe,
  POLL_INTERVAL_MS,
  POLLING_STATES,
  type AppState,
  type AppStatus,
} from "./connection";
import { sendStatus } from "./runtime";
import type { RuntimeResponse } from "./types";

export interface UseAppStateOptions {
  /** Side panel: also pause polling while the panel is hidden. */
  pauseWhenHidden?: boolean;
}

export interface AppStateHandle {
  status: AppStatus;
  /** Probe now (the "Check again" button); the polling timer restarts after it. */
  checkAgain: () => void;
  /**
   * Let a later request's result move the state (contract §4.2). Returns the
   * new state, or null when the result says nothing about the app.
   * `setupMessage`: the request was `get_setup_state` or `save_settings`.
   */
  report: (response: RuntimeResponse, setupMessage?: boolean) => AppState | null;
  /** Set the state directly, e.g. `ready` once setup has saved. */
  set: (status: AppStatus) => void;
}

function useDocumentVisible(enabled: boolean): boolean {
  const read = () => typeof document === "undefined" || document.visibilityState === "visible";
  const [visible, setVisible] = useState(read);
  useEffect(() => {
    if (!enabled) return;
    const onChange = () => setVisible(read());
    document.addEventListener("visibilitychange", onChange);
    return () => document.removeEventListener("visibilitychange", onChange);
  }, [enabled]);
  return enabled ? visible : true;
}

export function useAppState(options: UseAppStateOptions = {}): AppStateHandle {
  const [status, setStatus] = useState<AppStatus>({ state: "checking" });
  /** Bumped when a probe settles, so the next poll is timed from then. */
  const [settledCount, setSettledCount] = useState(0);
  const inflight = useRef(false);
  const mounted = useRef(true);
  const visible = useDocumentVisible(options.pauseWhenHidden === true);

  const probe = useCallback(async () => {
    if (inflight.current) return;
    inflight.current = true;
    let next: AppStatus;
    try {
      next = appStatusFromProbe(await sendStatus());
    } catch (err) {
      next = { state: "host_crashed", detail: err instanceof Error ? err.message : String(err) };
    }
    inflight.current = false;
    if (!mounted.current) return;
    setStatus((prev) => ({ ...next, version: next.version ?? prev.version }));
    setSettledCount((n) => n + 1);
  }, []);

  useEffect(() => {
    mounted.current = true;
    void probe();
    return () => {
      mounted.current = false;
    };
  }, [probe]);

  useEffect(() => {
    if (!POLLING_STATES.has(status.state) || !visible) return;
    const timer = setTimeout(() => void probe(), POLL_INTERVAL_MS);
    return () => clearTimeout(timer);
  }, [status.state, settledCount, visible, probe]);

  const report = useCallback((response: RuntimeResponse, setupMessage = false) => {
    const next = appStateFromResult(response, setupMessage);
    if (next) {
      const detail = response.success ? undefined : response.error;
      setStatus((prev) => ({ state: next, detail, version: prev.version }));
    }
    return next;
  }, []);

  const set = useCallback((next: AppStatus) => {
    setStatus((prev) => ({ ...next, version: next.version ?? prev.version }));
  }, []);

  const checkAgain = useCallback(() => void probe(), [probe]);

  return useMemo(() => ({ status, checkAgain, report, set }), [status, checkAgain, report, set]);
}
