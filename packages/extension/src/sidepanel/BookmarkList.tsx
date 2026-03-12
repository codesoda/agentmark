import type { BookmarkSummary, BookmarkStateFilter } from "../shared/types";
import BookmarkCard from "./BookmarkCard";

export type FilterValue = BookmarkStateFilter | "all";

interface BookmarkListProps {
  bookmarks: BookmarkSummary[];
  activeFilter: FilterValue;
  onFilterChange: (filter: FilterValue) => void;
  onSelect: (id: string) => void;
}

const FILTERS: { value: FilterValue; label: string }[] = [
  { value: "all", label: "All" },
  { value: "inbox", label: "Inbox" },
  { value: "processed", label: "Processed" },
  { value: "archived", label: "Archived" },
];

export default function BookmarkList({
  bookmarks,
  activeFilter,
  onFilterChange,
  onSelect,
}: BookmarkListProps) {
  return (
    <div className="flex flex-col h-full">
      <div className="flex border-b border-gray-200" data-testid="filter-tabs">
        {FILTERS.map(({ value, label }) => (
          <button
            key={value}
            type="button"
            className={`flex-1 px-2 py-2 text-xs font-medium transition-colors ${
              activeFilter === value
                ? "text-indigo-600 border-b-2 border-indigo-600"
                : "text-gray-500 hover:text-gray-700"
            }`}
            onClick={() => onFilterChange(value)}
            data-testid={`filter-${value}`}
          >
            {label}
          </button>
        ))}
      </div>
      <div className="flex-1 overflow-y-auto p-2 space-y-2">
        {bookmarks.length === 0 ? (
          <p className="text-center text-sm text-gray-500 mt-8" data-testid="empty-state">
            No bookmarks found
          </p>
        ) : (
          bookmarks.map((bookmark) => (
            <BookmarkCard
              key={bookmark.id}
              bookmark={bookmark}
              onSelect={onSelect}
            />
          ))
        )}
      </div>
    </div>
  );
}
