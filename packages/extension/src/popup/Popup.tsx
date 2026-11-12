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
} from "../shared/runtime";
import { classifyConnectionError, type ConnectionProblem } from "../shared/connection";
import InstallPrompt from "../shared/InstallPrompt";
import type { SaveResultResponse } from "../shared/types";
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

/** Problems the popup shows as setup steps instead of a failed save. */
function isSetupProblem(
  problem: ConnectionProblem | null,
): problem is "not_installed" | "forbidden" | "worker_unreachable" {
  return problem !== null && problem !== "host_crashed";
}

export default function Popup() {
  const [quickSaveState, setQuickSaveState] = useState<QuickSaveState>({
    status: "saving",
  });
  const [popupMode, setPopupMode] = useState<PopupMode>({ mode: "quick-save" });

  const dispatched = useRef(false);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const saveTarget = useRef<SaveTarget | null>(null);
  const tabId = useRef<number | undefined>(undefined);
  // Captured once so the quick-save and the form re-save see identical content.
  const pageHtml = useRef<string | undefined>(undefined);

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

  // Saves the target, resolving it on the first run. A retry reuses it: an
  // in-page save can only be taken once, so re-resolving would save the tab.
  const runQuickSave = useCallback(async () => {
    let target = saveTarget.current;
    let snapshotTab: number | undefined;
    if (!target) {
      // An in-page button (e.g. on a LinkedIn post) may have prepared this save:
      // then the popup saves that item rather than the page in the active tab.
      const pending = await takePendingSave();
      if (pending) {
        target = { url: pending.url, title: pending.title };
        pageHtml.current = pending.page_html;
      } else {
        let tab;
        try {
          tab = await queryActiveTab();
        } catch (err) {
          setQuickSaveState({
            status: "error",
            message: err instanceof Error ? err.message : String(err),
            problem: null,
          });
          return;
        }
        target = tab;
        tabId.current = tab.id;
        snapshotTab = tab.id;
      }
      saveTarget.current = target;
    }

    setQuickSaveState({ status: "saving", title: target.title, favIconUrl: target.favIconUrl });

    if (snapshotTab !== undefined) {
      pageHtml.current = await queryPageHtml(snapshotTab, target.url);
    }

    const response = await sendSaveMessage(target.url, target.title, {
      page_html: pageHtml.current,
    });

    if (!response.success) {
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
  }, []);

  // Quick-save on mount
  useEffect(() => {
    if (dispatched.current) return;
    dispatched.current = true;
    void runQuickSave();
  }, [runQuickSave]);

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

      {quickSaveState.status === "error" && isSetupProblem(quickSaveState.problem) && (
        <InstallPrompt
          problem={quickSaveState.problem}
          onRetry={() => void runQuickSave()}
          detail={quickSaveState.message}
        />
      )}

      {quickSaveState.status === "error" && !isSetupProblem(quickSaveState.problem) && (
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
