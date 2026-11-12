/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, cleanup, fireEvent, within } from "@testing-library/react";
import { resetChromeMock } from "../test/chrome-mock";
import InstallPrompt from "./InstallPrompt";
import { INSTALL_SCRIPT_URL, SETUP_GUIDE_URL, WEB_STORE_EXTENSION_ID } from "./constants";

const EXTENSION_ID = "abcdefghijklmnopabcdefghijklmnop";
const REGISTER = `agentmark install-extension --extension-id ${EXTENSION_ID}`;
const PREBUILT = `curl -sSL ${INSTALL_SCRIPT_URL} | bash -s -- --extension-id ${EXTENSION_ID}`;
const FROM_SOURCE = `curl -sSL ${INSTALL_SCRIPT_URL} | bash -s -- --from-source --extension-id ${EXTENSION_ID}`;

let chromeMock: ReturnType<typeof resetChromeMock>;

function setExtensionId(id: string | undefined) {
  (chrome.runtime as { id?: string }).id = id;
}

function setPlatform(os: string, arch: string) {
  chromeMock.runtime.getPlatformInfo.mockResolvedValue({ os, arch, nacl_arch: arch });
}

function mockClipboard(writeText: (text: string) => Promise<void>) {
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText: vi.fn(writeText) },
    configurable: true,
  });
  return navigator.clipboard.writeText as ReturnType<typeof vi.fn>;
}

async function renderMissing(onRetry = () => {}, detail?: string) {
  await act(async () => {
    render(<InstallPrompt problem="not_installed" onRetry={onRetry} detail={detail} />);
  });
}

function rowKeys(container: HTMLElement): string[] {
  return Array.from(container.querySelectorAll("[data-testid^='platform-']")).map((el) =>
    el.getAttribute("data-testid")!.replace("platform-", ""),
  );
}

