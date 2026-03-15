// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup, act } from "@testing-library/react";
import EditableField from "./EditableField";

describe("EditableField", () => {
  afterEach(cleanup);

  it("renders display value", () => {
    render(<EditableField value="hello" onSave={vi.fn()} label="Note" />);
    expect(screen.getByTestId("editable-field-display").textContent).toBe("hello");
  });

  it("renders placeholder when value is empty", () => {
    render(<EditableField value="" onSave={vi.fn()} label="Note" placeholder="Add note" />);
    expect(screen.getByTestId("editable-field-display").textContent).toContain("Add note");
  });

  it("enters edit mode on click", () => {
    render(<EditableField value="hello" onSave={vi.fn()} label="Note" />);
    fireEvent.click(screen.getByTestId("editable-field-display"));
    expect(screen.getByTestId("editable-field-input")).toBeDefined();
  });

  it("saves on blur", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(<EditableField value="old" onSave={onSave} label="Note" />);

    fireEvent.click(screen.getByTestId("editable-field-display"));
    const input = screen.getByTestId("editable-field-input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "new value" } });

    await act(async () => {
      fireEvent.blur(input);
    });

    expect(onSave).toHaveBeenCalledWith("new value");
  });

  it("saves on Enter for single-line", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(<EditableField value="old" onSave={onSave} label="Note" />);

    fireEvent.click(screen.getByTestId("editable-field-display"));
    const input = screen.getByTestId("editable-field-input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "updated" } });

    await act(async () => {
      fireEvent.keyDown(input, { key: "Enter" });
    });

    expect(onSave).toHaveBeenCalledWith("updated");
  });

  it("cancels on Escape", () => {
    render(<EditableField value="original" onSave={vi.fn()} label="Note" />);

    fireEvent.click(screen.getByTestId("editable-field-display"));
    const input = screen.getByTestId("editable-field-input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "changed" } });
    fireEvent.keyDown(input, { key: "Escape" });

    // Should return to display mode with original value
    expect(screen.getByTestId("editable-field-display").textContent).toBe("original");
  });

  it("does not save if value unchanged", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(<EditableField value="same" onSave={onSave} label="Note" />);

    fireEvent.click(screen.getByTestId("editable-field-display"));
    const input = screen.getByTestId("editable-field-input") as HTMLInputElement;

    await act(async () => {
      fireEvent.blur(input);
    });

    expect(onSave).not.toHaveBeenCalled();
  });

  it("shows error on save failure", async () => {
    const onSave = vi.fn().mockRejectedValue(new Error("Save failed"));
    render(<EditableField value="old" onSave={onSave} label="Note" />);

    fireEvent.click(screen.getByTestId("editable-field-display"));
    const input = screen.getByTestId("editable-field-input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "new" } });

    await act(async () => {
      fireEvent.blur(input);
    });

    expect(screen.getByTestId("editable-field-error").textContent).toContain("Save failed");
    // Should stay in edit mode
    expect(screen.getByTestId("editable-field-input")).toBeDefined();
  });

  it("trims whitespace before saving", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(<EditableField value="" onSave={onSave} label="Note" />);

    fireEvent.click(screen.getByTestId("editable-field-display"));
    const input = screen.getByTestId("editable-field-input") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "  trimmed  " } });

    await act(async () => {
      fireEvent.blur(input);
    });

    expect(onSave).toHaveBeenCalledWith("trimmed");
  });
});
