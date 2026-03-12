import { useState, type FormEvent } from "react";
import TagInput from "./TagInput";
import CollectionSelect from "./CollectionSelect";

export interface SaveFormValues {
  tags: string[];
  collection?: string;
  note?: string;
  action?: string;
  selected_text?: string;
}

interface SaveFormProps {
  initialTags: string[];
  initialCollection?: string;
  initialSelectedText: string;
  collections: string[];
  onSubmit: (values: SaveFormValues) => void;
  onCancel: () => void;
  submitting: boolean;
  error?: string;
}

/**
 * Extended save form for adding tags, notes, collections, and action prompts.
 * All fields are optional — user can submit with any combination.
 */
export default function SaveForm({
  initialTags,
  initialCollection,
  initialSelectedText,
  collections,
  onSubmit,
  onCancel,
  submitting,
  error,
}: SaveFormProps) {
  const [tags, setTags] = useState<string[]>(initialTags);
  const [collection, setCollection] = useState<string | undefined>(initialCollection);
  const [note, setNote] = useState("");
  const [action, setAction] = useState("");
  const [selectedText, setSelectedText] = useState(initialSelectedText);

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    onSubmit({
      tags: tags.length > 0 ? tags : [],
      collection,
      note: note.trim() || undefined,
      action: action.trim() || undefined,
      selected_text: selectedText.trim() || undefined,
    });
  };

  return (
    <form onSubmit={handleSubmit} data-testid="save-form" className="space-y-3">
      <TagInput tags={tags} onChange={setTags} />

      <CollectionSelect
        collections={collections}
        value={collection}
        onChange={setCollection}
      />

      <div>
        <label htmlFor="note-input" className="block text-xs font-medium text-gray-700">
          Note
        </label>
        <textarea
          id="note-input"
          value={note}
          onChange={(e) => setNote(e.target.value)}
          placeholder="Why I saved this..."
          rows={2}
          className="mt-1 block w-full rounded border border-gray-300 px-2 py-1 text-xs"
          data-testid="note-input"
        />
      </div>

      <div>
        <label htmlFor="action-input" className="block text-xs font-medium text-gray-700">
          Action prompt
        </label>
        <textarea
          id="action-input"
          value={action}
          onChange={(e) => setAction(e.target.value)}
          placeholder="Agent instructions..."
          rows={2}
          className="mt-1 block w-full rounded border border-gray-300 px-2 py-1 text-xs"
          data-testid="action-input"
        />
      </div>

      {initialSelectedText && (
        <div>
          <label htmlFor="selected-text-input" className="block text-xs font-medium text-gray-700">
            Selected text
          </label>
          <textarea
            id="selected-text-input"
            value={selectedText}
            onChange={(e) => setSelectedText(e.target.value)}
            rows={3}
            className="mt-1 block w-full rounded border border-gray-300 px-2 py-1 text-xs"
            data-testid="selected-text-input"
          />
        </div>
      )}

      {error && (
        <p className="text-xs text-red-600" data-testid="form-error">
          {error}
        </p>
      )}

      <div className="flex justify-end gap-2">
        <button
          type="button"
          onClick={onCancel}
          disabled={submitting}
          className="rounded border border-gray-300 px-3 py-1 text-xs text-gray-700 hover:bg-gray-50"
          data-testid="form-cancel"
        >
          Cancel
        </button>
        <button
          type="submit"
          disabled={submitting}
          className="rounded bg-indigo-600 px-3 py-1 text-xs text-white hover:bg-indigo-700 disabled:opacity-50"
          data-testid="form-submit"
        >
          {submitting ? "Saving..." : "Save"}
        </button>
      </div>
    </form>
  );
}
