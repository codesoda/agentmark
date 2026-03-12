import { describe, it, expect } from "vitest";
import { readFileSync } from "fs";
import { resolve } from "path";

const manifest = JSON.parse(
  readFileSync(resolve(__dirname, "../manifest.json"), "utf-8"),
);

describe("manifest.json", () => {
  it("has _execute_action command with keyboard shortcuts", () => {
    expect(manifest.commands).toBeDefined();
    expect(manifest.commands._execute_action).toBeDefined();
    expect(manifest.commands._execute_action.suggested_key.default).toBe("Ctrl+Shift+S");
    expect(manifest.commands._execute_action.suggested_key.mac).toBe("Command+Shift+S");
  });

  it("includes notifications permission", () => {
    expect(manifest.permissions).toContain("notifications");
  });

  it("includes required existing permissions", () => {
    expect(manifest.permissions).toContain("activeTab");
    expect(manifest.permissions).toContain("contextMenus");
    expect(manifest.permissions).toContain("nativeMessaging");
  });

  it("has popup entry point", () => {
    expect(manifest.action.default_popup).toBe("src/popup/index.html");
  });
});
