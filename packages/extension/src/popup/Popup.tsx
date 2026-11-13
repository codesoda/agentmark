import { useCallback, useEffect, useRef, useState } from "react";
import {
  queryActiveTab,
  sendSaveMessage,
  sendListCollectionsMessage,
  loadLastUsedIntent,
  saveLastUsedIntent,
  querySelectedText,
  queryPageHtml,
  takePendingSave,
  type ActiveTab,
} from "../shared/runtime";
import { appStateFromResult, classifyConnectionError, type ConnectionProblem } from "../shared/connection";
import InstallPrompt from "../shared/InstallPrompt";
import SetupForm from "../shared/SetupForm";
import UpdateRequired from "../shared/UpdateRequired";
import { useAppState } from "../shared/useAppState";
import { loadSetupDraft, updateSetupDraft } from "../shared/setup-draft";
import type { PendingSave, SaveResultResponse, SetupState } from "../shared/types";
import SaveForm, { type SaveFormValues } from "./SaveForm";

type QuickSaveState =
  | { status: "saving"; title?: string; favIconUrl?: string }
  | {
      status: "success";
      title?: string;
      favIconUrl?: string;
      bookmarkId: string;
      saveStatus: string;
    }
  | { status: "error"; message: string; problem: ConnectionProblem | null };

interface SaveTarget {
  url: string;
  title?: string;
  favIconUrl?: string;
}

interface FormContext {
  collections: string[];
  initialTags: string[];
  initialCollection?: string;
  selectedText: string;
}

type PopupMode =
  | { mode: "quick-save" }
  | { mode: "form"; context: FormContext; submitting: boolean; error?: string };

const AUTO_CLOSE_MS = 2000;

/** App states the popup shows as a setup screen instead of the save. */
type GateProblem = "not_installed" | "forbidden" | "worker_unreachable";

function isGateProblem(state: string): state is GateProblem {
  return state === "not_installed" || state === "forbidden" || state === "worker_unreachable";
}

/**
 * Where the page to save comes from; resolved once, when the popup opens.
 * `fromTab` false: an in-page item (a post), handed over by its button.
 */
type TargetLookup =
  | { status: "pending" }
  | { status: "ready"; target: SaveTarget; fromTab: boolean }
  | { status: "failed"; message: string };

