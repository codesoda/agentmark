import type { BookmarkSummary } from "../shared/types";
import { formatDate } from "./format";

interface BookmarkCardProps {
  bookmark: BookmarkSummary;
  onSelect: (id: string) => void;
}

const STATE_LABELS: Record<string, { label: string; className: string }> = {
  inbox: { label: "Inbox", className: "bg-blue-100 text-blue-700" },
  processed: { label: "Processed", className: "bg-green-100 text-green-700" },
  archived: { label: "Archived", className: "bg-gray-100 text-gray-600" },
};

export default function BookmarkCard({ bookmark, onSelect }: BookmarkCardProps) {
  const stateInfo = STATE_LABELS[bookmark.state] ?? STATE_LABELS.inbox;
  const title = bookmark.title || bookmark.url;

  return (
    <button
      type="button"
      className="w-full text-left p-3 border border-gray-200 rounded-lg hover:bg-gray-50 transition-colors cursor-pointer"
      onClick={() => onSelect(bookmark.id)}
      data-testid={`bookmark-card-${bookmark.id}`}
    >
      <div className="flex items-start justify-between gap-2">
        <h3 className="text-sm font-medium text-gray-900 line-clamp-2 flex-1">
          {title}
        </h3>
        <span
          className={`inline-flex items-center px-1.5 py-0.5 text-xs font-medium rounded shrink-0 ${stateInfo.className}`}
          data-testid="state-badge"
        >
          {stateInfo.label}
        </span>
      </div>
      <p className="mt-1 text-xs text-gray-500" data-testid="saved-date">
        {formatDate(bookmark.saved_at)}
      </p>
      {(bookmark.user_tags.length > 0 || bookmark.suggested_tags.length > 0) && (
        <div className="mt-1.5 flex flex-wrap gap-1">
          {bookmark.user_tags.map((tag) => (
            <span
              key={`user-${tag}`}
              className="inline-flex items-center px-1.5 py-0.5 text-xs bg-indigo-100 text-indigo-700 rounded"
              data-testid="user-tag"
            >
              {tag}
            </span>
          ))}
          {bookmark.suggested_tags.map((tag) => (
            <span
              key={`suggested-${tag}`}
              className="inline-flex items-center px-1.5 py-0.5 text-xs bg-amber-50 text-amber-700 rounded border border-amber-200"
              data-testid="suggested-tag"
            >
              {tag}
            </span>
          ))}
        </div>
      )}
    </button>
  );
}
