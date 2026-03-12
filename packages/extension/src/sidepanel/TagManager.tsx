interface TagManagerProps {
  suggestedTags: string[];
  userTags: string[];
  onAccept: (tag: string) => void;
  onReject: (tag: string) => void;
  disabled?: boolean;
}

export default function TagManager({
  suggestedTags,
  userTags,
  onAccept,
  onReject,
  disabled = false,
}: TagManagerProps) {
  if (suggestedTags.length === 0) {
    return null;
  }

  // Filter out tags already in user_tags to avoid duplicate accept
  const actionable = suggestedTags.filter(
    (tag) => !userTags.includes(tag),
  );

  if (actionable.length === 0) {
    return null;
  }

  return (
    <div>
      <label className="block text-xs font-medium text-gray-500 mb-1">
        Suggested tags
      </label>
      <div className="flex flex-wrap gap-1">
        {actionable.map((tag) => (
          <span
            key={tag}
            className="inline-flex items-center gap-0.5 rounded bg-amber-50 border border-amber-200 px-1.5 py-0.5 text-xs text-amber-700"
            data-testid="suggested-tag"
          >
            {tag}
            <button
              type="button"
              onClick={() => onAccept(tag)}
              disabled={disabled}
              className="ml-0.5 text-green-600 hover:text-green-800 disabled:opacity-50"
              aria-label={`Accept tag ${tag}`}
              data-testid={`accept-tag-${tag}`}
            >
              ✓
            </button>
            <button
              type="button"
              onClick={() => onReject(tag)}
              disabled={disabled}
              className="ml-0.5 text-red-500 hover:text-red-700 disabled:opacity-50"
              aria-label={`Reject tag ${tag}`}
              data-testid={`reject-tag-${tag}`}
            >
              ×
            </button>
          </span>
        ))}
      </div>
    </div>
  );
}
