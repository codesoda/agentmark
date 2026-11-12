/*
 * The AgentMark mark, drawn from the exact SVG sources in
 * packages/extension/icons/ (copied to web/public/mark/). Do not redraw or
 * reshape these paths: the bookmark shape is a licensed Noun Project icon
 * (favorite #8112368). Colours are the brand colours, not theme tokens, so
 * the states read the same in both themes. Blue only ever means saved.
 */

export type MarkLook = "idle" | "saving" | "saved-fresh" | "saved" | "error" | "brand";

const TILE =
  "M4.284 1.5H19.716A2.784 2.784 0 0 1 22.5 4.284V19.716A2.784 2.784 0 0 1 19.716 22.5H4.284A2.784 2.784 0 0 1 1.5 19.716V4.284A2.784 2.784 0 0 1 4.284 1.5Z";
const BOOKMARK =
  "M9.057 5.795H14.943A1.67 1.67 0 0 1 16.614 7.466V17.695Q16.614 18.205 16.105 18.205H15.834Q15.58 18.205 15.401 18.026L12 14.625L8.599 18.026Q8.42 18.205 8.166 18.205H7.895Q7.386 18.205 7.386 17.695V7.466A1.67 1.67 0 0 1 9.057 5.795Z";
const PLUS = "M12 8.86V11.56M10.65 10.21H13.35";
const TICK = "M10.4 10.31L11.55 11.41L13.7 9.06";

export const MARK_COLORS = {
  brand: "#4f46e5",
  idle: "#6b7280",
  saving: "#818cf8",
  error: "#b91c1c",
} as const;

const LABELS: Record<MarkLook, string> = {
  idle: "Not saved",
  saving: "Saving",
  "saved-fresh": "Saved",
  saved: "Saved",
  error: "Save failed",
  brand: "AgentMark",
};

export function AgentMarkMark({
  look = "brand",
  size = 20,
  className,
  label,
}: {
  /** Which state to draw. `brand` is the listing icon (icon.svg). */
  look?: MarkLook;
  /** Rendered width and height in CSS pixels. */
  size?: number;
  className?: string;
  /** Accessible name. Omit to hide the mark from assistive tech. */
  label?: string | true;
}) {
  const a11y =
    label === undefined
      ? ({ "aria-hidden": true } as const)
      : ({ role: "img", "aria-label": label === true ? LABELS[look] : label } as const);

  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="1.5 1.5 21 21"
      width={size}
      height={size}
      className={className}
      {...a11y}
    >
      {look === "brand" && (
        <>
          <rect x="1.5" y="1.5" width="21" height="21" rx="2.784" fill={MARK_COLORS.brand} />
          <path d={BOOKMARK} fill="#fff" />
          <path d={PLUS} stroke={MARK_COLORS.brand} strokeWidth="1.3" strokeLinecap="round" fill="none" />
        </>
      )}
      {look === "idle" && (
        <>
          <path d={TILE} fill={MARK_COLORS.idle} />
          <path d={BOOKMARK} fill="#fff" />
          <path
            d={PLUS}
            stroke={MARK_COLORS.idle}
            fill="none"
            strokeWidth="1.3"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </>
      )}
      {look === "saving" && (
        <>
          <rect
            x="2.3"
            y="2.3"
            width="19.4"
            height="19.4"
            rx="1.984"
            fill="none"
            stroke={MARK_COLORS.saving}
            strokeWidth="1.6"
          />
          <path d={BOOKMARK} fill={MARK_COLORS.saving} />
        </>
      )}
      {look === "saved-fresh" && (
        <>
          <path d={TILE} fill={MARK_COLORS.brand} />
          <path d={BOOKMARK} fill="#fff" />
          <path
            d={TICK}
            stroke={MARK_COLORS.brand}
            fill="none"
            strokeWidth="1.3"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </>
      )}
      {look === "saved" && (
        <>
          <path d={TILE} fill={MARK_COLORS.brand} />
          <path d={BOOKMARK} fill="#fff" />
        </>
      )}
      {look === "error" && (
        <>
          <path d={TILE} fill={MARK_COLORS.error} />
          <path d={BOOKMARK} fill="#fff" />
          <path
            d="M12 8.61V10.56"
            stroke={MARK_COLORS.error}
            fill="none"
            strokeWidth="1.3"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <circle cx="12" cy="11.96" r="0.72" fill={MARK_COLORS.error} />
        </>
      )}
    </svg>
  );
}
