/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, cleanup, fireEvent } from "@testing-library/react";
import { resetChromeMock } from "../test/chrome-mock";
import type { RuntimeResponse } from "../shared/types";

// Mock the runtime module so we can control responses
vi.mock("../shared/runtime", () => ({
  queryActiveTab: vi.fn(),
  sendSaveMessage: vi.fn(),
  sendListCollectionsMessage: vi.fn(),
  loadLastUsedIntent: vi.fn(),
  saveLastUsedIntent: vi.fn(),
  querySelectedText: vi.fn(),
  isConnectionError: vi.fn(),
}));

import Popup from "./Popup";
import {
  queryActiveTab,
  sendSaveMessage,
  sendListCollectionsMessage,
  loadLastUsedIntent,
  saveLastUsedIntent,
  querySelectedText,
  isConnectionError,
} from "../shared/runtime";

const mockQueryActiveTab = vi.mocked(queryActiveTab);
const mockSendSaveMessage = vi.mocked(sendSaveMessage);
const mockSendListCollections = vi.mocked(sendListCollectionsMessage);
const mockLoadLastUsedIntent = vi.mocked(loadLastUsedIntent);
const mockSaveLastUsedIntent = vi.mocked(saveLastUsedIntent);
const mockQuerySelectedText = vi.mocked(querySelectedText);
const mockIsConnectionError = vi.mocked(isConnectionError);

function setupSuccessfulQuickSave() {
  mockQueryActiveTab.mockResolvedValue({
    url: "https://example.com",
    title: "Example Page",
    id: 42,
  });
  const successResponse: RuntimeResponse = {
    success: true,
    data: { type: "save_result", id: "bm_abc123", path: "/tmp/path", status: "created" },
  };
  mockSendSaveMessage.mockResolvedValue(successResponse);
}

function setupFormDefaults() {
  mockSendListCollections.mockResolvedValue(["reading", "work"]);
  mockLoadLastUsedIntent.mockResolvedValue({ tags: ["saved-tag"], collection: "reading" });
  mockQuerySelectedText.mockResolvedValue("selected content");
  mockSaveLastUsedIntent.mockResolvedValue(undefined);
}

