import type { BookmarkDetail as BookmarkDetailType, BookmarkChanges, BookmarkStateFilter } from "../shared/types";
import TagInput from "../shared/TagInput";
import CollectionSelect from "../shared/CollectionSelect";
import TagManager from "./TagManager";
import EditableField from "./EditableField";
import { formatDate } from "./format";

interface BookmarkDetailProps {
  bookmark: BookmarkDetailType;
  collections: string[];
  onUpdate: (changes: BookmarkChanges) => Promise<void>;
  onBack: () => void;
  updating: boolean;
}

const STATE_LABELS: Record<string, string> = {
  inbox: "Inbox",
  processed: "Processed",
  archived: "Archived",
};

const STATE_TRANSITIONS: Record<string, { next: BookmarkStateFilter; label: string } | null> = {
  inbox: { next: "processed", label: "Mark Processed" },
  processed: { next: "archived", label: "Archive" },
  archived: null,
};

const CAPTURE_SOURCE_LABELS: Record<string, string> = {
  cli: "CLI",
  extension: "Extension",
  context_menu: "Context Menu",
};

export default function BookmarkDetail({
  bookmark,
  collections,
  onUpdate,
  onBack,
  updating,
}: BookmarkDetailProps) {
  const title = bookmark.title || bookmark.url;
  const transition = STATE_TRANSITIONS[bookmark.state];

  // Fire-and-forget callers use .catch(() => {}) because handleUpdate throws on
  // error for EditableField's try/catch flow, and the error is already surfaced
  // via mutationError state in SidePanel.

  const handleAcceptTag = (tag: string) => {
    const newUserTags = bookmark.user_tags.includes(tag)
      ? bookmark.user_tags
      : [...bookmark.user_tags, tag];
    const newSuggested = bookmark.suggested_tags.filter((t) => t !== tag);
    onUpdate({ user_tags: newUserTags, suggested_tags: newSuggested }).catch(() => {});
  };

  const handleRejectTag = (tag: string) => {
    const newSuggested = bookmark.suggested_tags.filter((t) => t !== tag);
    onUpdate({ suggested_tags: newSuggested }).catch(() => {});
  };

  const handleUserTagsChange = (tags: string[]) => {
    onUpdate({ user_tags: tags }).catch(() => {});
  };

  const handleNoteChange = async (note: string) => {
    await onUpdate({ note: note || null });
  };

  const handleCollectionChange = (value: string | undefined) => {
    onUpdate({ collections: value ? [value] : [] }).catch(() => {});
  };

  const handleStateChange = () => {
    if (transition) {
      onUpdate({ state: transition.next }).catch(() => {});
    }
  };

  const currentCollection = bookmark.collections.length > 0 ? bookmark.collections[0] : undefined;

  return (
    <div className="p-4 space-y-4" data-testid="bookmark-detail">
      <button
        type="button"
        className="text-sm text-indigo-600 hover:text-indigo-800"
        onClick={onBack}
        data-testid="back-to-list"
      >
        &larr; Back to list
      </button>

      <div>
        <a
          href={bookmark.url}
          target="_blank"
          rel="noopener noreferrer"
          className="text-sm font-medium text-indigo-600 hover:text-indigo-800 hover:underline line-clamp-2"
          data-testid="detail-title"
        >
          {title}
        </a>
      </div>

      {bookmark.summary && (
        <div>
          <label className="block text-xs font-medium text-gray-500 mb-0.5">Summary</label>
          <p className="text-xs text-gray-700" data-testid="detail-summary">{bookmark.summary}</p>
        </div>
      )}

      <div>
        <label className="block text-xs font-medium text-gray-500 mb-1">Tags</label>
        <TagInput tags={bookmark.user_tags} onChange={handleUserTagsChange} />
      </div>

      <TagManager
        suggestedTags={bookmark.suggested_tags}
        userTags={bookmark.user_tags}
        onAccept={handleAcceptTag}
        onReject={handleRejectTag}
        disabled={updating}
      />

      <EditableField
        value={bookmark.note || ""}
        onSave={handleNoteChange}
        label="Note"
        placeholder="Add a note..."
        multiline
      />

      <div>
        <label className="block text-xs font-medium text-gray-500 mb-0.5">Collection</label>
        <CollectionSelect
          collections={collections}
          value={currentCollection}
          onChange={handleCollectionChange}
        />
      </div>

      <div className="flex items-center gap-4 text-xs text-gray-500">
        <span data-testid="detail-date">{formatDate(bookmark.saved_at)}</span>
        <span data-testid="detail-source">
          {CAPTURE_SOURCE_LABELS[bookmark.capture_source] || bookmark.capture_source}
        </span>
        <span data-testid="detail-state">{STATE_LABELS[bookmark.state] || bookmark.state}</span>
      </div>

      {transition && (
        <button
          type="button"
          onClick={handleStateChange}
          disabled={updating}
          className="w-full rounded bg-indigo-600 px-3 py-1.5 text-xs text-white hover:bg-indigo-700 disabled:opacity-50"
          data-testid="state-transition-btn"
        >
          {transition.label}
        </button>
      )}
    </div>
  );
}
