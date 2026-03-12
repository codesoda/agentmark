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

  it("includes storage permission for intent defaults", () => {
    expect(manifest.permissions).toContain("storage");
  });

  it("includes scripting permission for selected text capture", () => {
    expect(manifest.permissions).toContain("scripting");
  });

  it("has popup entry point", () => {
    expect(manifest.action.default_popup).toBe("src/popup/index.html");
  });

  it("has side_panel registration", () => {
    expect(manifest.side_panel).toBeDefined();
    expect(manifest.side_panel.default_path).toBe("src/sidepanel/index.html");
  });

  it("includes sidePanel permission", () => {
    expect(manifest.permissions).toContain("sidePanel");
  });

  it("has open_side_panel command with keyboard shortcut", () => {
    expect(manifest.commands.open_side_panel).toBeDefined();
    expect(manifest.commands.open_side_panel.suggested_key.default).toBe("Ctrl+Shift+B");
    expect(manifest.commands.open_side_panel.suggested_key.mac).toBe("Command+Shift+B");
  });

  it("preserves _execute_action command for popup", () => {
    expect(manifest.commands._execute_action).toBeDefined();
    expect(manifest.commands._execute_action.suggested_key.default).toBe("Ctrl+Shift+S");
  });
});