describe("Popup", () => {
  beforeEach(() => {
    resetChromeMock();
    vi.useFakeTimers();
    vi.spyOn(window, "close").mockImplementation(() => {});
    mockIsConnectionError.mockReturnValue(false);
    setupFormDefaults();
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  // -- Quick-save mode (Spec 20 behavior preserved) --

  it("renders saving state immediately", async () => {
    mockQueryActiveTab.mockImplementation(() => new Promise(() => {})); // never resolves

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("saving-state")).toBeDefined();
    expect(screen.getByText("Saving...")).toBeDefined();
  });

  it("shows page title in saving state", async () => {
    let resolveTab!: (value: { url: string; title: string; favIconUrl?: string; id?: number }) => void;
    mockQueryActiveTab.mockImplementation(
      () => new Promise((r) => { resolveTab = r; }),
    );
    mockSendSaveMessage.mockImplementation(() => new Promise(() => {}));

    await act(async () => {
      render(<Popup />);
    });

    await act(async () => {
      resolveTab({ url: "https://example.com", title: "Example Page" });
    });

    expect(screen.getByText("Example Page")).toBeDefined();
    expect(screen.getByText("Saving...")).toBeDefined();
  });

  it("renders success state with title and bookmark ID", async () => {
    setupSuccessfulQuickSave();

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("success-state")).toBeDefined();
    expect(screen.getByText("Saved!")).toBeDefined();
    expect(screen.getByText("Example Page")).toBeDefined();
    expect(screen.getByText("ID: bm_abc123")).toBeDefined();
  });

  it("sends correct save payload", async () => {
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test Title",
      id: 1,
    });
    mockSendSaveMessage.mockResolvedValue({
      success: true,
      data: { type: "save_result", id: "bm_1", path: "/tmp", status: "created" },
    });

    await act(async () => {
      render(<Popup />);
    });

    expect(mockSendSaveMessage).toHaveBeenCalledWith("https://example.com", "Test Title");
    expect(mockSendSaveMessage).toHaveBeenCalledTimes(1);
  });

  it("auto-closes after 2 seconds on success", async () => {
    setupSuccessfulQuickSave();

    await act(async () => {
      render(<Popup />);
    });

    expect(window.close).not.toHaveBeenCalled();

    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    expect(window.close).toHaveBeenCalledTimes(1);
  });

  it("does not double-close", async () => {
    setupSuccessfulQuickSave();

    await act(async () => {
      render(<Popup />);
    });

    await act(async () => {
      vi.advanceTimersByTime(5000);
    });

    expect(window.close).toHaveBeenCalledTimes(1);
  });

  it("renders error state on save failure", async () => {
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test",
      id: 1,
    });
    mockSendSaveMessage.mockResolvedValue({
      success: false,
      error: "Failed to save bookmark",
    });

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("error-state")).toBeDefined();
    expect(screen.getByText("Save Failed")).toBeDefined();
    expect(screen.getByText("Failed to save bookmark")).toBeDefined();
  });

  it("renders error state when tab query fails", async () => {
    mockQueryActiveTab.mockRejectedValue(new Error("Unsupported page: chrome://extensions"));

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("error-state")).toBeDefined();
    expect(screen.getByText("Unsupported page: chrome://extensions")).toBeDefined();
  });

  it("shows install guidance for connection errors", async () => {
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test",
      id: 1,
    });
    mockSendSaveMessage.mockResolvedValue({
      success: false,
      error: "native messaging host not found",
    });
    mockIsConnectionError.mockReturnValue(true);

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("install-guidance")).toBeDefined();
    expect(screen.getByText("Native host not connected")).toBeDefined();
  });

  it("does not show install guidance for non-connection errors", async () => {
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test",
      id: 1,
    });
    mockSendSaveMessage.mockResolvedValue({
      success: false,
      error: "Some other error",
    });
    mockIsConnectionError.mockReturnValue(false);

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("error-state")).toBeDefined();
    expect(screen.queryByTestId("install-guidance")).toBeNull();
  });

  it("does not dispatch a second save under StrictMode", async () => {
    setupSuccessfulQuickSave();

    const { StrictMode } = await import("react");

    await act(async () => {
      render(
        <StrictMode>
          <Popup />
        </StrictMode>,
      );
    });

    expect(mockSendSaveMessage).toHaveBeenCalledTimes(1);
  });

  it("renders More options button on success", async () => {
    setupSuccessfulQuickSave();

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("more-options-btn")).toBeDefined();
  });

  it("renders More options button on error", async () => {
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test",
      id: 1,
    });
    mockSendSaveMessage.mockResolvedValue({
      success: false,
      error: "some error",
    });

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("more-options-btn")).toBeDefined();
  });

  it("handles updated save status", async () => {
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test",
      id: 1,
    });
    mockSendSaveMessage.mockResolvedValue({
      success: true,
      data: { type: "save_result", id: "bm_1", path: "/tmp", status: "updated" },
    });

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("success-state")).toBeDefined();
    expect(screen.getByText("Saved!")).toBeDefined();
  });

  // -- Form mode (Spec 21) --

  it("switches to form mode when More options is clicked", async () => {
    setupSuccessfulQuickSave();

    await act(async () => {
      render(<Popup />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    expect(screen.getByTestId("form-mode")).toBeDefined();
    expect(screen.getByTestId("save-form")).toBeDefined();
  });

  it("cancels auto-close timer when entering form mode", async () => {
    setupSuccessfulQuickSave();

    await act(async () => {
      render(<Popup />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    // Advance past the auto-close window
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });

    // Should NOT have closed
    expect(window.close).not.toHaveBeenCalled();
  });

  it("lazily loads collections, defaults, and selected text when form opens", async () => {
    setupSuccessfulQuickSave();

    await act(async () => {
      render(<Popup />);
    });

    // Before clicking More options, lazy loaders should not have been called
    expect(mockSendListCollections).not.toHaveBeenCalled();
    expect(mockLoadLastUsedIntent).not.toHaveBeenCalled();
    expect(mockQuerySelectedText).not.toHaveBeenCalled();

    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    expect(mockSendListCollections).toHaveBeenCalledTimes(1);
    expect(mockLoadLastUsedIntent).toHaveBeenCalledTimes(1);
    expect(mockQuerySelectedText).toHaveBeenCalledWith(42);
  });

  it("pre-fills form with last-used tags and collection", async () => {
    setupSuccessfulQuickSave();

    await act(async () => {
      render(<Popup />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    // Should have a tag pill for the saved tag
    const pills = screen.getAllByTestId("tag-pill");
    expect(pills).toHaveLength(1);
    expect(pills[0].textContent).toContain("saved-tag");

    // Collection select should show "reading"
    const select = screen.getByTestId("collection-select") as HTMLSelectElement;
    expect(select.value).toBe("reading");
  });

  it("pre-fills selected text when available", async () => {
    setupSuccessfulQuickSave();
    mockQuerySelectedText.mockResolvedValue("highlighted text");

    await act(async () => {
      render(<Popup />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    const textArea = screen.getByTestId("selected-text-input") as HTMLTextAreaElement;
    expect(textArea.value).toBe("highlighted text");
  });

  it("does not show selected text field when no text was selected", async () => {
    setupSuccessfulQuickSave();
    mockQuerySelectedText.mockResolvedValue("");

    await act(async () => {
      render(<Popup />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    expect(screen.queryByTestId("selected-text-input")).toBeNull();
  });

  it("returns to quick-save view on cancel", async () => {
    setupSuccessfulQuickSave();

    await act(async () => {
      render(<Popup />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    expect(screen.getByTestId("form-mode")).toBeDefined();

    await act(async () => {
      fireEvent.click(screen.getByTestId("form-cancel"));
    });

    expect(screen.queryByTestId("form-mode")).toBeNull();
    expect(screen.getByTestId("success-state")).toBeDefined();
  });

  it("restarts auto-close timer when cancelling from success state", async () => {
    setupSuccessfulQuickSave();

    await act(async () => {
      render(<Popup />);
    });

    // Enter form mode (cancels timer)
    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    // Cancel back to success
    await act(async () => {
      fireEvent.click(screen.getByTestId("form-cancel"));
    });

    // Timer should restart — advance and verify close
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    expect(window.close).toHaveBeenCalledTimes(1);
  });

  it("submits form with full payload through save pipeline", async () => {
    setupSuccessfulQuickSave();
    mockSendSaveMessage.mockResolvedValue({
      success: true,
      data: { type: "save_result", id: "bm_updated", path: "/tmp", status: "updated" },
    });

    await act(async () => {
      render(<Popup />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    // Fill in note
    await act(async () => {
      fireEvent.change(screen.getByTestId("note-input"), { target: { value: "my note" } });
    });

    // Submit
    await act(async () => {
      fireEvent.click(screen.getByTestId("form-submit"));
    });

    // Second call to sendSaveMessage is the form submission
    expect(mockSendSaveMessage).toHaveBeenCalledTimes(2);
    const [url, title, options] = mockSendSaveMessage.mock.calls[1];
    expect(url).toBe("https://example.com");
    expect(title).toBe("Example Page");
    expect(options).toBeDefined();
    expect(options!.tags).toEqual(["saved-tag"]);
    expect(options!.collection).toBe("reading");
    expect(options!.note).toBe("my note");
    expect(options!.selected_text).toBe("selected content");
  });

  it("persists last-used tags and collection on successful form submit", async () => {
    setupSuccessfulQuickSave();
    mockSendSaveMessage.mockResolvedValue({
      success: true,
      data: { type: "save_result", id: "bm_2", path: "/tmp", status: "updated" },
    });

    await act(async () => {
      render(<Popup />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("form-submit"));
    });

    expect(mockSaveLastUsedIntent).toHaveBeenCalledWith({
      tags: ["saved-tag"],
      collection: "reading",
    });
  });

  it("keeps form open with error on submission failure", async () => {
    setupSuccessfulQuickSave();

    await act(async () => {
      render(<Popup />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    // Make second save fail
    mockSendSaveMessage.mockResolvedValue({
      success: false,
      error: "Network error",
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("form-submit"));
    });

    // Form should still be visible with error
    expect(screen.getByTestId("form-mode")).toBeDefined();
    expect(screen.getByTestId("form-error")).toBeDefined();
    expect(screen.getByText("Network error")).toBeDefined();

    // Should NOT have persisted defaults
    expect(mockSaveLastUsedIntent).not.toHaveBeenCalled();
  });

  it("does not trigger a second quick-save when opening and closing the form", async () => {
    setupSuccessfulQuickSave();

    await act(async () => {
      render(<Popup />);
    });

    // Open form
    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    // Cancel
    await act(async () => {
      fireEvent.click(screen.getByTestId("form-cancel"));
    });

    // Only one save should have been dispatched (the initial quick save)
    expect(mockSendSaveMessage).toHaveBeenCalledTimes(1);
  });

  it("opens form from error state", async () => {
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test",
      id: 1,
    });
    mockSendSaveMessage.mockResolvedValue({
      success: false,
      error: "some error",
    });

    await act(async () => {
      render(<Popup />);
    });

    await act(async () => {
      fireEvent.click(screen.getByTestId("more-options-btn"));
    });

    expect(screen.getByTestId("form-mode")).toBeDefined();
  });
});
