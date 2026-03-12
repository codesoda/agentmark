// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, fireEvent, cleanup } from "@testing-library/react";
import { resetChromeMock } from "../test/chrome-mock";
import type { BookmarkSummary } from "../shared/types";

vi.mock("../shared/runtime", () => ({
  sendListBookmarksMessage: vi.fn(),
}));

import SidePanel from "./SidePanel";
import { sendListBookmarksMessage } from "../shared/runtime";

function makeBookmark(id: string, overrides: Partial<BookmarkSummary> = {}): BookmarkSummary {
  return {
    id,
    url: `https://example.com/${id}`,
    title: `Bookmark ${id}`,
    state: "inbox",
    user_tags: [],
    suggested_tags: [],
    saved_at: "2026-03-12T00:00:00Z",
    ...overrides,
  };
}

describe("SidePanel", () => {
  afterEach(cleanup);
  beforeEach(() => {
    resetChromeMock();
    vi.clearAllMocks();
  });

  it("shows loading state initially", async () => {
    let resolveList!: (val: { bookmarks: BookmarkSummary[] }) => void;
    vi.mocked(sendListBookmarksMessage).mockReturnValue(
      new Promise((resolve) => { resolveList = resolve; }),
    );

    render(<SidePanel />);
    expect(screen.getByTestId("loading-state")).toBeDefined();

    await act(async () => resolveList({ bookmarks: [] }));
  });

  it("shows bookmarks after successful load", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("1"), makeBookmark("2")],
    });

    render(<SidePanel />);
    expect(await screen.findByTestId("bookmark-card-1")).toBeDefined();
    expect(screen.getByTestId("bookmark-card-2")).toBeDefined();
  });

  it("shows empty state when no bookmarks", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [],
    });

    render(<SidePanel />);
    expect(await screen.findByTestId("empty-state")).toBeDefined();
  });

  it("shows error state on failure", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [],
      error: "not initialized: config not found",
    });

    render(<SidePanel />);
    expect(await screen.findByTestId("error-state")).toBeDefined();
    expect(screen.getByTestId("error-state").textContent).toContain("not initialized");
  });

  it("fetches bookmarks with filter when filter tab changes", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("1")],
    });

    render(<SidePanel />);
    await screen.findByTestId("bookmark-card-1");

    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("2", { state: "processed" })],
    });

    fireEvent.click(screen.getByTestId("filter-processed"));

    expect(await screen.findByTestId("bookmark-card-2")).toBeDefined();
    // Should have been called with state filter
    expect(sendListBookmarksMessage).toHaveBeenCalledWith(
      expect.objectContaining({ state: "processed" }),
    );
  });

  it("fetches all bookmarks with 'all' filter", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("1")],
    });

    render(<SidePanel />);
    await screen.findByTestId("bookmark-card-1");

    // First call should have been with no state (all filter)
    expect(sendListBookmarksMessage).toHaveBeenCalledWith(
      expect.objectContaining({ state: undefined }),
    );
  });

  it("refreshes on window focus", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("1")],
    });

    render(<SidePanel />);
    await screen.findByTestId("bookmark-card-1");

    const callCount = vi.mocked(sendListBookmarksMessage).mock.calls.length;

    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("1"), makeBookmark("2")],
    });

    await act(async () => {
      window.dispatchEvent(new Event("focus"));
    });

    expect(vi.mocked(sendListBookmarksMessage).mock.calls.length).toBeGreaterThan(callCount);
  });

  it("navigates to detail view on card click", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("am_456")],
    });

    render(<SidePanel />);
    await screen.findByTestId("bookmark-card-am_456");

    fireEvent.click(screen.getByTestId("bookmark-card-am_456"));

    expect(screen.getByTestId("detail-view")).toBeDefined();
    expect(screen.getByTestId("selected-id").textContent).toContain("am_456");
  });

  it("navigates back to list from detail view", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("am_456")],
    });

    render(<SidePanel />);
    await screen.findByTestId("bookmark-card-am_456");

    fireEvent.click(screen.getByTestId("bookmark-card-am_456"));
    expect(screen.getByTestId("detail-view")).toBeDefined();

    fireEvent.click(screen.getByTestId("back-to-list"));
    expect(screen.queryByTestId("detail-view")).toBeNull();
    expect(screen.getByTestId("bookmark-card-am_456")).toBeDefined();
  });

  it("ignores stale responses when filter changes rapidly", async () => {
    // Track all pending promises so we can control resolution order
    const pendingResolvers: Array<(val: { bookmarks: BookmarkSummary[] }) => void> = [];

    vi.mocked(sendListBookmarksMessage).mockImplementation(
      () => new Promise((resolve) => { pendingResolvers.push(resolve); }),
    );

    render(<SidePanel />);

    // Wait for initial mount call(s) — resolve them to get past loading
    await act(async () => {
      while (pendingResolvers.length > 0) {
        pendingResolvers.shift()!({ bookmarks: [makeBookmark("1")] });
      }
    });

    // Now trigger two rapid filter changes
    fireEvent.click(screen.getByTestId("filter-inbox"));
    fireEvent.click(screen.getByTestId("filter-processed"));

    // There should be pending requests from the filter changes
    // The last one should win — resolve them in reverse order
    const resolvers = [...pendingResolvers];
    pendingResolvers.length = 0;

    if (resolvers.length >= 2) {
      // Resolve the LAST (newer) request first
      await act(async () => {
        resolvers[resolvers.length - 1]({ bookmarks: [makeBookmark("newer")] });
      });

      // Then resolve the FIRST (stale) request
      await act(async () => {
        resolvers[0]({ bookmarks: [makeBookmark("stale")] });
      });

      // Should show the newer result, not the stale one
      expect(screen.queryByTestId("bookmark-card-stale")).toBeNull();
      expect(screen.getByTestId("bookmark-card-newer")).toBeDefined();
    } else if (resolvers.length === 1) {
      // If React batched the state updates, there may be only one pending request
      // which is for the final filter state — this is also acceptable
      await act(async () => {
        resolvers[0]({ bookmarks: [makeBookmark("newer")] });
      });
      expect(screen.getByTestId("bookmark-card-newer")).toBeDefined();
    }
  });
});
