/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import SaveForm from "./SaveForm";

const defaultProps = {
  initialTags: [],
  initialCollection: undefined,
  initialSelectedText: "",
  collections: ["reading", "work"],
  onSubmit: vi.fn(),
  onCancel: vi.fn(),
  submitting: false,
};

describe("SaveForm", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("renders all form fields", () => {
    render(<SaveForm {...defaultProps} initialSelectedText="some text" />);

    expect(screen.getByTestId("tag-input")).toBeDefined();
    expect(screen.getByTestId("collection-select")).toBeDefined();
    expect(screen.getByTestId("note-input")).toBeDefined();
    expect(screen.getByTestId("action-input")).toBeDefined();
    expect(screen.getByTestId("selected-text-input")).toBeDefined();
    expect(screen.getByTestId("form-submit")).toBeDefined();
    expect(screen.getByTestId("form-cancel")).toBeDefined();
  });

  it("hides selected text field when empty", () => {
    render(<SaveForm {...defaultProps} />);
    expect(screen.queryByTestId("selected-text-input")).toBeNull();
  });

  it("pre-fills initial tags", () => {
    render(<SaveForm {...defaultProps} initialTags={["tag1", "tag2"]} />);
    const pills = screen.getAllByTestId("tag-pill");
    expect(pills).toHaveLength(2);
  });

  it("pre-fills initial collection", () => {
    render(<SaveForm {...defaultProps} initialCollection="work" />);
    const select = screen.getByTestId("collection-select") as HTMLSelectElement;
    expect(select.value).toBe("work");
  });

  it("pre-fills selected text", () => {
    render(<SaveForm {...defaultProps} initialSelectedText="quoted text" />);
    const textarea = screen.getByTestId("selected-text-input") as HTMLTextAreaElement;
    expect(textarea.value).toBe("quoted text");
  });

  it("submits with all values", () => {
    const onSubmit = vi.fn();
    render(
      <SaveForm
        {...defaultProps}
        onSubmit={onSubmit}
        initialTags={["rust"]}
        initialCollection="reading"
        initialSelectedText="selection"
      />,
    );

    fireEvent.change(screen.getByTestId("note-input"), { target: { value: "my note" } });
    fireEvent.change(screen.getByTestId("action-input"), { target: { value: "summarize" } });
    fireEvent.click(screen.getByTestId("form-submit"));

    expect(onSubmit).toHaveBeenCalledWith({
      tags: ["rust"],
      collection: "reading",
      note: "my note",
      action: "summarize",
      selected_text: "selection",
    });
  });

  it("submits with all fields empty", () => {
    const onSubmit = vi.fn();
    render(<SaveForm {...defaultProps} onSubmit={onSubmit} />);
    fireEvent.click(screen.getByTestId("form-submit"));
    expect(onSubmit).toHaveBeenCalledWith({
      tags: [],
      collection: undefined,
      note: undefined,
      action: undefined,
      selected_text: undefined,
    });
  });

  it("trims whitespace-only note to undefined", () => {
    const onSubmit = vi.fn();
    render(<SaveForm {...defaultProps} onSubmit={onSubmit} />);
    fireEvent.change(screen.getByTestId("note-input"), { target: { value: "   " } });
    fireEvent.click(screen.getByTestId("form-submit"));
    expect(onSubmit.mock.calls[0][0].note).toBeUndefined();
  });

  it("calls onCancel when cancel button is clicked", () => {
    const onCancel = vi.fn();
    render(<SaveForm {...defaultProps} onCancel={onCancel} />);
    fireEvent.click(screen.getByTestId("form-cancel"));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("disables buttons when submitting", () => {
    render(<SaveForm {...defaultProps} submitting={true} />);
    expect(
      (screen.getByTestId("form-submit") as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(
      (screen.getByTestId("form-cancel") as HTMLButtonElement).disabled,
    ).toBe(true);
  });

  it("shows submit button text as Saving... when submitting", () => {
    render(<SaveForm {...defaultProps} submitting={true} />);
    expect(screen.getByTestId("form-submit").textContent).toBe("Saving...");
  });

  it("displays error message", () => {
    render(<SaveForm {...defaultProps} error="Save failed" />);
    expect(screen.getByTestId("form-error")).toBeDefined();
    expect(screen.getByText("Save failed")).toBeDefined();
  });
});
