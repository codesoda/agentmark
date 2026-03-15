import { useState, useRef, useEffect, type KeyboardEvent } from "react";

interface EditableFieldProps {
  value: string;
  onSave: (value: string) => Promise<void>;
  placeholder?: string;
  label: string;
  multiline?: boolean;
}

export default function EditableField({
  value,
  onSave,
  placeholder,
  label,
  multiline = false,
}: EditableFieldProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement | HTMLTextAreaElement>(null);

  useEffect(() => {
    if (!editing) {
      setDraft(value);
    }
  }, [value, editing]);

  useEffect(() => {
    if (editing && inputRef.current) {
      inputRef.current.focus();
    }
  }, [editing]);

  const save = async () => {
    if (saving) return;
    const trimmed = draft.trim();
    // No-op if unchanged
    if (trimmed === (value || "")) {
      setEditing(false);
      setError(null);
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await onSave(trimmed);
      setEditing(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !multiline) {
      e.preventDefault();
      save();
    }
    if (e.key === "Escape") {
      setDraft(value);
      setEditing(false);
      setError(null);
    }
  };

  if (editing) {
    const className = "w-full rounded border border-gray-300 px-2 py-1 text-xs";
    return (
      <div>
        <label className="block text-xs font-medium text-gray-500 mb-0.5">{label}</label>
        {multiline ? (
          <textarea
            ref={inputRef as React.RefObject<HTMLTextAreaElement>}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={save}
            onKeyDown={handleKeyDown}
            placeholder={placeholder}
            rows={3}
            className={className}
            disabled={saving}
            data-testid="editable-field-input"
          />
        ) : (
          <input
            ref={inputRef as React.RefObject<HTMLInputElement>}
            type="text"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={save}
            onKeyDown={handleKeyDown}
            placeholder={placeholder}
            className={className}
            disabled={saving}
            data-testid="editable-field-input"
          />
        )}
        {saving && <p className="text-xs text-gray-400 mt-0.5">Saving...</p>}
        {error && <p className="text-xs text-red-600 mt-0.5" data-testid="editable-field-error">{error}</p>}
      </div>
    );
  }

  return (
    <div>
      <label className="block text-xs font-medium text-gray-500 mb-0.5">{label}</label>
      <button
        type="button"
        onClick={() => setEditing(true)}
        className="w-full text-left text-xs text-gray-900 hover:bg-gray-50 rounded px-2 py-1 min-h-[28px]"
        data-testid="editable-field-display"
      >
        {value || <span className="text-gray-400 italic">{placeholder || "Click to edit"}</span>}
      </button>
    </div>
  );
}
