// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import BookmarkList from "./BookmarkList";
import type { BookmarkSummary } from "../shared/types";

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

describe("BookmarkList", () => {
  afterEach(cleanup);
  it("renders list of bookmarks as cards", () => {
    const bookmarks = [makeBookmark("1"), makeBookmark("2")];
    render(
      <BookmarkList
        bookmarks={bookmarks}
        activeFilter="all"
        onFilterChange={vi.fn()}
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByTestId("bookmark-card-1")).toBeDefined();
    expect(screen.getByTestId("bookmark-card-2")).toBeDefined();
  });

  it("renders empty state when no bookmarks", () => {
    render(
      <BookmarkList
        bookmarks={[]}
        activeFilter="all"
        onFilterChange={vi.fn()}
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByTestId("empty-state").textContent).toContain("No bookmarks found");
  });

  it("renders all filter tabs", () => {
    render(
      <BookmarkList
        bookmarks={[]}
        activeFilter="all"
        onFilterChange={vi.fn()}
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getByTestId("filter-all")).toBeDefined();
    expect(screen.getByTestId("filter-inbox")).toBeDefined();
    expect(screen.getByTestId("filter-processed")).toBeDefined();
    expect(screen.getByTestId("filter-archived")).toBeDefined();
  });

  it("calls onFilterChange when filter tab is clicked", () => {
    const onFilterChange = vi.fn();
    render(
      <BookmarkList
        bookmarks={[]}
        activeFilter="all"
        onFilterChange={onFilterChange}
        onSelect={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByTestId("filter-inbox"));
    expect(onFilterChange).toHaveBeenCalledWith("inbox");
  });

  it("calls onSelect when card is clicked", () => {
    const onSelect = vi.fn();
    render(
      <BookmarkList
        bookmarks={[makeBookmark("1")]}
        activeFilter="all"
        onFilterChange={vi.fn()}
        onSelect={onSelect}
      />,
    );
    fireEvent.click(screen.getByTestId("bookmark-card-1"));
    expect(onSelect).toHaveBeenCalledWith("1");
  });

  it("highlights active filter tab", () => {
    render(
      <BookmarkList
        bookmarks={[]}
        activeFilter="inbox"
        onFilterChange={vi.fn()}
        onSelect={vi.fn()}
      />,
    );
    const inboxTab = screen.getByTestId("filter-inbox");
    expect(inboxTab.className).toContain("text-indigo-600");
    const allTab = screen.getByTestId("filter-all");
    expect(allTab.className).not.toContain("text-indigo-600");
  });
});
