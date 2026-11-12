import { useCallback, useEffect, useRef, useState } from "react";
import {
  sendListBookmarksMessage,
  sendListCollectionsMessage,
  sendShowBookmarkMessage,
  sendUpdateBookmarkMessage,
} from "../shared/runtime";
import { appStateFromResult, classifyConnectionError } from "../shared/connection";
import AgentMarkMark from "../shared/AgentMarkMark";
import { SIDEPANEL_VIEW_KEY } from "../shared/constants";
import InstallPrompt from "../shared/InstallPrompt";
import SetupForm from "../shared/SetupForm";
import UpdateRequired from "../shared/UpdateRequired";
import { useAppState } from "../shared/useAppState";
import type { BookmarkSummary, BookmarkDetail as BookmarkDetailType, BookmarkChanges, BookmarkStateFilter, SetupState } from "../shared/types";
import BookmarkList, { type FilterValue } from "./BookmarkList";
import BookmarkDetail from "./BookmarkDetail";
import SettingsView from "./SettingsView";

type View = "list" | "detail";
/** The header switch. No router: plain view state, like list/detail. */
type Panel = "bookmarks" | "settings";

const DEFAULT_LIMIT = 50;

/**
 * Switch to the view the service worker asked for (the "Settings" item in the
 * toolbar icon's menu), then clear the request so a later open starts fresh.
 */
function useRequestedPanel(setPanel: (panel: Panel) => void): void {
  useEffect(() => {
    const session = chrome.storage?.session;
    if (!session) return;
    const take = (value: unknown) => {
      if (value !== "settings") return;
      setPanel("settings");
      void Promise.resolve(session.remove(SIDEPANEL_VIEW_KEY)).catch(() => {});
    };
    Promise.resolve(session.get(SIDEPANEL_VIEW_KEY))
      .then((items) => take(items?.[SIDEPANEL_VIEW_KEY]))
      .catch(() => {});
    const onChanged = (changes: Record<string, chrome.storage.StorageChange>, area: string) => {
      if (area === "session" && SIDEPANEL_VIEW_KEY in changes) take(changes[SIDEPANEL_VIEW_KEY].newValue);
    };
    chrome.storage.onChanged?.addListener(onChanged);
    return () => chrome.storage.onChanged?.removeListener(onChanged);
  }, [setPanel]);
}

