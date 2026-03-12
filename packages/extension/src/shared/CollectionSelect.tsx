import { useState, type ChangeEvent } from "react";

interface CollectionSelectProps {
  collections: string[];
  value?: string;
  onChange: (value: string | undefined) => void;
}

const NEW_COLLECTION_SENTINEL = "__new__";

/**
 * Dropdown of existing collections plus a "New collection..." option
 * that reveals a text input for freetext entry.
 */
export default function CollectionSelect({
  collections,
  value,
  onChange,
}: CollectionSelectProps) {
  const [isCustom, setIsCustom] = useState(false);
  const [customText, setCustomText] = useState("");

  const handleSelectChange = (e: ChangeEvent<HTMLSelectElement>) => {
    const selected = e.target.value;
    if (selected === NEW_COLLECTION_SENTINEL) {
      setIsCustom(true);
      setCustomText("");
      onChange(undefined);
    } else if (selected === "") {
      setIsCustom(false);
      onChange(undefined);
    } else {
      setIsCustom(false);
      onChange(selected);
    }
  };

  const handleCustomChange = (e: ChangeEvent<HTMLInputElement>) => {
    const text = e.target.value;
    setCustomText(text);
    const trimmed = text.trim();
    onChange(trimmed || undefined);
  };

  return (
    <div>
      <label htmlFor="collection-select" className="block text-xs font-medium text-gray-700">
        Collection
      </label>
      <select
        id="collection-select"
        value={isCustom ? NEW_COLLECTION_SENTINEL : value ?? ""}
        onChange={handleSelectChange}
        className="mt-1 block w-full rounded border border-gray-300 px-2 py-1 text-xs"
        data-testid="collection-select"
      >
        <option value="">None</option>
        {collections.map((c) => (
          <option key={c} value={c}>
            {c}
          </option>
        ))}
        <option value={NEW_COLLECTION_SENTINEL}>New collection...</option>
      </select>
      {isCustom && (
        <input
          type="text"
          value={customText}
          onChange={handleCustomChange}
          placeholder="Collection name"
          className="mt-1 block w-full rounded border border-gray-300 px-2 py-1 text-xs"
          data-testid="collection-custom-input"
          autoFocus
        />
      )}
    </div>
  );
}
