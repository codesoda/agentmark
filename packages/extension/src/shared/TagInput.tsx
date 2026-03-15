import { useState, useCallback, type KeyboardEvent, type ChangeEvent } from "react";

interface TagInputProps {
  tags: string[];
  onChange: (tags: string[]) => void;
}

/**
 * Comma-separated tag input with removable pills.
 * Commits tags on comma, Enter, and blur.
 * Deduplicates and trims whitespace.
 */
export default function TagInput({ tags, onChange }: TagInputProps) {
  const [input, setInput] = useState("");

  const commitTags = useCallback(
    (text: string) => {
      const newTags = text
        .split(",")
        .map((t) => t.trim().toLowerCase())
        .filter(Boolean);
      if (newTags.length === 0) return;
      const seen = new Set(tags);
      const merged = [...tags];
      for (const tag of newTags) {
        if (!seen.has(tag)) {
          seen.add(tag);
          merged.push(tag);
        }
      }
      onChange(merged);
      setInput("");
    },
    [tags, onChange],
  );

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      commitTags(input);
    }
    if (e.key === "Backspace" && input === "" && tags.length > 0) {
      onChange(tags.slice(0, -1));
    }
  };

  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    // Auto-commit on trailing comma
    if (value.includes(",")) {
      commitTags(value);
    } else {
      setInput(value);
    }
  };

  const handleBlur = () => {
    if (input.trim()) {
      commitTags(input);
    }
  };

  const removeTag = (index: number) => {
    onChange(tags.filter((_, i) => i !== index));
  };

  return (
    <div>
      <label htmlFor="tag-input" className="block text-xs font-medium text-gray-700">
        Tags
      </label>
      <div className="mt-1 flex flex-wrap items-center gap-1 rounded border border-gray-300 px-2 py-1">
        {tags.map((tag, i) => (
          <span
            key={`${tag}-${i}`}
            className="inline-flex items-center gap-0.5 rounded bg-indigo-100 px-1.5 py-0.5 text-xs text-indigo-700"
            data-testid="tag-pill"
          >
            {tag}
            <button
              type="button"
              onClick={() => removeTag(i)}
              className="ml-0.5 text-indigo-400 hover:text-indigo-600"
              aria-label={`Remove tag ${tag}`}
            >
              ×
            </button>
          </span>
        ))}
        <input
          id="tag-input"
          type="text"
          value={input}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          onBlur={handleBlur}
          placeholder={tags.length === 0 ? "Add tags..." : ""}
          className="min-w-[60px] flex-1 border-none p-0.5 text-xs outline-none"
          data-testid="tag-input"
        />
      </div>
    </div>
  );
}