describe("InstallPrompt: app not found (screen A)", () => {
  beforeEach(() => {
    chromeMock = resetChromeMock();
    setExtensionId(EXTENSION_ID);
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("explains what is missing, with the grey idle mark", async () => {
    setPlatform("mac", "arm64");
    await renderMissing();

    const prompt = screen.getByTestId("install-prompt");
    expect(prompt.getAttribute("data-problem")).toBe("not_installed");
    expect(screen.getByRole("heading", { name: "Finish setting up AgentMark" })).toBeDefined();
    expect(prompt.textContent).toContain("The extension saves pages through the AgentMark app on this computer.");
    expect(prompt.querySelector("svg[data-look='idle']")).not.toBeNull();
  });

  it.each([
    ["mac", "arm64", "mac_arm64", "Your computer · One Terminal command"],
    ["mac", "x86-64", "mac_x64", "Your computer · Build from source (needs Rust and Node.js)"],
    ["linux", "x86-64", "linux", "Your computer · Build from source (needs Rust and Node.js)"],
    ["win", "x86-64", "windows", "Your computer · Not supported yet"],
  ])("puts %s/%s (%s) first, set apart as Your computer", async (os, arch, key, subline) => {
    setPlatform(os, arch);
    await renderMissing();

    const container = screen.getByTestId("install-prompt");
    const keys = rowKeys(container);
    expect(keys[0]).toBe(key);
    expect(keys).toHaveLength(4);
    const detected = screen.getByTestId(`platform-${key}`);
    expect(detected.getAttribute("data-detected")).toBe("true");
    expect(detected.textContent).toContain(subline);
    // The rest sit under "Other platforms", without the detected one.
    const others = within(screen.getByTestId("other-platforms")).getAllByRole("button");
    expect(others).toHaveLength(3);
    expect(others.map((b) => b.getAttribute("data-testid"))).not.toContain(`platform-${key}`);
  });

  it("highlights nothing on an unmatched platform and lists all four as others", async () => {
    setPlatform("cros", "x86-64");
    await renderMissing();

    expect(screen.getByTestId("install-prompt").querySelector("[data-detected]")).toBeNull();
    expect(within(screen.getByTestId("other-platforms")).getAllByRole("button")).toHaveLength(4);
    expect(screen.queryByText(/Your computer/)).toBeNull();
  });

  it("highlights nothing when the platform cannot be read", async () => {
    chromeMock.runtime.getPlatformInfo.mockRejectedValue(new Error("no"));
    await renderMissing();

    expect(within(screen.getByTestId("other-platforms")).getAllByRole("button")).toHaveLength(4);
  });

  it("checks again from the footer", async () => {
    const onRetry = vi.fn();
    await renderMissing(onRetry);

    fireEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });
});

describe("InstallPrompt: install method (screen B)", () => {
  beforeEach(() => {
    chromeMock = resetChromeMock();
    setExtensionId(EXTENSION_ID);
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("shows one Terminal command on Apple Silicon, naming this extension's ID", async () => {
    setPlatform("mac", "arm64");
    await renderMissing();
    fireEvent.click(screen.getByTestId("platform-mac_arm64"));

    expect(screen.getByRole("heading", { name: "Install on Mac Apple Silicon" })).toBeDefined();
    expect(screen.getByTestId("install-method").getAttribute("data-method")).toBe("cli_command");
    expect(screen.getByText(/paste this/).textContent).toContain("Open Terminal and paste this");
    expect(screen.getByTestId("command-text").textContent).toBe(PREBUILT);
    expect(screen.getByText("Come back here. This updates by itself.")).toBeDefined();
    expect(screen.getByTestId("waiting-for-app").textContent).toContain("Waiting for the app…");
    // No download exists yet, so none is offered.
    expect(screen.queryByTestId("download-app")).toBeNull();
    expect(screen.queryByText(/Download AgentMark/)).toBeNull();
  });

  it("shows the short command when this copy has the store ID", async () => {
    setExtensionId(WEB_STORE_EXTENSION_ID);
    setPlatform("mac", "arm64");
    await renderMissing();
    fireEvent.click(screen.getByTestId("platform-mac_arm64"));
    expect(screen.getByTestId("command-text").textContent).toBe(`curl -sSL ${INSTALL_SCRIPT_URL} | bash`);
  });

  it.each([
    ["mac_x64", "Install on Mac Intel"],
    ["linux", "Install on Linux"],
  ])("shows honest build-from-source steps for %s", async (key, heading) => {
    setPlatform("mac", "arm64");
    await renderMissing();
    fireEvent.click(screen.getByTestId(`platform-${key}`));

    expect(screen.getByRole("heading", { name: heading })).toBeDefined();
    const method = screen.getByTestId("install-method");
    expect(method.getAttribute("data-method")).toBe("build_from_source");
    expect(method.textContent).toContain("There is no ready-made build");
    expect(screen.getByRole("link", { name: "Rust" }).getAttribute("href")).toBe("https://rustup.rs");
    expect(screen.getByRole("link", { name: "Node.js" })).toBeDefined();
    expect(screen.getByTestId("command-text").textContent).toBe(FROM_SOURCE);
  });

  it("says Windows is not supported yet, without a command or a wait", async () => {
    setPlatform("win", "x86-64");
    await renderMissing();
    fireEvent.click(screen.getByTestId("platform-windows"));

    expect(screen.getByTestId("unsupported").textContent).toBe("AgentMark does not run on Windows yet.");
    expect(screen.queryByTestId("command-text")).toBeNull();
    expect(screen.queryByTestId("waiting-for-app")).toBeNull();
  });

  it("goes back to all platforms", async () => {
    await renderMissing();
    fireEvent.click(screen.getByTestId("platform-linux"));
    fireEvent.click(screen.getByRole("button", { name: "All platforms" }));

    expect(screen.getByRole("heading", { name: "Finish setting up AgentMark" })).toBeDefined();
  });

  it("checks again from the waiting line", async () => {
    const onRetry = vi.fn();
    await renderMissing(onRetry);
    fireEvent.click(screen.getByTestId("platform-mac_arm64"));
    fireEvent.click(within(screen.getByTestId("waiting-for-app")).getByRole("button", { name: "Check again" }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("links to the setup guide in a new tab and shows the raw error collapsed", async () => {
    await renderMissing(() => {}, "Specified native messaging host not found.");
    fireEvent.click(screen.getByTestId("platform-mac_arm64"));

    const link = screen.getByRole("link", { name: "Setup guide" });
    expect(link.getAttribute("href")).toBe(SETUP_GUIDE_URL);
    expect(link.getAttribute("target")).toBe("_blank");
    expect(link.getAttribute("rel")).toBe("noreferrer");
    expect(screen.getByTestId("install-prompt-detail").textContent).toBe(
      "Specified native messaging host not found.",
    );
  });

  it("copies the command and briefly shows Copied", async () => {
    vi.useFakeTimers();
    const writeText = mockClipboard(() => Promise.resolve());
    await renderMissing();
    fireEvent.click(screen.getByTestId("platform-mac_arm64"));

    const button = screen.getByRole("button", { name: "Copy install command" });
    await act(async () => {
      fireEvent.click(button);
    });

    expect(writeText).toHaveBeenCalledWith(PREBUILT);
    expect(button.textContent).toBe("Copied");
    const live = button.parentElement!.querySelector("[aria-live]")!;
    expect(live.textContent).toBe("Copied");

    await act(async () => {
      vi.advanceTimersByTime(1500);
    });
    expect(button.textContent).toBe("Copy");
  });

  it("survives a rejected clipboard write", async () => {
    mockClipboard(() => Promise.reject(new Error("Document is not focused")));
    await renderMissing();
    fireEvent.click(screen.getByTestId("platform-mac_arm64"));

    const button = screen.getByRole("button", { name: "Copy install command" });
    await act(async () => {
      fireEvent.click(button);
    });

    expect(button.textContent).toBe("Copy");
    expect(button.parentElement!.querySelector("[aria-live]")!.textContent).toContain("Copy failed");
  });
});

describe("InstallPrompt: other problems", () => {
  beforeEach(() => {
    chromeMock = resetChromeMock();
    setExtensionId(EXTENSION_ID);
  });

  afterEach(cleanup);

  it("shows only the connect command when the host is registered for another id, and waits", () => {
    render(<InstallPrompt problem="forbidden" onRetry={() => {}} />);

    const prompt = screen.getByTestId("install-prompt");
    expect(prompt.textContent).toContain(REGISTER);
    expect(prompt.textContent).not.toContain("curl");
    expect(prompt.textContent).toContain("Waiting for the app…");
    expect(screen.getByRole("button", { name: "Check again" })).toBeDefined();
    expect(screen.queryByText("Finish setting up AgentMark")).toBeNull();
  });

  it("shows only reload guidance when the service worker is unreachable", () => {
    render(<InstallPrompt problem="worker_unreachable" onRetry={() => {}} />);

    const prompt = screen.getByTestId("install-prompt");
    expect(prompt.textContent).toContain("chrome://extensions");
    expect(prompt.textContent).not.toContain("agentmark install-extension");
    expect(prompt.textContent).not.toContain("curl");
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

  it("calls onRetry from Try again", () => {
    const onRetry = vi.fn();
    render(<InstallPrompt problem="host_crashed" onRetry={onRetry} />);

    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("copies the connect command with the real extension id", async () => {
    const writeText = mockClipboard(() => Promise.resolve());
    render(<InstallPrompt problem="forbidden" onRetry={() => {}} />);

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Copy connect command" }));
    });

    expect(writeText).toHaveBeenCalledWith(REGISTER);
  });
});
