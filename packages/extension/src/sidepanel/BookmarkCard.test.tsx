// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import BookmarkCard from "./BookmarkCard";
import { formatDate } from "./format";
import type { BookmarkSummary } from "../shared/types";

function makeBookmark(overrides: Partial<BookmarkSummary> = {}): BookmarkSummary {
  return {
    id: "am_123",
    url: "https://example.com",
    title: "Example Page",
    state: "inbox",
    user_tags: [],
    suggested_tags: [],
    saved_at: "2026-03-12T00:00:00Z",
    ...overrides,
  };
}

describe("BookmarkCard", () => {
  afterEach(cleanup);
  it("renders title", () => {
    render(<BookmarkCard bookmark={makeBookmark()} onSelect={vi.fn()} />);
    expect(screen.getByText("Example Page")).toBeDefined();
  });

  it("falls back to URL when title is empty", () => {
    render(<BookmarkCard bookmark={makeBookmark({ title: "" })} onSelect={vi.fn()} />);
    expect(screen.getByText("https://example.com")).toBeDefined();
  });

  it("renders formatted date", () => {
    render(<BookmarkCard bookmark={makeBookmark()} onSelect={vi.fn()} />);
    const dateEl = screen.getByTestId("saved-date");
    expect(dateEl.textContent).toBeTruthy();
  });

  it("renders state badge", () => {
    render(<BookmarkCard bookmark={makeBookmark({ state: "processed" })} onSelect={vi.fn()} />);
    expect(screen.getByTestId("state-badge").textContent).toContain("Processed");
  });

  it("renders inbox state badge", () => {
    render(<BookmarkCard bookmark={makeBookmark({ state: "inbox" })} onSelect={vi.fn()} />);
    expect(screen.getByTestId("state-badge").textContent).toContain("Inbox");
  });

  it("renders archived state badge", () => {
    render(<BookmarkCard bookmark={makeBookmark({ state: "archived" })} onSelect={vi.fn()} />);
    expect(screen.getByTestId("state-badge").textContent).toContain("Archived");
  });

  it("renders user tags with distinct styling", () => {
    render(
      <BookmarkCard
        bookmark={makeBookmark({ user_tags: ["rust", "cli"] })}
        onSelect={vi.fn()}
      />,
    );
    const userTags = screen.getAllByTestId("user-tag");
    expect(userTags).toHaveLength(2);
    expect(userTags[0].textContent).toContain("rust");
    expect(userTags[1].textContent).toContain("cli");
  });

  it("renders suggested tags with distinct styling", () => {
    render(
      <BookmarkCard
        bookmark={makeBookmark({ suggested_tags: ["dev", "web"] })}
        onSelect={vi.fn()}
      />,
    );
    const suggestedTags = screen.getAllByTestId("suggested-tag");
    expect(suggestedTags).toHaveLength(2);
    expect(suggestedTags[0].textContent).toContain("dev");
  });

  it("renders both user and suggested tags together", () => {
    render(
      <BookmarkCard
        bookmark={makeBookmark({ user_tags: ["rust"], suggested_tags: ["dev"] })}
        onSelect={vi.fn()}
      />,
    );
    expect(screen.getAllByTestId("user-tag")).toHaveLength(1);
    expect(screen.getAllByTestId("suggested-tag")).toHaveLength(1);
  });

  it("does not render tag section when no tags", () => {
    render(<BookmarkCard bookmark={makeBookmark()} onSelect={vi.fn()} />);
    expect(screen.queryByTestId("user-tag")).toBeNull();
    expect(screen.queryByTestId("suggested-tag")).toBeNull();
  });

  it("calls onSelect with bookmark ID on click", () => {
    const onSelect = vi.fn();
    render(<BookmarkCard bookmark={makeBookmark()} onSelect={onSelect} />);
    fireEvent.click(screen.getByTestId("bookmark-card-am_123"));
    expect(onSelect).toHaveBeenCalledWith("am_123");
  });
});

describe("formatDate", () => {
  it("formats ISO date string", () => {
    const result = formatDate("2026-03-12T00:00:00Z");
    expect(result).toBeTruthy();
    expect(result).toContain("2026");
  });

  it("returns raw string for invalid date", () => {
    expect(formatDate("not-a-date")).toBe("not-a-date");
  });
});
