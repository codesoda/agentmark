/**
 * @vitest-environment jsdom
 *
 * Screen B once the desktop app downloads exist (their URL constants are set).
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, cleanup, fireEvent } from "@testing-library/react";
import { resetChromeMock } from "../test/chrome-mock";

vi.mock("./constants", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./constants")>()),
  MAC_APP_DOWNLOAD_URL: "https://example.com/AgentMark-arm64.dmg",
  MAC_INTEL_APP_DOWNLOAD_URL: "https://example.com/AgentMark-x64.dmg",
  WEB_STORE_EXTENSION_ID: "ponmlkjihgfedcbaponmlkjihgfedcba",
}));

import AppMissing from "./AppMissing";

describe("AppMissing with app downloads", () => {
  beforeEach(() => {
    const chromeMock = resetChromeMock();
    chromeMock.runtime.getPlatformInfo.mockResolvedValue({ os: "mac", arch: "arm64", nacl_arch: "arm" });
  });

  afterEach(cleanup);

  it("says Download the app in the Mac rows", async () => {
    await act(async () => {
      render(<AppMissing onCheckAgain={() => {}} />);
    });
    expect(screen.getByTestId("platform-mac_arm64").textContent).toContain("Your computer · Download the app");
    expect(screen.getByTestId("platform-mac_x64").textContent).toContain("Download the app");
    expect(screen.getByTestId("platform-linux").textContent).toContain("Build from source");
  });

  it.each([
    ["mac_arm64", "https://example.com/AgentMark-arm64.dmg", "| bash"],
    ["mac_x64", "https://example.com/AgentMark-x64.dmg", "--from-source"],
  ])("offers the %s download first and the command second", async (key, url, commandPart) => {
    (chrome.runtime as { id?: string }).id = "ponmlkjihgfedcbaponmlkjihgfedcba";
    await act(async () => {
      render(<AppMissing onCheckAgain={() => {}} />);
    });
    fireEvent.click(screen.getByTestId(`platform-${key}`));

    const download = screen.getByRole("link", { name: "Download AgentMark for Mac" });
    expect(download.getAttribute("href")).toBe(url);
    expect(download.getAttribute("target")).toBe("_blank");
    const command = screen.getByTestId("command-text").textContent!;
    expect(command).toContain(commandPart);
    // The store copy uses the short command, without an extension ID.
    expect(command).not.toContain("--extension-id");
  });
});