export default function SidePanel() {
  const app = useAppState({ pauseWhenHidden: true });
  const appState = app.status.state;
  const { report, set: setAppStatus, checkAgain } = app;
  const ready = appState === "ready";
  // Setup and connection screens draw the mark beside their own title; the
  // header then leaves it out, so it shows once.
  const screenHasMark = !ready && appState !== "checking" && appState !== "host_crashed";
  const [panel, setPanel] = useState<Panel>("bookmarks");
  useRequestedPanel(setPanel);
  const [view, setView] = useState<View>("list");
  const [selectedBookmarkId, setSelectedBookmarkId] = useState<string | null>(null);
  const [bookmarks, setBookmarks] = useState<BookmarkSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeFilter, setActiveFilter] = useState<FilterValue>("all");

  // Detail state
  const [selectedBookmark, setSelectedBookmark] = useState<BookmarkDetailType | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);
  const [mutationError, setMutationError] = useState<string | null>(null);
  const [collections, setCollections] = useState<string[]>([]);

  // Stale-response guard: only apply results from the latest request
  const requestSeqRef = useRef(0);
  const detailSeqRef = useRef(0);

  const loadBookmarks = useCallback((filter: FilterValue) => {
    const seq = ++requestSeqRef.current;
    setLoading(true);
    setError(null);

    const state: BookmarkStateFilter | undefined =
      filter === "all" ? undefined : filter;

    sendListBookmarksMessage({ limit: DEFAULT_LIMIT, state }).then((result) => {
      // Ignore stale responses
      if (seq !== requestSeqRef.current) return;

      if (result.error) {
        // Not set up, or the app went away: the gate takes over.
        report({ success: false, error: result.error });
        setError(result.error);
        setBookmarks([]);
      } else {
        setBookmarks(result.bookmarks);
      }
      setLoading(false);
    });
  }, [report]);

  // Load once the app is ready, and again when the filter changes.
  useEffect(() => {
    if (ready) loadBookmarks(activeFilter);
  }, [ready, loadBookmarks, activeFilter]);

  // Refresh on window focus
  useEffect(() => {
    if (!ready) return;
    const handleFocus = () => {
      loadBookmarks(activeFilter);
    };
    window.addEventListener("focus", handleFocus);
    return () => window.removeEventListener("focus", handleFocus);
  }, [ready, loadBookmarks, activeFilter]);

  const handleRetry = useCallback(() => {
    loadBookmarks(activeFilter);
  }, [loadBookmarks, activeFilter]);

  const handleSetupDone = useCallback(
    (state: SetupState) => setAppStatus({ state: "ready", version: state.version }),
    [setAppStatus],
  );

  const connectionProblem = error ? classifyConnectionError(error) : null;

  const handleFilterChange = useCallback((filter: FilterValue) => {
    setActiveFilter(filter);
  }, []);

  const handleSelect = useCallback((id: string) => {
    setSelectedBookmarkId(id);
    setView("detail");
    setDetailLoading(true);
    setDetailError(null);
    setMutationError(null);
    setSelectedBookmark(null);

    const seq = ++detailSeqRef.current;

    Promise.all([
      sendShowBookmarkMessage(id),
      sendListCollectionsMessage(),
    ]).then(([showResult, collectionsList]) => {
      if (seq !== detailSeqRef.current) return;

      if (showResult.error) {
        if (appStateFromResult({ success: false, error: showResult.error }) === "setup_required") {
          report({ success: false, error: showResult.error });
        }
        setDetailError(showResult.error);
      } else if (showResult.bookmark) {
        setSelectedBookmark(showResult.bookmark);
        setCollections(collectionsList);
      }
      setDetailLoading(false);
    });
  }, [report]);

  const handleUpdate = useCallback(async (changes: BookmarkChanges) => {
    if (!selectedBookmarkId || updating) return;
    setUpdating(true);
    setMutationError(null);

    const result = await sendUpdateBookmarkMessage(selectedBookmarkId, changes);

    if (result.error) {
      setMutationError(result.error);
      setUpdating(false);
      throw new Error(result.error);
    }

    if (result.bookmark) {
      setSelectedBookmark(result.bookmark);

      // Patch list item locally
      setBookmarks((prev) =>
        prev.map((b) =>
          b.id === result.bookmark!.id
            ? {
                ...b,
                title: result.bookmark!.title,
                state: result.bookmark!.state,
                user_tags: result.bookmark!.user_tags,
                suggested_tags: result.bookmark!.suggested_tags,
              }
            : b,
        ),
      );

      // If state changed and the active filter would exclude it, remove from list
      if (
        changes.state &&
        activeFilter !== "all" &&
        changes.state !== activeFilter
      ) {
        setBookmarks((prev) => prev.filter((b) => b.id !== result.bookmark!.id));
      }
    }

    setUpdating(false);
  }, [selectedBookmarkId, updating, activeFilter]);

  const handleRetryDetail = useCallback(() => {
    if (selectedBookmarkId) handleSelect(selectedBookmarkId);
  }, [selectedBookmarkId, handleSelect]);

  const handleBackToList = useCallback(() => {
    setView("list");
    setSelectedBookmarkId(null);
    setSelectedBookmark(null);
    setDetailError(null);
    setMutationError(null);
  }, []);

  const header = (
    <header className="flex items-center gap-2 border-b border-gray-200 px-3 py-2.5">
      {!screenHasMark && <AgentMarkMark look={ready ? "saved" : "idle"} />}
      <h1 className="text-base font-semibold text-gray-900">AgentMark</h1>
      {ready && (
        <nav className="ml-auto flex gap-0.5 rounded-lg bg-gray-100 p-0.5" aria-label="AgentMark views" data-testid="panel-switch">
          {(["bookmarks", "settings"] as const).map((value) => (
            <button
              key={value}
              type="button"
              aria-pressed={panel === value}
              onClick={() => setPanel(value)}
              className={`rounded-md px-2.5 py-1 text-[13px] font-medium transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 ${
                panel === value ? "bg-white text-indigo-700 shadow-sm" : "text-gray-600 hover:text-gray-900"
              }`}
              data-testid={`panel-${value}`}
            >
              {value === "bookmarks" ? "Bookmarks" : "Settings"}
            </button>
          ))}
        </nav>
      )}
    </header>
  );

  return (
    <div className="flex h-screen flex-col">
      {header}
      {renderBody()}
    </div>
  );

  function renderBody() {
    if (!ready) {
      return <div className="min-h-0 flex-1 overflow-y-auto p-4">{renderGate()}</div>;
    }
    if (panel === "settings") return <SettingsView onResult={report} />;
    return renderBookmarks();
  }

  function renderGate() {
    switch (appState) {
      case "checking":
      case "ready":
        return (
          <div className="flex h-full items-center justify-center" data-testid="checking-state">
            <p className="text-sm text-gray-500">Connecting to AgentMark…</p>
          </div>
        );
      case "setup_required":
        return <SetupForm submitLabel="Finish setup" onDone={handleSetupDone} onResult={report} />;
      case "update_required":
        return <UpdateRequired onCheckAgain={checkAgain} version={app.status.version} />;
      default:
        return <InstallPrompt problem={appState} onRetry={checkAgain} detail={app.status.detail} />;
    }
  }

  function renderBookmarks() {
  if (view === "detail" && selectedBookmarkId) {
    const detailProblem = detailError ? classifyConnectionError(detailError) : null;
    const mutationProblem = mutationError ? classifyConnectionError(mutationError) : null;

    if (detailLoading) {
      return (
        <div className="p-4" data-testid="detail-loading">
          <button
            type="button"
            className="text-sm text-indigo-600 hover:text-indigo-800 mb-4"
            onClick={handleBackToList}
            data-testid="back-to-list"
          >
            &larr; Back to list
          </button>
          <p className="text-sm text-gray-500">Loading bookmark...</p>
        </div>
      );
    }

    if (detailError) {
      return (
        <div className="p-4" data-testid="detail-error">
          <button
            type="button"
            className="text-sm text-indigo-600 hover:text-indigo-800 mb-4"
            onClick={handleBackToList}
            data-testid="back-to-list"
          >
            &larr; Back to list
          </button>
          {detailProblem ? (
            <InstallPrompt problem={detailProblem} onRetry={handleRetryDetail} detail={detailError} />
          ) : (
            <p className="text-sm text-red-600">{detailError}</p>
          )}
        </div>
      );
    }

    if (selectedBookmark) {
      return (
        <div data-testid="detail-view">
          {mutationProblem ? (
            <div className="px-4 pt-3">
              <InstallPrompt problem={mutationProblem} onRetry={handleRetryDetail} detail={mutationError ?? undefined} />
            </div>
          ) : mutationError ? (
            <div className="px-4 pt-2">
              <p className="text-xs text-red-600" data-testid="mutation-error">{mutationError}</p>
            </div>
          ) : null}
          <BookmarkDetail
            bookmark={selectedBookmark}
            collections={collections}
            onUpdate={handleUpdate}
            onBack={handleBackToList}
            updating={updating}
          />
        </div>
      );
    }
  }

  return (
    <>
      {loading && bookmarks.length === 0 ? (
        <div className="flex-1 flex items-center justify-center" data-testid="loading-state">
          <p className="text-sm text-gray-500">Loading bookmarks...</p>
        </div>
      ) : connectionProblem && bookmarks.length === 0 ? (
        <div className="flex-1 overflow-y-auto p-4">
          <InstallPrompt problem={connectionProblem} onRetry={handleRetry} detail={error ?? undefined} />
        </div>
      ) : error && bookmarks.length === 0 ? (
        <div className="flex-1 flex items-center justify-center p-4" data-testid="error-state">
          <p className="text-sm text-red-600 text-center">{error}</p>
        </div>
      ) : (
        <BookmarkList
          bookmarks={bookmarks}
          activeFilter={activeFilter}
          onFilterChange={handleFilterChange}
          onSelect={handleSelect}
        />
      )}
    </>
  );
  }
}
