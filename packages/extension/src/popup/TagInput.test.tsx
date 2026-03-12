/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import TagInput from "./TagInput";

describe("TagInput", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders existing tags as pills", () => {
    render(<TagInput tags={["rust", "cli"]} onChange={() => {}} />);
    const pills = screen.getAllByTestId("tag-pill");
    expect(pills).toHaveLength(2);
    expect(pills[0].textContent).toContain("rust");
    expect(pills[1].textContent).toContain("cli");
  });

  it("renders empty state with placeholder", () => {
    render(<TagInput tags={[]} onChange={() => {}} />);
    const input = screen.getByTestId("tag-input") as HTMLInputElement;
    expect(input.placeholder).toBe("Add tags...");
  });

  it("commits tag on Enter", () => {
    const onChange = vi.fn();
    render(<TagInput tags={[]} onChange={onChange} />);
    const input = screen.getByTestId("tag-input");
    fireEvent.change(input, { target: { value: "newtag" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith(["newtag"]);
  });

  it("commits tag on comma", () => {
    const onChange = vi.fn();
    render(<TagInput tags={[]} onChange={onChange} />);
    const input = screen.getByTestId("tag-input");
    fireEvent.change(input, { target: { value: "tag1," } });
    expect(onChange).toHaveBeenCalledWith(["tag1"]);
  });

  it("commits tag on blur", () => {
    const onChange = vi.fn();
    render(<TagInput tags={[]} onChange={onChange} />);
    const input = screen.getByTestId("tag-input");
    fireEvent.change(input, { target: { value: "blurtag" } });
    fireEvent.blur(input);
    expect(onChange).toHaveBeenCalledWith(["blurtag"]);
  });

  it("deduplicates tags", () => {
    const onChange = vi.fn();
    render(<TagInput tags={["existing"]} onChange={onChange} />);
    const input = screen.getByTestId("tag-input");
    fireEvent.change(input, { target: { value: "existing" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith(["existing"]);
  });

  it("trims whitespace and lowercases", () => {
    const onChange = vi.fn();
    render(<TagInput tags={[]} onChange={onChange} />);
    const input = screen.getByTestId("tag-input");
    fireEvent.change(input, { target: { value: "  MyTag  " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith(["mytag"]);
  });

  it("ignores blank tags from repeated commas", () => {
    const onChange = vi.fn();
    render(<TagInput tags={[]} onChange={onChange} />);
    const input = screen.getByTestId("tag-input");
    fireEvent.change(input, { target: { value: ",,," } });
    expect(onChange).not.toHaveBeenCalled();
  });

  it("handles multiple comma-separated tags at once", () => {
    const onChange = vi.fn();
    render(<TagInput tags={[]} onChange={onChange} />);
    const input = screen.getByTestId("tag-input");
    fireEvent.change(input, { target: { value: "a, b, c," } });
    expect(onChange).toHaveBeenCalledWith(["a", "b", "c"]);
  });

  it("removes tag when pill close button is clicked", () => {
    const onChange = vi.fn();
    render(<TagInput tags={["alpha", "beta"]} onChange={onChange} />);
    const removeButtons = screen.getAllByLabelText(/Remove tag/);
    fireEvent.click(removeButtons[0]);
    expect(onChange).toHaveBeenCalledWith(["beta"]);
  });

  it("removes last tag on Backspace when input is empty", () => {
    const onChange = vi.fn();
    render(<TagInput tags={["alpha", "beta"]} onChange={onChange} />);
    const input = screen.getByTestId("tag-input");
    fireEvent.keyDown(input, { key: "Backspace" });
    expect(onChange).toHaveBeenCalledWith(["alpha"]);
  });

  it("does not remove tags on Backspace when input has text", () => {
    const onChange = vi.fn();
    render(<TagInput tags={["alpha"]} onChange={onChange} />);
    const input = screen.getByTestId("tag-input");
    fireEvent.change(input, { target: { value: "x" } });
    fireEvent.keyDown(input, { key: "Backspace" });
    expect(onChange).not.toHaveBeenCalled();
  });
});
