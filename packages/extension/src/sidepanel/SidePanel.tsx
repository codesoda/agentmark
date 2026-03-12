import { useCallback, useEffect, useRef, useState } from "react";
import {
  sendListBookmarksMessage,
  sendListCollectionsMessage,
  sendShowBookmarkMessage,
  sendUpdateBookmarkMessage,
} from "../shared/runtime";
import type { BookmarkSummary, BookmarkDetail as BookmarkDetailType, BookmarkChanges, BookmarkStateFilter } from "../shared/types";
import BookmarkList, { type FilterValue } from "./BookmarkList";
import BookmarkDetail from "./BookmarkDetail";

type View = "list" | "detail";

const DEFAULT_LIMIT = 50;

export default function SidePanel() {
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
        setError(result.error);
        setBookmarks([]);
      } else {
        setBookmarks(result.bookmarks);
      }
      setLoading(false);
    });
  }, []);

  // Load on mount
  useEffect(() => {
    loadBookmarks(activeFilter);
  }, [loadBookmarks, activeFilter]);

  // Refresh on window focus
  useEffect(() => {
    const handleFocus = () => {
      loadBookmarks(activeFilter);
    };
    window.addEventListener("focus", handleFocus);
    return () => window.removeEventListener("focus", handleFocus);
  }, [loadBookmarks, activeFilter]);

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
        setDetailError(showResult.error);
      } else if (showResult.bookmark) {
        setSelectedBookmark(showResult.bookmark);
        setCollections(collectionsList);
      }
      setDetailLoading(false);
    });
  }, []);

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

  const handleBackToList = useCallback(() => {
    setView("list");
    setSelectedBookmarkId(null);
    setSelectedBookmark(null);
    setDetailError(null);
    setMutationError(null);
  }, []);

  if (view === "detail" && selectedBookmarkId) {
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
          <p className="text-sm text-red-600">{detailError}</p>
        </div>
      );
    }

    if (selectedBookmark) {
      return (
        <div data-testid="detail-view">
          {mutationError && (
            <div className="px-4 pt-2">
              <p className="text-xs text-red-600" data-testid="mutation-error">{mutationError}</p>
            </div>
          )}
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
    <div className="flex flex-col h-screen">
      <div className="p-3 border-b border-gray-200">
        <h1 className="text-base font-semibold text-gray-900">AgentMark</h1>
      </div>
      {loading && bookmarks.length === 0 ? (
        <div className="flex-1 flex items-center justify-center" data-testid="loading-state">
          <p className="text-sm text-gray-500">Loading bookmarks...</p>
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
    </div>
  );
}
