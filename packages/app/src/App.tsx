/**
 * The app's one window. On every launch it runs the three install steps
 * (idempotent), then shows Home with what is left to do: Setup, then Add the
 * Chrome extension. Settings and About open from the header once set up.
 * No router: one `view` state, like the extension's side panel.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorText } from "./api";
import Header, { type View } from "./components/Header";
import About from "./screens/About";
import Extension from "./screens/Extension";
import Home from "./screens/Home";
import Settings from "./screens/Settings";
import Setup from "./screens/Setup";
import { STEP_ORDER, type StepState } from "./steps";
import type { ConnectionRecord, SetupState, StepId, StepReport } from "./types";

export type Steps = Record<StepId, StepState>;

const PENDING: Steps = { cli: "pending", native_host: "pending", skill: "pending" };

function failedReport(id: StepId, err: unknown): StepReport {
  return { id, status: "failed", summary: "This step could not run.", detail: errorText(err) };
}

export default function App({ initialView = "home" }: { initialView?: View }) {
  const [view, setView] = useState<View>(initialView);
  const [steps, setSteps] = useState<Steps>(PENDING);
  const [state, setState] = useState<SetupState | null>(null);
  const [stateError, setStateError] = useState<string | null>(null);
  const [connection, setConnection] = useState<ConnectionRecord | null>(null);
  const started = useRef(false);

  const setStep = (id: StepId, value: StepState) => setSteps((prev) => ({ ...prev, [id]: value }));

  const runStep = useCallback(async (id: StepId) => {
    setStep(id, "running");
    try {
      setStep(id, await api.runStep(id));
    } catch (err) {
      setStep(id, failedReport(id, err));
    }
  }, []);

  const loadState = useCallback(async () => {
    try {
      setState(await api.getSetupState());
      setStateError(null);
    } catch (err) {
      setStateError(errorText(err));
    }
  }, []);

  const loadConnection = useCallback(async () => {
    try {
      setConnection(await api.getConnection());
    } catch {
      setConnection(null);
    }
  }, []);

  useEffect(() => {
    // StrictMode mounts twice in development; the steps run once per launch.
    if (started.current) return;
    started.current = true;
    void (async () => {
      // Steps run in order: the host manifest points at the CLI copy step 1 makes.
      for (const id of STEP_ORDER) await runStep(id);
    })();
    void loadState();
    void loadConnection();
  }, [runStep, loadState, loadConnection]);

  const go = (next: View) => {
    if (next === "home") void loadConnection();
    setView(next);
  };

  const initialized = state?.initialized === true;

  return (
    <div className="flex h-screen flex-col overflow-hidden bg-neutral-50 text-neutral-900 antialiased dark:bg-neutral-900 dark:text-neutral-100">
      <Header view={view} onNavigate={go} tabs={initialized} />
      <main className="flex min-h-0 flex-1 flex-col">
        {view === "home" && (
          <Home
            steps={steps}
            state={state}
            stateError={stateError}
            connection={connection}
            onRetry={(id) => void runStep(id)}
            onReloadState={() => void loadState()}
            onNavigate={go}
          />
        )}
        {view === "setup" && (
          <Setup
            onBack={() => go("home")}
            onDone={(next) => {
              setState(next);
              go(connection ? "home" : "extension");
            }}
          />
        )}
        {view === "extension" && (
          <Extension
            nativeHost={steps.native_host}
            onNativeHost={(report) => setStep("native_host", report)}
            onConnection={setConnection}
            onDone={() => go("home")}
          />
        )}
        {view === "settings" && <Settings connection={connection} onSaved={setState} />}
        {view === "about" && <About />}
      </main>
    </div>
  );
}
