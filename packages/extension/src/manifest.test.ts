import { describe, it, expect } from "vitest";
import { createHash } from "crypto";
import { existsSync, readFileSync } from "fs";
import { resolve } from "path";
import { WEB_STORE_EXTENSION_ID } from "./shared/constants";

const manifest = JSON.parse(
  readFileSync(resolve(__dirname, "../manifest.json"), "utf-8"),
);

const packageJson = JSON.parse(
  readFileSync(resolve(__dirname, "../package.json"), "utf-8"),
);

/** Chrome's rule: SHA-256 of the public key (DER), first 16 bytes, hex digits 0-f as letters a-p. */
function idFromKey(base64Key: string): string {
  const hex = createHash("sha256").update(Buffer.from(base64Key, "base64")).digest("hex").slice(0, 32);
  return [...hex].map((c) => String.fromCharCode(97 + parseInt(c, 16))).join("");
}

describe("manifest.json", () => {
  it("carries the store item's public key, so an unpacked copy has the store ID", () => {
    expect(typeof manifest.key).toBe("string");
    expect(manifest.key).toMatch(/^[A-Za-z0-9+/]+={0,2}$/);
    expect(idFromKey(manifest.key)).toBe(WEB_STORE_EXTENSION_ID);
  });

  it("strips the key from the store zip", () => {
    const script = readFileSync(resolve(__dirname, "../scripts/build-store-zip.sh"), "utf-8");
    expect(script).toContain("delete manifest.key;");
  });

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

  it("asks for tabs only on request, so install does not warn about browsing history", () => {
    expect(manifest.permissions).not.toContain("tabs");
    expect(manifest.optional_permissions).toEqual(["tabs"]);
    expect(manifest.host_permissions).toBeUndefined();
    expect(manifest.optional_host_permissions).toBeUndefined();
  });

  it("requests exactly the permissions the code uses", () => {
    expect([...manifest.permissions].sort()).toEqual([
      "activeTab",
      "contextMenus",
      "nativeMessaging",
      "notifications",
      "scripting",
      "sidePanel",
      "storage",
    ]);
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
      // reddit.com, new.reddit.com and sh.reddit.com redirect to www.reddit.com.
      "https://www.reddit.com/*",
      "https://old.reddit.com/*",
    ]);
    expect(isolated?.js).toEqual(["content/content-posts.js"]);
  });
});
