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
  queryPageHtml: vi.fn(),
  takePendingSave: vi.fn(),
  sendStatus: vi.fn(),
  sendGetSetupState: vi.fn(),
  sendSaveSettings: vi.fn(),
  sendChooseFolder: vi.fn(),
}));
// Setup and polling are covered in Popup.setup.test.tsx; here the app is ready.

import Popup from "./Popup";
import {
  queryActiveTab,
  sendSaveMessage,
  sendListCollectionsMessage,
  loadLastUsedIntent,
  saveLastUsedIntent,
  querySelectedText,
  queryPageHtml,
  takePendingSave,
  sendStatus,
} from "../shared/runtime";

const mockQueryActiveTab = vi.mocked(queryActiveTab);
const mockSendSaveMessage = vi.mocked(sendSaveMessage);
const mockSendListCollections = vi.mocked(sendListCollectionsMessage);
const mockLoadLastUsedIntent = vi.mocked(loadLastUsedIntent);
const mockSaveLastUsedIntent = vi.mocked(saveLastUsedIntent);
const mockQuerySelectedText = vi.mocked(querySelectedText);
const mockQueryPageHtml = vi.mocked(queryPageHtml);
const mockTakePendingSave = vi.mocked(takePendingSave);
const mockSendStatus = vi.mocked(sendStatus);

const READY: RuntimeResponse = {
  success: true,
  data: { type: "status_result", ok: true, version: "0.3.0", initialized: true },
};

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
  mockQueryPageHtml.mockResolvedValue(undefined);
  mockTakePendingSave.mockResolvedValue(undefined);
  mockSaveLastUsedIntent.mockResolvedValue(undefined);
  mockSendStatus.mockResolvedValue(READY);
}

