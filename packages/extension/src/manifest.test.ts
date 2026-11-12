import { describe, it, expect } from "vitest";
import { existsSync, readFileSync } from "fs";
import { resolve } from "path";

const manifest = JSON.parse(
  readFileSync(resolve(__dirname, "../manifest.json"), "utf-8"),
);

const packageJson = JSON.parse(
  readFileSync(resolve(__dirname, "../package.json"), "utf-8"),
);

describe("manifest.json", () => {
  it("has the same version as package.json", () => {
    expect(manifest.version).toMatch(/^\d+\.\d+\.\d+$/);
    expect(manifest.version).toBe(packageJson.version);
  });

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

  it("includes tabs permission so the toolbar icon can read tab URLs outside a user gesture", () => {
    expect(manifest.permissions).toContain("tabs");
  });

  it("starts the toolbar on the idle look and lists the extension with the brand icon", () => {
    expect(manifest.action.default_icon).toEqual({
      16: "icons/toolbar-idle-16.png",
      32: "icons/toolbar-idle-32.png",
      48: "icons/toolbar-idle-48.png",
    });
    expect(manifest.icons).toEqual({
      16: "icons/icon-16.png",
      32: "icons/icon-32.png",
      48: "icons/icon-48.png",
      128: "icons/icon-128.png",
    });
  });

  it("ships every icon it references", () => {
    const paths = [
      ...Object.values(manifest.icons as Record<string, string>),
      ...Object.values(manifest.action.default_icon as Record<string, string>),
    ];
    for (const path of paths) {
      expect(existsSync(resolve(__dirname, "../public", path)), path).toBe(true);
    }
  });

  it("ships every toolbar look the service worker can paint", () => {
    for (const look of ["idle", "saving", "saved-fresh", "saved", "error"]) {
      for (const size of [16, 32, 48]) {
        const path = `icons/toolbar-${look}-${size}.png`;
        expect(existsSync(resolve(__dirname, "../public", path)), path).toBe(true);
      }
    }
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

  it("injects the post save button only on the supported sites", () => {
    const scripts = manifest.content_scripts as Array<{
      matches: string[];
      js: string[];
      world?: string;
    }>;
    expect(scripts).toHaveLength(2);
    const main = scripts.find((s) => s.world === "MAIN");
    const isolated = scripts.find((s) => s.world === undefined);
    // The page-world helper reads LinkedIn's data and must not run anywhere else.
    expect(main?.matches).toEqual(["https://www.linkedin.com/*"]);
    expect(main?.js).toEqual(["content/content-linkedin-main.js"]);
    expect(isolated?.matches).toEqual([
      "https://www.linkedin.com/*",
      "https://x.com/*",
      "https://twitter.com/*",
      "https://news.ycombinator.com/*",
    ]);
    expect(isolated?.js).toEqual(["content/content-posts.js"]);
  });
});
