/**
 * The AgentMark mark for extension pages, in two of the toolbar icon's looks:
 * `idle` (grey with a plus: not set up, not saved) and `saved` (brand blue:
 * ready). Same shapes as `icons/toolbar-*.svg` and the in-page badge; the
 * path data is duplicated from `src/content/save-button.ts` (a test keeps
 * them equal) because popup code must not share modules with content scripts.
 */

export const MARK_BADGE_PATH =
  "M4.284 1.5H19.716A2.784 2.784 0 0 1 22.5 4.284V19.716A2.784 2.784 0 0 1 19.716 22.5H4.284A2.784 2.784 0 0 1 1.5 19.716V4.284A2.784 2.784 0 0 1 4.284 1.5Z";
export const MARK_BOOKMARK_PATH =
  "M9.057 5.795H14.943A1.67 1.67 0 0 1 16.614 7.466V17.695Q16.614 18.205 16.105 18.205H15.834Q15.58 18.205 15.401 18.026L12 14.625L8.599 18.026Q8.42 18.205 8.166 18.205H7.895Q7.386 18.205 7.386 17.695V7.466A1.67 1.67 0 0 1 9.057 5.795Z";
export const MARK_PLUS_PATH = "M12 8.86V11.56M10.65 10.21H13.35";

/** Grey of the idle toolbar icon; brand blue of the saved one. */
const LOOK_COLOR = { idle: "#6b7280", saved: "#4f46e5" } as const;

export type MarkLook = keyof typeof LOOK_COLOR;

export default function AgentMarkMark({ look, size = 20 }: { look: MarkLook; size?: number }) {
  const color = LOOK_COLOR[look];
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="1.5 1.5 21 21"
      width={size}
      height={size}
      aria-hidden="true"
      data-look={look}
      className="shrink-0"
    >
      <path d={MARK_BADGE_PATH} fill={color} />
      <path d={MARK_BOOKMARK_PATH} fill="#fff" />
      {look === "idle" && (
        <path
          d={MARK_PLUS_PATH}
          stroke={color}
          fill="none"
          strokeWidth={1.3}
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      )}
    </svg>
  );
}
