/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, cleanup, fireEvent, within } from "@testing-library/react";
import { resetChromeMock } from "../test/chrome-mock";
import InstallPrompt from "./InstallPrompt";
import { INSTALL_COMMAND, SETUP_GUIDE_URL } from "./connection";

const EXTENSION_ID = "abcdefghijklmnopabcdefghijklmnop";
const REGISTER = `agentmark install-extension --extension-id ${EXTENSION_ID}`;

function setExtensionId(id: string | undefined) {
  (chrome.runtime as { id?: string }).id = id;
}

function mockClipboard(writeText: (text: string) => Promise<void>) {
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText: vi.fn(writeText) },
    configurable: true,
  });
  return navigator.clipboard.writeText as ReturnType<typeof vi.fn>;
}

describe("InstallPrompt", () => {
  beforeEach(() => {
    resetChromeMock();
    setExtensionId(EXTENSION_ID);
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("shows the install, connect and reload steps when the app is not installed", () => {
    render(<InstallPrompt problem="not_installed" onRetry={() => {}} />);

    const prompt = screen.getByTestId("install-prompt");
    expect(prompt.getAttribute("data-problem")).toBe("not_installed");
    expect(screen.getByRole("heading", { name: "Install the AgentMark app" })).toBeDefined();
    expect(prompt.textContent).toContain("it was not found");
    const steps = within(prompt).getAllByRole("listitem");
    expect(steps).toHaveLength(3);
    expect(steps[0].textContent).toContain(INSTALL_COMMAND);
    expect(steps[1].textContent).toContain(REGISTER);
    expect(steps[2].textContent).toContain("chrome://extensions");
    expect(screen.getAllByTestId("copy-command")).toHaveLength(2);
  });

  it("shows only the connect command when the host is registered for another id", () => {
    render(<InstallPrompt problem="forbidden" onRetry={() => {}} />);

    const prompt = screen.getByTestId("install-prompt");
    expect(prompt.textContent).toContain(REGISTER);
    expect(prompt.textContent).not.toContain(INSTALL_COMMAND);
    expect(screen.queryByText("Install the AgentMark app")).toBeNull();
  });

  it("shows only reload guidance when the service worker is unreachable", () => {
    render(<InstallPrompt problem="worker_unreachable" onRetry={() => {}} />);

    const prompt = screen.getByTestId("install-prompt");
    expect(prompt.textContent).toContain("chrome://extensions");
    expect(prompt.textContent).not.toContain("agentmark install-extension");
    expect(prompt.textContent).not.toContain(INSTALL_COMMAND);
    expect(screen.queryByTestId("copy-command")).toBeNull();
  });

  it("offers reload and the connect command after the host exits", () => {
    render(<InstallPrompt problem="host_crashed" onRetry={() => {}} />);

    const steps = within(screen.getByTestId("install-prompt")).getAllByRole("listitem");
    expect(steps[0].textContent).toContain("chrome://extensions");
    expect(steps[1].textContent).toContain(REGISTER);
    expect(screen.getByText("Couldn't reach AgentMark")).toBeDefined();
  });

  it("falls back to a placeholder when the extension id is unavailable", () => {
    setExtensionId(undefined);
    render(<InstallPrompt problem="forbidden" onRetry={() => {}} />);

    expect(screen.getByTestId("install-prompt").textContent).toContain(
      "agentmark install-extension --extension-id YOUR_EXTENSION_ID",
    );
  });

  it("links to the setup guide in a new tab", () => {
    render(<InstallPrompt problem="not_installed" onRetry={() => {}} />);

    const link = screen.getByRole("link", { name: "Setup guide" });
    expect(link.getAttribute("href")).toBe(SETUP_GUIDE_URL);
    expect(link.getAttribute("target")).toBe("_blank");
    expect(link.getAttribute("rel")).toBe("noreferrer");
  });

  it("calls onRetry from Try again", () => {
    const onRetry = vi.fn();
    render(<InstallPrompt problem="not_installed" onRetry={onRetry} />);

    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("shows the raw error collapsed when given", () => {
    render(<InstallPrompt problem="not_installed" onRetry={() => {}} detail="Specified native messaging host not found." />);

    expect(screen.getByTestId("install-prompt-detail").textContent).toBe(
      "Specified native messaging host not found.",
    );
  });

  it("copies a command and briefly shows Copied", async () => {
    vi.useFakeTimers();
    const writeText = mockClipboard(() => Promise.resolve());
    render(<InstallPrompt problem="not_installed" onRetry={() => {}} />);

    const button = screen.getByRole("button", { name: "Copy install command" });
    await act(async () => {
      fireEvent.click(button);
    });

    expect(writeText).toHaveBeenCalledWith(INSTALL_COMMAND);
    expect(button.textContent).toBe("Copied");
    const live = button.parentElement!.querySelector("[aria-live]")!;
    expect(live.textContent).toBe("Copied");

    await act(async () => {
      vi.advanceTimersByTime(1500);
    });
    expect(button.textContent).toBe("Copy");
  });

  it("copies the connect command with the real extension id", async () => {
    const writeText = mockClipboard(() => Promise.resolve());
    render(<InstallPrompt problem="not_installed" onRetry={() => {}} />);

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy connect command" }));
    });

    expect(writeText).toHaveBeenCalledWith(REGISTER);
  });

  it("survives a rejected clipboard write", async () => {
    mockClipboard(() => Promise.reject(new Error("Document is not focused")));
    render(<InstallPrompt problem="not_installed" onRetry={() => {}} />);

    const button = screen.getByRole("button", { name: "Copy install command" });
    await act(async () => {
      fireEvent.click(button);
    });

    expect(button.textContent).toBe("Copy");
    expect(button.parentElement!.querySelector("[aria-live]")!.textContent).toContain("Copy failed");
  });
});
