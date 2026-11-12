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
  isConnectionError,
} from "../shared/runtime";
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
  | { status: "error"; message: string; isConnectionError: boolean };

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

export default function Popup() {
  const [quickSaveState, setQuickSaveState] = useState<QuickSaveState>({
    status: "saving",
  });
  const [popupMode, setPopupMode] = useState<PopupMode>({ mode: "quick-save" });

  const dispatched = useRef(false);
  const closeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const tabUrl = useRef<string>("");
  const tabTitle = useRef<string | undefined>(undefined);
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

  // Quick-save on mount
  useEffect(() => {
    if (dispatched.current) return;
    dispatched.current = true;

    (async () => {
      // An in-page button (e.g. on a LinkedIn post) may have prepared this save:
      // then the popup saves that item rather than the page in the active tab.
      const pending = await takePendingSave();

      let target: { url: string; title?: string; favIconUrl?: string };
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
            isConnectionError: false,
          });
          return;
        }
        target = tab;
        tabId.current = tab.id;
      }

      tabUrl.current = target.url;
      tabTitle.current = target.title;

      setQuickSaveState({ status: "saving", title: target.title, favIconUrl: target.favIconUrl });

      if (!pending && tabId.current !== undefined) {
        pageHtml.current = await queryPageHtml(tabId.current, target.url);
      }

      const response = await sendSaveMessage(target.url, target.title, {
        page_html: pageHtml.current,
      });

      if (!response.success) {
        setQuickSaveState({
          status: "error",
          message: response.error,
          isConnectionError: isConnectionError(response.error),
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
    })();

    return () => {
      clearCloseTimer();
    };
  }, [clearCloseTimer]);

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
    const response = await sendSaveMessage(tabUrl.current, tabTitle.current, {
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

      {quickSaveState.status === "error" && (
        <div data-testid="error-state">
          <h1 className="text-lg font-semibold text-red-700">Save Failed</h1>
          <p className="mt-1 text-sm text-gray-600">{quickSaveState.message}</p>
          {quickSaveState.isConnectionError && (
            <div className="mt-3 rounded border border-amber-200 bg-amber-50 p-2 text-xs text-amber-800" data-testid="install-guidance">
              <p className="font-medium">Couldn&apos;t reach AgentMark</p>
              <p className="mt-2">1. Reload the extension at <span className="font-mono">chrome://extensions</span> (↻ icon), then try again.</p>
              <p className="mt-2">2. If it still fails, register the native host and reload:</p>
              <code className="mt-1 block rounded bg-amber-100 p-1.5 text-[11px] select-all">agentmark install-extension --extension-id {chrome.runtime?.id ?? "YOUR_ID"}</code>
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
