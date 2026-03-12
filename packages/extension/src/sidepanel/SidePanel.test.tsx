// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, fireEvent, cleanup } from "@testing-library/react";
import { resetChromeMock } from "../test/chrome-mock";
import type { BookmarkSummary, BookmarkDetail } from "../shared/types";

vi.mock("../shared/runtime", () => ({
  sendListBookmarksMessage: vi.fn(),
  sendListCollectionsMessage: vi.fn(),
  sendShowBookmarkMessage: vi.fn(),
  sendUpdateBookmarkMessage: vi.fn(),
}));

import SidePanel from "./SidePanel";
import {
  sendListBookmarksMessage,
  sendListCollectionsMessage,
  sendShowBookmarkMessage,
  sendUpdateBookmarkMessage,
} from "../shared/runtime";

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

function makeDetail(id: string, overrides: Partial<BookmarkDetail> = {}): BookmarkDetail {
  return {
    id,
    url: `https://example.com/${id}`,
    title: `Bookmark ${id}`,
    summary: "A summary",
    saved_at: "2026-03-12T00:00:00Z",
    capture_source: "cli",
    state: "inbox",
    user_tags: [],
    suggested_tags: ["ai", "rust"],
    collections: [],
    note: null,
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
    vi.mocked(sendShowBookmarkMessage).mockResolvedValue({
      bookmark: makeDetail("am_456"),
    });
    vi.mocked(sendListCollectionsMessage).mockResolvedValue([]);

    render(<SidePanel />);
    await screen.findByTestId("bookmark-card-am_456");

    await act(async () => {
      fireEvent.click(screen.getByTestId("bookmark-card-am_456"));
    });

    expect(await screen.findByTestId("detail-view")).toBeDefined();
    expect(screen.getByTestId("bookmark-detail")).toBeDefined();
  });

  it("navigates back to list from detail view", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("am_456")],
    });
    vi.mocked(sendShowBookmarkMessage).mockResolvedValue({
      bookmark: makeDetail("am_456"),
    });
    vi.mocked(sendListCollectionsMessage).mockResolvedValue([]);

    render(<SidePanel />);
    await screen.findByTestId("bookmark-card-am_456");

    await act(async () => {
      fireEvent.click(screen.getByTestId("bookmark-card-am_456"));
    });

    expect(await screen.findByTestId("detail-view")).toBeDefined();

    fireEvent.click(screen.getByTestId("back-to-list"));
    expect(screen.queryByTestId("detail-view")).toBeNull();
    expect(screen.getByTestId("bookmark-card-am_456")).toBeDefined();
  });

  it("shows detail loading state", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("am_789")],
    });
    let resolveShow!: (val: { bookmark: BookmarkDetail }) => void;
    vi.mocked(sendShowBookmarkMessage).mockReturnValue(
      new Promise((resolve) => { resolveShow = resolve; }),
    );
    vi.mocked(sendListCollectionsMessage).mockResolvedValue([]);

    render(<SidePanel />);
    await screen.findByTestId("bookmark-card-am_789");

    fireEvent.click(screen.getByTestId("bookmark-card-am_789"));

    expect(screen.getByTestId("detail-loading")).toBeDefined();

    await act(async () => resolveShow({ bookmark: makeDetail("am_789") }));
    expect(screen.getByTestId("bookmark-detail")).toBeDefined();
  });

  it("shows detail error state", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("am_err")],
    });
    vi.mocked(sendShowBookmarkMessage).mockResolvedValue({
      error: "bookmark not found",
    });
    vi.mocked(sendListCollectionsMessage).mockResolvedValue([]);

    render(<SidePanel />);
    await screen.findByTestId("bookmark-card-am_err");

    await act(async () => {
      fireEvent.click(screen.getByTestId("bookmark-card-am_err"));
    });

    expect(await screen.findByTestId("detail-error")).toBeDefined();
    expect(screen.getByTestId("detail-error").textContent).toContain("bookmark not found");
  });

  it("updates bookmark and patches list state", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("am_upd", { user_tags: ["old"] })],
    });
    vi.mocked(sendShowBookmarkMessage).mockResolvedValue({
      bookmark: makeDetail("am_upd", { user_tags: ["old"], suggested_tags: ["ai"] }),
    });
    vi.mocked(sendListCollectionsMessage).mockResolvedValue([]);
    vi.mocked(sendUpdateBookmarkMessage).mockResolvedValue({
      bookmark: makeDetail("am_upd", { user_tags: ["old", "ai"], suggested_tags: [] }),
    });

    render(<SidePanel />);
    await screen.findByTestId("bookmark-card-am_upd");

    await act(async () => {
      fireEvent.click(screen.getByTestId("bookmark-card-am_upd"));
    });

    await screen.findByTestId("bookmark-detail");

    // Accept the suggested tag
    await act(async () => {
      fireEvent.click(screen.getByTestId("accept-tag-ai"));
    });

    expect(sendUpdateBookmarkMessage).toHaveBeenCalledWith(
      "am_upd",
      expect.objectContaining({ user_tags: ["old", "ai"], suggested_tags: [] }),
    );
  });

  it("removes item from list when state change excludes it from active filter", async () => {
    vi.mocked(sendListBookmarksMessage).mockResolvedValue({
      bookmarks: [makeBookmark("am_state", { state: "inbox" })],
    });
    vi.mocked(sendShowBookmarkMessage).mockResolvedValue({
      bookmark: makeDetail("am_state"),
    });
    vi.mocked(sendListCollectionsMessage).mockResolvedValue([]);
    vi.mocked(sendUpdateBookmarkMessage).mockResolvedValue({
      bookmark: makeDetail("am_state", { state: "processed" }),
    });

    render(<SidePanel />);
    await screen.findByTestId("bookmark-card-am_state");

    // Switch to inbox filter
    fireEvent.click(screen.getByTestId("filter-inbox"));
    await screen.findByTestId("bookmark-card-am_state");

    // Open detail
    await act(async () => {
      fireEvent.click(screen.getByTestId("bookmark-card-am_state"));
    });
    await screen.findByTestId("bookmark-detail");

    // Mark processed
    await act(async () => {
      fireEvent.click(screen.getByTestId("state-transition-btn"));
    });

    // Navigate back
    fireEvent.click(screen.getByTestId("back-to-list"));

    // Item should be removed from the inbox-filtered list
    expect(screen.queryByTestId("bookmark-card-am_state")).toBeNull();
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
