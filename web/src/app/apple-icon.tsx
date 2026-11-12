import { ImageResponse } from "next/og";

export const size = { width: 180, height: 180 };
export const contentType = "image/png";

// The listing mark (packages/extension/icons/icon.svg) on a full-bleed tile.
export default function AppleIcon() {
  return new ImageResponse(
    (
      <div style={{ width: "100%", height: "100%", display: "flex", background: "#4f46e5" }}>
        <svg viewBox="1.5 1.5 21 21" width="180" height="180">
          <rect x="1.5" y="1.5" width="21" height="21" fill="#4F46E5" />
          <path
            d="M9.057 5.795H14.943A1.67 1.67 0 0 1 16.614 7.466V17.695Q16.614 18.205 16.105 18.205H15.834Q15.58 18.205 15.401 18.026L12 14.625L8.599 18.026Q8.42 18.205 8.166 18.205H7.895Q7.386 18.205 7.386 17.695V7.466A1.67 1.67 0 0 1 9.057 5.795Z"
            fill="#fff"
          />
          <path d="M12 8.86V11.56M10.65 10.21H13.35" stroke="#4F46E5" strokeWidth="1.3" strokeLinecap="round" fill="none" />
        </svg>
      </div>
    ),
    size,
  );
}
