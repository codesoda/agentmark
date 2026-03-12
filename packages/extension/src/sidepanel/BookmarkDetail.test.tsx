// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup, act } from "@testing-library/react";
import type { BookmarkDetail as BookmarkDetailType } from "../shared/types";
import BookmarkDetail from "./BookmarkDetail";

function makeDetail(overrides: Partial<BookmarkDetailType> = {}): BookmarkDetailType {
  return {
    id: "am_test",
    url: "https://example.com/test",
    title: "Test Bookmark",
    summary: "A test summary",
    saved_at: "2026-03-12T00:00:00Z",
    capture_source: "cli",
    state: "inbox",
    user_tags: ["rust"],
    suggested_tags: ["ai", "web"],
    collections: ["reading"],
    note: "Test note",
    ...overrides,
  };
}

describe("BookmarkDetail", () => {
  afterEach(cleanup);

  it("renders all fields", () => {
    const onUpdate = vi.fn().mockResolvedValue(undefined);
    render(
      <BookmarkDetail
        bookmark={makeDetail()}
        collections={["reading", "work"]}
        onUpdate={onUpdate}
        onBack={vi.fn()}
        updating={false}
      />,
    );

    expect(screen.getByTestId("detail-title").textContent).toBe("Test Bookmark");
    expect(screen.getByTestId("detail-title").getAttribute("href")).toBe("https://example.com/test");
    expect(screen.getByTestId("detail-summary").textContent).toBe("A test summary");
    expect(screen.getByTestId("detail-date")).toBeDefined();
    expect(screen.getByTestId("detail-source").textContent).toBe("CLI");
    expect(screen.getByTestId("detail-state").textContent).toBe("Inbox");
  });

  it("falls back to URL when title is empty", () => {
    render(
      <BookmarkDetail
        bookmark={makeDetail({ title: "" })}
        collections={[]}
        onUpdate={vi.fn().mockResolvedValue(undefined)}
        onBack={vi.fn()}
        updating={false}
      />,
    );

    expect(screen.getByTestId("detail-title").textContent).toBe("https://example.com/test");
  });

  it("hides summary when null", () => {
    render(
      <BookmarkDetail
        bookmark={makeDetail({ summary: null })}
        collections={[]}
        onUpdate={vi.fn().mockResolvedValue(undefined)}
        onBack={vi.fn()}
        updating={false}
      />,
    );

    expect(screen.queryByTestId("detail-summary")).toBeNull();
  });

  it("calls onBack when back button clicked", () => {
    const onBack = vi.fn();
    render(
      <BookmarkDetail
        bookmark={makeDetail()}
        collections={[]}
        onUpdate={vi.fn().mockResolvedValue(undefined)}
        onBack={onBack}
        updating={false}
      />,
    );

    fireEvent.click(screen.getByTestId("back-to-list"));
    expect(onBack).toHaveBeenCalled();
  });

  it("shows state transition button for inbox", () => {
    render(
      <BookmarkDetail
        bookmark={makeDetail({ state: "inbox" })}
        collections={[]}
        onUpdate={vi.fn().mockResolvedValue(undefined)}
        onBack={vi.fn()}
        updating={false}
      />,
    );

    expect(screen.getByTestId("state-transition-btn").textContent).toBe("Mark Processed");
  });

  it("shows archive button for processed", () => {
    render(
      <BookmarkDetail
        bookmark={makeDetail({ state: "processed" })}
        collections={[]}
        onUpdate={vi.fn().mockResolvedValue(undefined)}
        onBack={vi.fn()}
        updating={false}
      />,
    );

    expect(screen.getByTestId("state-transition-btn").textContent).toBe("Archive");
  });

  it("hides state transition button for archived", () => {
    render(
      <BookmarkDetail
        bookmark={makeDetail({ state: "archived" })}
        collections={[]}
        onUpdate={vi.fn().mockResolvedValue(undefined)}
        onBack={vi.fn()}
        updating={false}
      />,
    );

    expect(screen.queryByTestId("state-transition-btn")).toBeNull();
  });

  it("calls onUpdate with state change", async () => {
    const onUpdate = vi.fn().mockResolvedValue(undefined);
    render(
      <BookmarkDetail
        bookmark={makeDetail({ state: "inbox" })}
        collections={[]}
        onUpdate={onUpdate}
        onBack={vi.fn()}
        updating={false}
      />,
    );

    await act(async () => {
      fireEvent.click(screen.getByTestId("state-transition-btn"));
    });

    expect(onUpdate).toHaveBeenCalledWith({ state: "processed" });
  });

  it("renders suggested tags with accept/reject buttons", () => {
    render(
      <BookmarkDetail
        bookmark={makeDetail({ suggested_tags: ["ai", "web"] })}
        collections={[]}
        onUpdate={vi.fn().mockResolvedValue(undefined)}
        onBack={vi.fn()}
        updating={false}
      />,
    );

    expect(screen.getByTestId("accept-tag-ai")).toBeDefined();
    expect(screen.getByTestId("reject-tag-ai")).toBeDefined();
  });

  it("calls onUpdate with accepted tag", async () => {
    const onUpdate = vi.fn().mockResolvedValue(undefined);
    render(
      <BookmarkDetail
        bookmark={makeDetail({ user_tags: ["rust"], suggested_tags: ["ai"] })}
        collections={[]}
        onUpdate={onUpdate}
        onBack={vi.fn()}
        updating={false}
      />,
    );

    await act(async () => {
      fireEvent.click(screen.getByTestId("accept-tag-ai"));
    });

    expect(onUpdate).toHaveBeenCalledWith({
      user_tags: ["rust", "ai"],
      suggested_tags: [],
    });
  });

  it("calls onUpdate with rejected tag", async () => {
    const onUpdate = vi.fn().mockResolvedValue(undefined);
    render(
      <BookmarkDetail
        bookmark={makeDetail({ suggested_tags: ["ai", "web"] })}
        collections={[]}
        onUpdate={onUpdate}
        onBack={vi.fn()}
        updating={false}
      />,
    );

    await act(async () => {
      fireEvent.click(screen.getByTestId("reject-tag-ai"));
    });

    expect(onUpdate).toHaveBeenCalledWith({
      suggested_tags: ["web"],
    });
  });

  it("does not duplicate when accepting tag already in user_tags", async () => {
    const onUpdate = vi.fn().mockResolvedValue(undefined);
    render(
      <BookmarkDetail
        bookmark={makeDetail({ user_tags: ["ai"], suggested_tags: ["ai"] })}
        collections={[]}
        onUpdate={onUpdate}
        onBack={vi.fn()}
        updating={false}
      />,
    );

    // Tag "ai" is in both arrays — TagManager should filter it out
    // so there should be no accept button visible
    expect(screen.queryByTestId("accept-tag-ai")).toBeNull();
  });
});