describe("Popup", () => {
  beforeEach(() => {
    resetChromeMock();
    vi.useFakeTimers();
    vi.spyOn(window, "close").mockImplementation(() => {});
    (chrome.runtime as { id?: string }).id = "ext-id-123";
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

  it("saves the item an in-page button prepared instead of the active tab", async () => {
    mockTakePendingSave.mockResolvedValue({
      url: "https://www.linkedin.com/feed/update/urn:li:share:42/",
      title: "LinkedIn post by Ada Lovelace",
      page_html: "<html><body>the post</body></html>",
    });
    mockSendSaveMessage.mockResolvedValue({
      success: true,
      data: { type: "save_result", id: "bm_1", path: "/tmp", status: "created" },
    });

    await act(async () => {
      render(<Popup />);
    });

    expect(mockQueryActiveTab).not.toHaveBeenCalled();
    expect(mockQueryPageHtml).not.toHaveBeenCalled();
    expect(mockSendSaveMessage).toHaveBeenCalledWith(
      "https://www.linkedin.com/feed/update/urn:li:share:42/",
      "LinkedIn post by Ada Lovelace",
      { page_html: "<html><body>the post</body></html>" },
    );
    expect(screen.getByText("LinkedIn post by Ada Lovelace")).toBeDefined();
  });

  it("re-saves the prepared item, not the tab, when tags are added", async () => {
    mockTakePendingSave.mockResolvedValue({
      url: "https://www.linkedin.com/feed/update/urn:li:share:42/",
      title: "LinkedIn post by Ada Lovelace",
      page_html: "<html><body>the post</body></html>",
    });
    mockSendSaveMessage.mockResolvedValue({
      success: true,
      data: { type: "save_result", id: "bm_1", path: "/tmp", status: "created" },
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

    expect(mockQuerySelectedText).not.toHaveBeenCalled();
    const [url, , options] = mockSendSaveMessage.mock.calls[1];
    expect(url).toBe("https://www.linkedin.com/feed/update/urn:li:share:42/");
    expect(options!.page_html).toBe("<html><body>the post</body></html>");
  });

  it("sends the tab's HTML snapshot with the quick-save", async () => {
    mockQueryActiveTab.mockResolvedValue({
      url: "https://www.linkedin.com/in/someone/",
      title: "Someone | LinkedIn",
      id: 7,
    });
    mockQueryPageHtml.mockResolvedValue("<html><body>profile</body></html>");
    mockSendSaveMessage.mockResolvedValue({
      success: true,
      data: { type: "save_result", id: "bm_1", path: "/tmp", status: "created" },
    });

    await act(async () => {
      render(<Popup />);
    });

    expect(mockQueryPageHtml).toHaveBeenCalledWith(7, "https://www.linkedin.com/in/someone/");
    expect(mockSendSaveMessage).toHaveBeenCalledWith(
      "https://www.linkedin.com/in/someone/",
      "Someone | LinkedIn",
      { page_html: "<html><body>profile</body></html>" },
    );
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

    expect(mockSendSaveMessage).toHaveBeenCalledWith("https://example.com", "Test Title", {
      page_html: undefined,
    });
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

  it("shows the install screen and does not save when the app is not installed", async () => {
    setupSuccessfulQuickSave();
    mockSendStatus.mockResolvedValue({ success: false, error: "Specified native messaging host not found." });

    await act(async () => {
      render(<Popup />);
    });

    const prompt = screen.getByTestId("install-prompt");
    expect(prompt.getAttribute("data-problem")).toBe("not_installed");
    expect(screen.getByText("Finish setting up AgentMark")).toBeDefined();
    expect(screen.queryByText("Save Failed")).toBeNull();
    expect(screen.queryByTestId("error-state")).toBeNull();
    expect(mockSendSaveMessage).not.toHaveBeenCalled();
    // Nothing is snapshotted until there is an app to save to.
    expect(mockQueryPageHtml).not.toHaveBeenCalled();
  });

  it("shows the connect step when the host is registered for another extension", async () => {
    setupSuccessfulQuickSave();
    mockSendStatus.mockResolvedValue({
      success: false,
      error: "Access to the specified native messaging host is forbidden.",
    });

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("install-prompt").getAttribute("data-problem")).toBe("forbidden");
    expect(screen.getByTestId("install-prompt").textContent).toContain(
      "agentmark install-extension --extension-id ext-id-123",
    );
    expect(screen.queryByText("Save Failed")).toBeNull();
  });

  it("shows reload guidance when the status probe finds the host broken", async () => {
    setupSuccessfulQuickSave();
    mockSendStatus.mockResolvedValue({ success: false, error: "Native host request timed out" });

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("install-prompt").getAttribute("data-problem")).toBe("host_crashed");
    expect(mockSendSaveMessage).not.toHaveBeenCalled();
  });

  it("keeps Save Failed with reload guidance when the native host exits", async () => {
    setupSuccessfulQuickSave();
    mockSendSaveMessage.mockResolvedValue({
      success: false,
      error: "Native host has exited.",
    });

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("error-state")).toBeDefined();
    expect(screen.getByText("Save Failed")).toBeDefined();
    expect(screen.getByTestId("install-prompt").getAttribute("data-problem")).toBe("host_crashed");
    expect(screen.getByText("Couldn't reach AgentMark")).toBeDefined();
  });

  it("re-sends the save from Try again after a crash, reusing the page and its snapshot", async () => {
    setupSuccessfulQuickSave();
    mockQueryPageHtml.mockResolvedValue("<html>tab</html>");
    mockSendSaveMessage.mockResolvedValueOnce({ success: false, error: "Native host has exited." });

    await act(async () => {
      render(<Popup />);
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    });

    expect(mockSendSaveMessage).toHaveBeenCalledTimes(2);
    expect(mockSendSaveMessage.mock.calls[1]).toEqual([
      "https://example.com",
      "Example Page",
      { page_html: "<html>tab</html>" },
    ]);
    expect(mockQueryActiveTab).toHaveBeenCalledTimes(1);
    expect(mockQueryPageHtml).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("success-state")).toBeDefined();
  });

  it("moves to the install screen when the save finds the app gone, and saves once it is back", async () => {
    setupSuccessfulQuickSave();
    mockQueryPageHtml.mockResolvedValue("<html>tab</html>");
    mockSendSaveMessage.mockResolvedValueOnce({
      success: false,
      error: "Specified native messaging host not found.",
    });

    await act(async () => {
      render(<Popup />);
    });
    expect(screen.getByTestId("install-prompt").getAttribute("data-problem")).toBe("not_installed");

    // "Check again" probes; the app is back, so the same page is saved again.
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Check again" }));
    });

    expect(mockSendSaveMessage).toHaveBeenCalledTimes(2);
    expect(mockSendSaveMessage.mock.calls[1]).toEqual([
      "https://example.com",
      "Example Page",
      { page_html: "<html>tab</html>" },
    ]);
    expect(mockQueryActiveTab).toHaveBeenCalledTimes(1);
    expect(mockQueryPageHtml).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("success-state")).toBeDefined();
  });

  it("keeps a pending in-page save through the install screen and saves that post", async () => {
    const post = {
      url: "https://www.linkedin.com/feed/update/urn:li:share:42/",
      title: "LinkedIn post by Ada Lovelace",
      page_html: "<html><body>the post</body></html>",
    };
    // The service worker hands the pending save over only once.
    mockTakePendingSave.mockResolvedValueOnce(post).mockResolvedValue(undefined);
    setupSuccessfulQuickSave();
    mockSendStatus.mockResolvedValueOnce({ success: false, error: "Specified native messaging host not found." });

    await act(async () => {
      render(<Popup />);
    });
    expect(mockSendSaveMessage).not.toHaveBeenCalled();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Check again" }));
    });

    expect(mockTakePendingSave).toHaveBeenCalledTimes(1);
    expect(mockQueryActiveTab).not.toHaveBeenCalled();
    expect(mockSendSaveMessage.mock.calls).toEqual([[post.url, post.title, { page_html: post.page_html }]]);
    expect(screen.getByTestId("success-state")).toBeDefined();
    expect(screen.getByText(post.title)).toBeDefined();
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
    (chrome.runtime as { id?: string }).id = "ext-id-123";

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("error-state")).toBeDefined();
    expect(screen.getByText("Save Failed")).toBeDefined();
    expect(screen.getByText("Some other error")).toBeDefined();
    expect(screen.queryByTestId("install-prompt")).toBeNull();
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
