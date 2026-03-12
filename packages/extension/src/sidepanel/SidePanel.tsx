import { useCallback, useEffect, useRef, useState } from "react";
import { sendListBookmarksMessage } from "../shared/runtime";
import type { BookmarkSummary, BookmarkStateFilter } from "../shared/types";
import BookmarkList, { type FilterValue } from "./BookmarkList";

type View = "list" | "detail";

const DEFAULT_LIMIT = 50;

export default function SidePanel() {
  const [view, setView] = useState<View>("list");
  const [selectedBookmarkId, setSelectedBookmarkId] = useState<string | null>(null);
  const [bookmarks, setBookmarks] = useState<BookmarkSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeFilter, setActiveFilter] = useState<FilterValue>("all");

  // Stale-response guard: only apply results from the latest request
  const requestSeqRef = useRef(0);

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
  }, []);

  const handleBackToList = useCallback(() => {
    setView("list");
    setSelectedBookmarkId(null);
  }, []);

  if (view === "detail" && selectedBookmarkId) {
    return (
      <div className="p-4" data-testid="detail-view">
        <button
          type="button"
          className="text-sm text-indigo-600 hover:text-indigo-800 mb-4"
          onClick={handleBackToList}
          data-testid="back-to-list"
        >
          &larr; Back to list
        </button>
        <p className="text-sm text-gray-600">
          Bookmark detail view coming in Spec 23.
        </p>
        <p className="text-xs text-gray-400 mt-1" data-testid="selected-id">
          ID: {selectedBookmarkId}
        </p>
      </div>
    );
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
