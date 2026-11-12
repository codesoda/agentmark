import { ImageResponse } from "next/og";

export const alt = "AgentMark: bookmarks your agent can read";
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

const BOOKMARK =
  "M9.057 5.795H14.943A1.67 1.67 0 0 1 16.614 7.466V17.695Q16.614 18.205 16.105 18.205H15.834Q15.58 18.205 15.401 18.026L12 14.625L8.599 18.026Q8.42 18.205 8.166 18.205H7.895Q7.386 18.205 7.386 17.695V7.466A1.67 1.67 0 0 1 9.057 5.795Z";

export default function OpenGraphImage() {
  return new ImageResponse(
    (
      <div
        style={{
          width: "100%",
          height: "100%",
          display: "flex",
          flexDirection: "column",
          justifyContent: "space-between",
          padding: "72px 80px",
          background: "#0c0c0e",
          color: "#ededf0",
          fontFamily: "sans-serif",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 20 }}>
          <svg viewBox="1.5 1.5 21 21" width="64" height="64">
            <rect x="1.5" y="1.5" width="21" height="21" rx="2.784" fill="#4F46E5" />
            <path d={BOOKMARK} fill="#fff" />
            <path d="M12 8.86V11.56M10.65 10.21H13.35" stroke="#4F46E5" strokeWidth="1.3" strokeLinecap="round" fill="none" />
          </svg>
          <div style={{ fontSize: 40, fontWeight: 600, letterSpacing: -1 }}>AgentMark</div>
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 28 }}>
          <div style={{ fontSize: 84, fontWeight: 600, letterSpacing: -3, lineHeight: 1.02, maxWidth: 980 }}>
            Bookmarks your agent can actually read.
          </div>
          <div style={{ fontSize: 30, color: "#a6a6b0", maxWidth: 940, lineHeight: 1.35 }}>
            Pages saved as plain files on your machine. Searchable by you, Claude Code and Codex.
          </div>
        </div>
        <div style={{ display: "flex", fontSize: 24, color: "#9ea4ff", fontFamily: "monospace" }}>
          github.com/codesoda/agentmark
        </div>
      </div>
    ),
    size,
  );
}
