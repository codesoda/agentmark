import { describe, it, expect } from "vitest";
import { existsSync, readFileSync } from "fs";
import { resolve } from "path";
import {
  cliCommand,
  detectPlatform,
  downloadUrl,
  fromSourceCommand,
  installCommandFor,
  installMethod,
  METHOD_SUBLINE,
  orderPlatforms,
} from "./platforms";
import * as constants from "./constants";

const SCRIPT = "https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh";
const NO_DOWNLOADS = { mac_arm64: "", mac_x64: "" };
const ALL_DOWNLOADS = { mac_arm64: "https://example.com/arm.dmg", mac_x64: "https://example.com/intel.dmg" };

describe("platform detection and ordering (contract §7.3)", () => {
  it.each([
    [{ os: "mac", arch: "arm64" }, "mac_arm64"],
    [{ os: "mac", arch: "x86-64" }, "mac_x64"],
    // Rosetta-translated Chrome on Apple Silicon reports x86-64 and is shown as Mac Intel.
    [{ os: "mac", arch: "x86-32" }, "mac_x64"],
    [{ os: "linux", arch: "x86-64" }, "linux"],
    [{ os: "linux", arch: "arm64" }, "linux"],
    [{ os: "win", arch: "x86-64" }, "windows"],
  ] as const)("detects %j as %s and lists it first", (info, key) => {
    expect(detectPlatform(info)?.key).toBe(key);
    const { detected, others } = orderPlatforms(info);
    expect(detected?.key).toBe(key);
    expect(others.map((p) => p.key)).not.toContain(key);
    expect(others).toHaveLength(3);
  });

  it("keeps the other platforms in table order", () => {
    expect(orderPlatforms({ os: "linux", arch: "x86-64" }).others.map((p) => p.key)).toEqual([
      "mac_arm64",
      "mac_x64",
      "windows",
    ]);
  });

  it.each([{ os: "cros", arch: "x86-64" }, { os: "openbsd", arch: "x86-64" }, undefined])(
    "highlights nothing for an unmatched platform %j",
    (info) => {
      const { detected, others } = orderPlatforms(info);
      expect(detected).toBeUndefined();
      expect(others.map((p) => p.key)).toEqual(["mac_arm64", "mac_x64", "linux", "windows"]);
    },
  );
});

describe("install methods", () => {
  it("uses what exists today: one command on Apple Silicon, source elsewhere, nothing on Windows", () => {
    expect(installMethod("mac_arm64", NO_DOWNLOADS)).toBe("cli_command");
    expect(installMethod("mac_x64", NO_DOWNLOADS)).toBe("build_from_source");
    expect(installMethod("linux", NO_DOWNLOADS)).toBe("build_from_source");
    expect(installMethod("windows", NO_DOWNLOADS)).toBe("unsupported");
  });

  it("offers the app download only when its URL is set", () => {
    expect(installMethod("mac_arm64", ALL_DOWNLOADS)).toBe("app_download");
    expect(installMethod("mac_x64", ALL_DOWNLOADS)).toBe("app_download");
    expect(installMethod("linux", ALL_DOWNLOADS)).toBe("build_from_source");
    expect(downloadUrl("mac_x64", ALL_DOWNLOADS)).toBe("https://example.com/intel.dmg");
    expect(downloadUrl("linux", ALL_DOWNLOADS)).toBe("");
  });

  it("hides downloads with the shipped (empty) constants", () => {
    expect(constants.MAC_APP_DOWNLOAD_URL).toBe("");
    expect(installMethod("mac_arm64")).toBe("cli_command");
    expect(installMethod("mac_x64")).toBe("build_from_source");
  });

  it("describes each method in one sub-line", () => {
    expect(METHOD_SUBLINE).toEqual({
      app_download: "Download the app",
      cli_command: "One Terminal command",
      build_from_source: "Build from source (needs Rust and Node.js)",
      unsupported: "Not supported yet",
    });
  });
});

describe("install commands (contract §7.2)", () => {
  const DEV_ID = "abcdefghijklmnopabcdefghijklmnop";
  const STORE_ID = "ponmlkjihgfedcbaponmlkjihgfedcba";

  it("names this extension's ID while it is not the store copy", () => {
    expect(cliCommand(DEV_ID, "")).toBe(`curl -sSL ${SCRIPT} | bash -s -- --extension-id ${DEV_ID}`);
    expect(cliCommand(DEV_ID, STORE_ID)).toBe(`curl -sSL ${SCRIPT} | bash -s -- --extension-id ${DEV_ID}`);
    expect(fromSourceCommand(DEV_ID, "")).toBe(
      `curl -sSL ${SCRIPT} | bash -s -- --from-source --extension-id ${DEV_ID}`,
    );
  });

  it("uses the short command for the store copy", () => {
    expect(cliCommand(STORE_ID, STORE_ID)).toBe(`curl -sSL ${SCRIPT} | bash`);
    expect(fromSourceCommand(STORE_ID, STORE_ID)).toBe(`curl -sSL ${SCRIPT} | bash -s -- --from-source`);
  });

  it("omits the ID when there is none", () => {
    expect(cliCommand(undefined, "")).toBe(`curl -sSL ${SCRIPT} | bash`);
  });

  it("picks the update command for the detected platform", () => {
    expect(installCommandFor("mac_arm64", DEV_ID, "")).toBe(cliCommand(DEV_ID, ""));
    expect(installCommandFor("linux", DEV_ID, "")).toBe(fromSourceCommand(DEV_ID, ""));
    expect(installCommandFor(undefined, DEV_ID, "")).toBe(fromSourceCommand(DEV_ID, ""));
    expect(installCommandFor("windows", DEV_ID, "")).toBeUndefined();
  });
});

describe("constants (contract §7.1)", () => {
  it("has the working site and installer addresses", () => {
    expect(constants.SITE_URL).toBe("https://agentmark.app");
    expect(constants.SETUP_GUIDE_URL).toBe("https://agentmark.app/#install");
    expect(constants.INSTALL_SCRIPT_URL).toBe(SCRIPT);
  });

  it("has WEB_STORE_EXTENSION_ID set to a valid Chrome ID", () => {
    expect(constants.WEB_STORE_EXTENSION_ID).toMatch(/^[a-p]{32}$/);
  });

  it("gives the store copy the short command and any other copy its ID", () => {
    const store = constants.WEB_STORE_EXTENSION_ID;
    expect(cliCommand(store)).toBe(`curl -sSL ${SCRIPT} | bash`);
    expect(installCommandFor("linux", store)).toBe(`curl -sSL ${SCRIPT} | bash -s -- --from-source`);
    const other = "abcdefghijklmnopabcdefghijklmnop";
    expect(cliCommand(other)).toBe(`curl -sSL ${SCRIPT} | bash -s -- --extension-id ${other}`);
  });

  const rustFile = resolve(__dirname, "../../../cli/src/setup/extension_ids.rs");
  it.skipIf(!existsSync(rustFile))("matches the CLI's WEB_STORE_EXTENSION_ID", () => {
    const rust = readFileSync(rustFile, "utf-8");
    const match = rust.match(/pub const WEB_STORE_EXTENSION_ID\s*:\s*&str\s*=\s*"([^"]*)"/);
    expect(match, "WEB_STORE_EXTENSION_ID not found in extension_ids.rs").not.toBeNull();
    expect(match![1]).toBe(constants.WEB_STORE_EXTENSION_ID);
  });
});
