/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, cleanup } from "@testing-library/react";
import { resetChromeMock } from "../test/chrome-mock";
import type { RuntimeResponse } from "../shared/types";

// Mock the runtime module so we can control responses
vi.mock("../shared/runtime", () => ({
  queryActiveTab: vi.fn(),
  sendSaveMessage: vi.fn(),
  isConnectionError: vi.fn(),
}));

import Popup from "./Popup";
import { queryActiveTab, sendSaveMessage, isConnectionError } from "../shared/runtime";

const mockQueryActiveTab = vi.mocked(queryActiveTab);
const mockSendSaveMessage = vi.mocked(sendSaveMessage);
const mockIsConnectionError = vi.mocked(isConnectionError);

describe("Popup", () => {
  beforeEach(() => {
    resetChromeMock();
    vi.useFakeTimers();
    vi.spyOn(window, "close").mockImplementation(() => {});
    mockIsConnectionError.mockReturnValue(false);
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("renders saving state immediately", async () => {
    mockQueryActiveTab.mockImplementation(() => new Promise(() => {})); // never resolves

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByTestId("saving-state")).toBeDefined();
    expect(screen.getByText("Saving...")).toBeDefined();
  });

  it("shows page title in saving state", async () => {
    let resolveTab!: (value: { url: string; title: string; favIconUrl?: string }) => void;
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
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Example Page",
      favIconUrl: "https://example.com/favicon.ico",
    });
    const successResponse: RuntimeResponse = {
      success: true,
      data: { type: "save_result", id: "bm_abc123", path: "/tmp/path", status: "created" },
    };
    mockSendSaveMessage.mockResolvedValue(successResponse);

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
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test",
    });
    mockSendSaveMessage.mockResolvedValue({
      success: true,
      data: { type: "save_result", id: "bm_1", path: "/tmp", status: "created" },
    });

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
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test",
    });
    mockSendSaveMessage.mockResolvedValue({
      success: true,
      data: { type: "save_result", id: "bm_1", path: "/tmp", status: "created" },
    });

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
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test",
    });
    mockSendSaveMessage.mockResolvedValue({
      success: true,
      data: { type: "save_result", id: "bm_1", path: "/tmp", status: "created" },
    });

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
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test",
    });
    mockSendSaveMessage.mockResolvedValue({
      success: true,
      data: { type: "save_result", id: "bm_1", path: "/tmp", status: "created" },
    });

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByText("More options")).toBeDefined();
  });

  it("renders More options button on error", async () => {
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test",
    });
    mockSendSaveMessage.mockResolvedValue({
      success: false,
      error: "some error",
    });

    await act(async () => {
      render(<Popup />);
    });

    expect(screen.getByText("More options")).toBeDefined();
  });

  it("handles updated save status", async () => {
    mockQueryActiveTab.mockResolvedValue({
      url: "https://example.com",
      title: "Test",
    });
    mockSendSaveMessage.mockResolvedValue({
      success: true,
      data: { type: "save_result", id: "bm_1", path: "/tmp", status: "updated" },
    });

    await act(async () => {
      render(<Popup />);
    });

    // Should still show success state even for "updated" status
    expect(screen.getByTestId("success-state")).toBeDefined();
    expect(screen.getByText("Saved!")).toBeDefined();
  });
});
