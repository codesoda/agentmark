// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, cleanup, fireEvent } from "@testing-library/react";
import { resetChromeMock } from "../test/chrome-mock";
import UpdateRequired from "./UpdateRequired";

const ID = "abcdefghijklmnopabcdefghijklmnop";
const SCRIPT = "https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh";

describe("UpdateRequired", () => {
  let chromeMock: ReturnType<typeof resetChromeMock>;

  beforeEach(() => {
    chromeMock = resetChromeMock();
    (chrome.runtime as { id?: string }).id = ID;
  });

  afterEach(cleanup);

  it.each([
    ["mac", "arm64", `curl -sSL ${SCRIPT} | bash -s -- --extension-id ${ID}`],
    ["linux", "x86-64", `curl -sSL ${SCRIPT} | bash -s -- --from-source --extension-id ${ID}`],
  ])("gives the update command for %s/%s", async (os, arch, command) => {
    chromeMock.runtime.getPlatformInfo.mockResolvedValue({ os, arch, nacl_arch: arch });
    await act(async () => {
      render(<UpdateRequired onCheckAgain={() => {}} version="0.2.6" />);
    });
    expect(screen.getByText("This AgentMark app (0.2.6) is too old for this extension. Update it:")).toBeDefined();
    expect(screen.getByTestId("command-text").textContent).toBe(command);
  });

  it("checks again", async () => {
    const onCheckAgain = vi.fn();
    await act(async () => {
      render(<UpdateRequired onCheckAgain={onCheckAgain} />);
    });
    fireEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(onCheckAgain).toHaveBeenCalledTimes(1);
  });
});