export default function Popup() {
  const app = useAppState();
  const appState = app.status.state;
  const { report, set: setAppStatus } = app;
  const [quickSaveState, setQuickSaveState] = useState<QuickSaveState>({
    status: "saving",
  });
  const [popupMode, setPopupMode] = useState<PopupMode>({ mode: "quick-save" });
  const [targetLookup, setTargetLookup] = useState<TargetLookup>({ status: "pending" });

  const resolving = useRef(false);
  /** Set once a save has been sent; cleared when the app turns out not to be ready. */
  const saveStarted = useRef(false);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const saveTarget = useRef<SaveTarget | null>(null);
  const tabId = useRef<number | undefined>(undefined);
  /** Whether the target is the active tab's page, which is snapshotted at save time. */
  const snapshotTab = useRef<number | undefined>(undefined);
  // Captured once so the quick-save and the form re-save see identical content.
  const pageHtml = useRef<string | undefined>(undefined);
  /** The in-page item this popup saves, kept in the setup draft while setup interrupts it. */
  const pendingItem = useRef<PendingSave | null>(null);
  /** The item came from the setup draft (an earlier popup of this setup), not a click just now. */
  const itemFromDraft = useRef(false);
  /** The active tab, in case a drafted item turns out not to apply. */
  const activeTab = useRef<ActiveTab | null>(null);
  /** This popup showed setup: the app was not set up when it opened. */
  const setupShown = useRef(false);

  const clearCloseTimer = useCallback(() => {
    if (closeTimer.current) {
      clearTimeout(closeTimer.current);
      closeTimer.current = null;
    }
  }, []);

  const startCloseTimer = useCallback(() => {
    clearCloseTimer();
    closeTimer.current = setTimeout(() => {
      window.close();
    }, AUTO_CLOSE_MS);
  }, [clearCloseTimer]);

  // Resolve what to save as soon as the popup opens, while the app is probed.
  // An in-page save can be taken only once (and expires after a few seconds),
  // so it is taken now even if setup comes first.
  useEffect(() => {
    if (resolving.current) return;
    resolving.current = true;
    void (async () => {
      // An in-page button (e.g. on a LinkedIn post) may have prepared this save:
      // then the popup saves that item rather than the page in the active tab.
      // If setup interrupted such a save in an earlier popup (the folder dialog
      // closes it), the item waits in the setup draft.
      const fresh = await takePendingSave();
      const drafted = fresh ? null : ((await loadSetupDraft())?.pending_save ?? null);
      const pending = fresh ?? drafted;
      let tab: ActiveTab | null = null;
      let tabError: string | null = null;
      if (!fresh) {
        try {
          tab = await queryActiveTab();
        } catch (err) {
          tabError = err instanceof Error ? err.message : String(err);
        }
      }
      activeTab.current = tab;
      if (pending) {
        pendingItem.current = pending;
        itemFromDraft.current = drafted !== null;
        targetItem(pending);
        return;
      }
      if (tab) {
        targetTab(tab);
      } else {
        setTargetLookup({ status: "failed", message: tabError ?? "No active tab found" });
      }
    })();
  }, []);

  function targetItem(item: PendingSave) {
    saveTarget.current = { url: item.url, title: item.title };
    pageHtml.current = item.page_html;
    snapshotTab.current = undefined;
    tabId.current = undefined;
    setTargetLookup({ status: "ready", target: saveTarget.current, fromTab: false });
    setQuickSaveState({ status: "saving", title: item.title });
  }

  function targetTab(tab: ActiveTab) {
    saveTarget.current = tab;
    tabId.current = tab.id;
    snapshotTab.current = tab.id;
    pageHtml.current = undefined;
    setTargetLookup({ status: "ready", target: tab, fromTab: true });
    setQuickSaveState({ status: "saving", title: tab.title, favIconUrl: tab.favIconUrl });
  }

  // Setup interrupts an in-page save: keep the item in the setup draft, so a
  // popup reopened after the folder dialog still saves it.
  useEffect(() => {
    if (appState !== "setup_required") return;
    setupShown.current = true;
    if (pendingItem.current && !itemFromDraft.current) void updateSetupDraft({ pending_save: pendingItem.current });
  }, [appState, targetLookup]);

  // A drafted item belongs to a setup that is no longer in progress (the app
  // was set up elsewhere): forget it and save the active tab as usual.
  useEffect(() => {
    if (appState !== "ready" || !itemFromDraft.current || setupShown.current) return;
    itemFromDraft.current = false;
    pendingItem.current = null;
    void updateSetupDraft({ pending_save: null });
    if (activeTab.current) {
      targetTab(activeTab.current);
    } else {
      setTargetLookup({ status: "failed", message: "No active tab found" });
    }
  }, [appState, targetLookup]);

  // Saves the resolved target. A retry reuses it and its snapshot.
  const runQuickSave = useCallback(async () => {
    const target = saveTarget.current;
    if (!target) return;
    saveStarted.current = true;
    setQuickSaveState({ status: "saving", title: target.title, favIconUrl: target.favIconUrl });

    if (snapshotTab.current !== undefined) {
      const tab = snapshotTab.current;
      snapshotTab.current = undefined;
      pageHtml.current = await queryPageHtml(tab, target.url);
    }

    const response = await sendSaveMessage(target.url, target.title, {
      page_html: pageHtml.current,
    });

    if (!response.success) {
      // Not set up, or the app went away: show that screen, and save again once it is fixed.
      const next = appStateFromResult(response);
      if (next && next !== "host_crashed") {
        saveStarted.current = false;
        report(response);
        return;
      }
      setQuickSaveState({
        status: "error",
        message: response.error,
        problem: classifyConnectionError(response.error),
      });
      return;
    }

    const data = response.data as SaveResultResponse;
    setQuickSaveState({
      status: "success",
      title: target.title,
      favIconUrl: target.favIconUrl,
      bookmarkId: data.id,
      saveStatus: data.status,
    });
  }, [report]);

  // Save once the app is ready and the target is known: at once on a normal
  // open, or after setup or an install finishes.
  useEffect(() => {
    if (appState !== "ready" || targetLookup.status !== "ready" || saveStarted.current) return;
    if (itemFromDraft.current && !setupShown.current) return; // Swapped for the tab first, above.
    void runQuickSave();
  }, [appState, targetLookup, runQuickSave]);

  const handleSetupDone = useCallback(
    (state: SetupState) => {
      setAppStatus({ state: "ready", version: state.version });
    },
    [setAppStatus],
  );

  useEffect(() => clearCloseTimer, [clearCloseTimer]);

  // Auto-close on success (only in quick-save mode)
  useEffect(() => {
    if (quickSaveState.status === "success" && popupMode.mode === "quick-save") {
      startCloseTimer();
      return () => {
        clearCloseTimer();
      };
    }
  }, [quickSaveState.status, popupMode.mode, startCloseTimer, clearCloseTimer]);

  const handleMoreOptions = async () => {
    clearCloseTimer();

    // Lazily fetch collections, defaults, and selected text
    const [collections, defaults, selectedText] = await Promise.all([
      sendListCollectionsMessage(),
      loadLastUsedIntent(),
      tabId.current !== undefined ? querySelectedText(tabId.current) : Promise.resolve(""),
    ]);

    setPopupMode({
      mode: "form",
      context: {
        collections,
        initialTags: defaults.tags,
        initialCollection: defaults.collection,
        selectedText,
      },
      submitting: false,
    });
  };

  const handleFormSubmit = async (values: SaveFormValues) => {
    setPopupMode((prev) =>
      prev.mode === "form" ? { ...prev, submitting: true, error: undefined } : prev,
    );

    // Re-save the same URL with intent fields — merge semantics will handle dedup
    const response = await sendSaveMessage(saveTarget.current?.url ?? "", saveTarget.current?.title, {
      tags: values.tags,
      collection: values.collection,
      note: values.note,
      action: values.action,
      selected_text: values.selected_text,
      page_html: pageHtml.current,
    });

    if (!response.success) {
      setPopupMode((prev) =>
        prev.mode === "form"
          ? { ...prev, submitting: false, error: response.error }
          : prev,
      );
      return;
    }

    // Persist only last-used tags and collection (best-effort)
    await saveLastUsedIntent({
      tags: values.tags,
      collection: values.collection,
    });

    const data = response.data as SaveResultResponse;
    setQuickSaveState((prev) => ({
      ...prev,
      status: "success",
      bookmarkId: data.id,
      saveStatus: data.status,
    } as QuickSaveState));
    setPopupMode({ mode: "quick-save" });
    startCloseTimer();
  };

  const handleFormCancel = () => {
    setPopupMode({ mode: "quick-save" });
    if (quickSaveState.status === "success") {
      startCloseTimer();
    }
  };

  // Intent form mode
  if (popupMode.mode === "form") {
    return (
      <div className="w-80 p-4" data-testid="form-mode">
        <h1 className="mb-3 text-sm font-semibold text-gray-900">Save with intent</h1>
        <SaveForm
          initialTags={popupMode.context.initialTags}
          initialCollection={popupMode.context.initialCollection}
          initialSelectedText={popupMode.context.selectedText}
          collections={popupMode.context.collections}
          onSubmit={handleFormSubmit}
          onCancel={handleFormCancel}
          submitting={popupMode.submitting}
          error={popupMode.error}
        />
      </div>
    );
  }

  if (isGateProblem(appState)) {
    return (
      <div className="w-80 p-4">
        <InstallPrompt problem={appState} onRetry={app.checkAgain} detail={app.status.detail} />
      </div>
    );
  }

  if (appState === "update_required") {
    return (
      <div className="w-80 p-4">
        <UpdateRequired onCheckAgain={app.checkAgain} version={app.status.version} />
      </div>
    );
  }

  if (appState === "setup_required") {
    return (
      <div className="w-80 p-4">
        <SetupForm
          submitLabel={
            targetLookup.status === "failed"
              ? "Finish setup"
              : targetLookup.status === "ready" && !targetLookup.fromTab
                ? "Finish and save this post"
                : "Finish and save this page"
          }
          useDraft
          onDone={handleSetupDone}
          onResult={app.report}
        />
      </div>
    );
  }

  if (appState === "host_crashed" && !saveStarted.current) {
    return (
      <div className="w-80 p-4" data-testid="error-state">
        <InstallPrompt problem="host_crashed" onRetry={app.checkAgain} detail={app.status.detail} />
      </div>
    );
  }

  if (targetLookup.status === "failed" && appState !== "checking") {
    return (
      <div className="w-80 p-4">
        <div data-testid="error-state">
          <h1 className="text-lg font-semibold text-red-700">Save Failed</h1>
          <p className="mt-1 text-sm text-gray-600">{targetLookup.message}</p>
          {appState === "ready" && <p className="mt-2 text-xs text-gray-400">AgentMark saves web pages (http and https).</p>}
        </div>
      </div>
    );
  }

  // Quick-save mode
  return (
    <div className="w-80 p-4">
      {quickSaveState.status === "saving" && (
        <div data-testid="saving-state">
          <div className="flex items-center gap-2">
            {quickSaveState.favIconUrl && (
              <img
                src={quickSaveState.favIconUrl}
                alt=""
                className="h-4 w-4"
              />
            )}
            <h1 className="text-lg font-semibold text-gray-900">Saving...</h1>
          </div>
          {quickSaveState.title && (
            <p className="mt-1 truncate text-sm text-gray-600">{quickSaveState.title}</p>
          )}
        </div>
      )}

      {quickSaveState.status === "success" && (
        <div data-testid="success-state">
          <div className="flex items-center gap-2">
            {quickSaveState.favIconUrl && (
              <img
                src={quickSaveState.favIconUrl}
                alt=""
                className="h-4 w-4"
              />
            )}
            <h1 className="text-lg font-semibold text-green-700">Saved!</h1>
          </div>
          {quickSaveState.title && (
            <p className="mt-1 truncate text-sm text-gray-600">{quickSaveState.title}</p>
          )}
          <p className="mt-1 text-xs text-gray-400">
            ID: {quickSaveState.bookmarkId}
          </p>
          <button
            type="button"
            onClick={handleMoreOptions}
            className="mt-2 text-xs text-indigo-600 hover:text-indigo-800"
            data-testid="more-options-btn"
          >
            More options
          </button>
        </div>
      )}

      {quickSaveState.status === "error" && (
        <div data-testid="error-state">
          <h1 className="text-lg font-semibold text-red-700">Save Failed</h1>
          <p className="mt-1 text-sm text-gray-600">{quickSaveState.message}</p>
          {quickSaveState.problem && (
            <div className="mt-3">
              <InstallPrompt problem={quickSaveState.problem} onRetry={() => void runQuickSave()} />
            </div>
          )}
          <button
            type="button"
            onClick={handleMoreOptions}
            className="mt-2 text-xs text-indigo-600 hover:text-indigo-800"
            data-testid="more-options-btn"
          >
            More options
          </button>
        </div>
      )}
    </div>
  );
}
