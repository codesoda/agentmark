/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import CollectionSelect from "./CollectionSelect";

describe("CollectionSelect", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders existing collections in dropdown", () => {
    render(
      <CollectionSelect collections={["reading", "work"]} onChange={() => {}} />,
    );
    const select = screen.getByTestId("collection-select") as HTMLSelectElement;
    const options = Array.from(select.options).map((o) => o.value);
    expect(options).toContain("reading");
    expect(options).toContain("work");
  });

  it("renders None option by default", () => {
    render(
      <CollectionSelect collections={[]} onChange={() => {}} />,
    );
    const select = screen.getByTestId("collection-select") as HTMLSelectElement;
    expect(select.value).toBe("");
    expect(Array.from(select.options).map((o) => o.text)).toContain("None");
  });

  it("selects existing collection", () => {
    const onChange = vi.fn();
    render(
      <CollectionSelect collections={["reading"]} value="reading" onChange={onChange} />,
    );
    const select = screen.getByTestId("collection-select") as HTMLSelectElement;
    expect(select.value).toBe("reading");
  });

  it("calls onChange with selected value", () => {
    const onChange = vi.fn();
    render(
      <CollectionSelect collections={["reading", "work"]} onChange={onChange} />,
    );
    fireEvent.change(screen.getByTestId("collection-select"), { target: { value: "work" } });
    expect(onChange).toHaveBeenCalledWith("work");
  });

  it("calls onChange with undefined when None is selected", () => {
    const onChange = vi.fn();
    render(
      <CollectionSelect collections={["reading"]} value="reading" onChange={onChange} />,
    );
    fireEvent.change(screen.getByTestId("collection-select"), { target: { value: "" } });
    expect(onChange).toHaveBeenCalledWith(undefined);
  });

  it("shows text input when New collection is selected", () => {
    render(
      <CollectionSelect collections={["reading"]} onChange={() => {}} />,
    );
    fireEvent.change(screen.getByTestId("collection-select"), { target: { value: "__new__" } });
    expect(screen.getByTestId("collection-custom-input")).toBeDefined();
  });

  it("calls onChange with custom text value", () => {
    const onChange = vi.fn();
    render(
      <CollectionSelect collections={[]} onChange={onChange} />,
    );
    fireEvent.change(screen.getByTestId("collection-select"), { target: { value: "__new__" } });
    fireEvent.change(screen.getByTestId("collection-custom-input"), {
      target: { value: "my-collection" },
    });
    expect(onChange).toHaveBeenCalledWith("my-collection");
  });

  it("calls onChange with undefined for blank custom text", () => {
    const onChange = vi.fn();
    render(
      <CollectionSelect collections={[]} onChange={onChange} />,
    );
    fireEvent.change(screen.getByTestId("collection-select"), { target: { value: "__new__" } });
    fireEvent.change(screen.getByTestId("collection-custom-input"), {
      target: { value: "   " },
    });
    expect(onChange).toHaveBeenCalledWith(undefined);
  });

  it("renders with no existing collections", () => {
    render(
      <CollectionSelect collections={[]} onChange={() => {}} />,
    );
    const select = screen.getByTestId("collection-select") as HTMLSelectElement;
    // Should have None and New collection... options only
    expect(select.options).toHaveLength(2);
  });
});
