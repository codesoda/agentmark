// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import TagManager from "./TagManager";

describe("TagManager", () => {
  afterEach(cleanup);

  it("renders suggested tags with accept and reject buttons", () => {
    render(
      <TagManager
        suggestedTags={["ai", "rust"]}
        userTags={[]}
        onAccept={vi.fn()}
        onReject={vi.fn()}
      />,
    );

    const tags = screen.getAllByTestId("suggested-tag");
    expect(tags).toHaveLength(2);
    expect(screen.getByTestId("accept-tag-ai")).toBeDefined();
    expect(screen.getByTestId("reject-tag-ai")).toBeDefined();
    expect(screen.getByTestId("accept-tag-rust")).toBeDefined();
    expect(screen.getByTestId("reject-tag-rust")).toBeDefined();
  });

  it("calls onAccept when accept button is clicked", () => {
    const onAccept = vi.fn();
    render(
      <TagManager
        suggestedTags={["ai"]}
        userTags={[]}
        onAccept={onAccept}
        onReject={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByTestId("accept-tag-ai"));
    expect(onAccept).toHaveBeenCalledWith("ai");
  });

  it("calls onReject when reject button is clicked", () => {
    const onReject = vi.fn();
    render(
      <TagManager
        suggestedTags={["ai"]}
        userTags={[]}
        onAccept={vi.fn()}
        onReject={onReject}
      />,
    );

    fireEvent.click(screen.getByTestId("reject-tag-ai"));
    expect(onReject).toHaveBeenCalledWith("ai");
  });

  it("renders nothing when no suggested tags", () => {
    const { container } = render(
      <TagManager
        suggestedTags={[]}
        userTags={[]}
        onAccept={vi.fn()}
        onReject={vi.fn()}
      />,
    );

    expect(container.innerHTML).toBe("");
  });

  it("filters out suggested tags already in user_tags", () => {
    render(
      <TagManager
        suggestedTags={["ai", "rust"]}
        userTags={["ai"]}
        onAccept={vi.fn()}
        onReject={vi.fn()}
      />,
    );

    const tags = screen.getAllByTestId("suggested-tag");
    expect(tags).toHaveLength(1);
    expect(tags[0].textContent).toContain("rust");
  });

  it("renders nothing when all suggested tags are already user tags", () => {
    const { container } = render(
      <TagManager
        suggestedTags={["ai"]}
        userTags={["ai"]}
        onAccept={vi.fn()}
        onReject={vi.fn()}
      />,
    );

    expect(container.innerHTML).toBe("");
  });

  it("disables buttons when disabled prop is true", () => {
    render(
      <TagManager
        suggestedTags={["ai"]}
        userTags={[]}
        onAccept={vi.fn()}
        onReject={vi.fn()}
        disabled
      />,
    );

    expect((screen.getByTestId("accept-tag-ai") as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByTestId("reject-tag-ai") as HTMLButtonElement).disabled).toBe(true);
  });
});
