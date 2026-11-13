// @vitest-environment jsdom
import { describe, it, expect, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import CopyCommand from "./CopyCommand";
import { commandPieces } from "./command-pieces";

const COMMAND =
  "curl -fsSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash -s -- --extension-id abcdefghijklmnop";

describe("CopyCommand", () => {
  afterEach(cleanup);

  it("breaks only after slashes and at spaces, keeping host names whole", () => {
    const pieces = commandPieces(COMMAND);
    expect(pieces.join("")).toBe(COMMAND);
    expect(pieces).toContain("raw.githubusercontent.com/");
    expect(pieces).toContain("https:/" + "/");
    expect(pieces).toContain("--extension-id ");
    expect(pieces.every((piece) => /^[^/\s]*(?:\/+|\s+)?$/.test(piece))).toBe(true);
  });

  it("keeps a command without break points whole", () => {
    expect(commandPieces("agentmark")).toEqual(["agentmark"]);
    expect(commandPieces("")).toEqual([]);
  });

  it("shows the exact command, with break hints instead of mid-word breaks", () => {
    render(<CopyCommand command={COMMAND} label="install command" />);
    const code = screen.getByTestId("command-text");

    expect(code.textContent).toBe(COMMAND);
    expect(code.querySelectorAll("wbr").length).toBe(commandPieces(COMMAND).length - 1);
    expect(code.className).not.toContain("break-all");
    // A piece wider than the block may still wrap, so the popup never scrolls sideways.
    expect(code.className).toContain("[overflow-wrap:anywhere]");
    expect(code.className).toContain("min-w-0");
  });
});
