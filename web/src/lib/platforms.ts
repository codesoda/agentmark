/**
 * What really exists to install AgentMark on each platform, and the commands
 * to do it. Canonical table: docs/setup-contract.md §7.3. This is the
 * website's copy of packages/extension/src/shared/platforms.ts: keep the
 * names, sub-lines and rules the same, and change both together.
 *
 * The website has no extension ID, so its commands never carry
 * `--extension-id` (contract §7.2).
 */

import {
  INSTALL_SCRIPT_URL,
  MAC_APP_DOWNLOAD_URL,
  MAC_INTEL_APP_DOWNLOAD_URL,
  WEB_STORE_URL,
} from "./site";

export type PlatformKey = "mac_arm64" | "mac_x64" | "linux" | "windows";

export type InstallMethod = "app_download" | "cli_command" | "build_from_source" | "unsupported";

/** The subset of `chrome.runtime.PlatformInfo` used for detection. */
export interface PlatformInfoLike {
  os: string;
  arch: string;
}

export interface Platform {
  key: PlatformKey;
  name: string;
  matches(info: PlatformInfoLike): boolean;
}

/** In table order; "Other platforms" keeps this order. */
export const PLATFORMS: readonly Platform[] = [
  { key: "mac_arm64", name: "Mac Apple Silicon", matches: (i) => i.os === "mac" && i.arch === "arm64" },
  { key: "mac_x64", name: "Mac Intel", matches: (i) => i.os === "mac" && i.arch !== "arm64" },
  { key: "linux", name: "Linux", matches: (i) => i.os === "linux" },
  { key: "windows", name: "Windows", matches: (i) => i.os === "win" },
];

export function platformByKey(key: PlatformKey): Platform {
  return PLATFORMS.find((p) => p.key === key)!;
}

/** The platform this browser runs on, or undefined when no row matches (e.g. ChromeOS). */
export function detectPlatform(info: PlatformInfoLike | undefined): Platform | undefined {
  if (!info) return undefined;
  return PLATFORMS.find((p) => p.matches(info));
}

/** The detected platform first (if any), then the rest in table order. */
export function orderPlatforms(info: PlatformInfoLike | undefined): {
  detected: Platform | undefined;
  others: Platform[];
} {
  const detected = detectPlatform(info);
  return { detected, others: PLATFORMS.filter((p) => p !== detected) };
}

/* ------------------------------------------------------------ Links */

/**
 * Set `NEXT_PUBLIC_AGENTMARK_PREVIEW_INSTALL_URLS=1` at build or dev time to
 * see the page as it will look once the store listing and app downloads
 * exist. The preview addresses are placeholders and must never ship.
 */
export const PREVIEW_INSTALL_URLS = process.env.NEXT_PUBLIC_AGENTMARK_PREVIEW_INSTALL_URLS === "1";

export interface InstallLinks {
  webStore: string;
  macApp: string;
  macIntelApp: string;
}

export const INSTALL_LINKS: InstallLinks = PREVIEW_INSTALL_URLS
  ? {
      webStore: "https://chromewebstore.google.com/detail/agentmark-preview",
      macApp: "https://example.com/agentmark-preview/AgentMark-arm64.dmg",
      macIntelApp: "https://example.com/agentmark-preview/AgentMark-x64.dmg",
    }
  : { webStore: WEB_STORE_URL, macApp: MAC_APP_DOWNLOAD_URL, macIntelApp: MAC_INTEL_APP_DOWNLOAD_URL };

/** Download URLs, injectable for tests. Empty = no download exists. */
export interface DownloadUrls {
  mac_arm64: string;
  mac_x64: string;
}

export const DOWNLOAD_URLS: DownloadUrls = {
  mac_arm64: INSTALL_LINKS.macApp,
  mac_x64: INSTALL_LINKS.macIntelApp,
};

/** How to install on `key`. A download is offered only when its URL is set. */
export function installMethod(key: PlatformKey, urls: DownloadUrls = DOWNLOAD_URLS): InstallMethod {
  switch (key) {
    case "mac_arm64":
      return urls.mac_arm64 ? "app_download" : "cli_command";
    case "mac_x64":
      return urls.mac_x64 ? "app_download" : "build_from_source";
    case "linux":
      return "build_from_source";
    case "windows":
      return "unsupported";
  }
}

/** The download URL for `key`, or "" when there is none. */
export function downloadUrl(key: PlatformKey, urls: DownloadUrls = DOWNLOAD_URLS): string {
  if (key === "mac_arm64") return urls.mac_arm64;
  if (key === "mac_x64") return urls.mac_x64;
  return "";
}

export const METHOD_SUBLINE: Record<InstallMethod, string> = {
  app_download: "Download the app",
  cli_command: "One Terminal command",
  build_from_source: "Build from source (needs Rust and Node.js)",
  unsupported: "Not supported yet",
};

/* --------------------------------------------------------- Commands */

/** Install the pre-built CLI. */
export function cliCommand(): string {
  return `curl -sSL ${INSTALL_SCRIPT_URL} | bash`;
}

/** Build the CLI from source. */
export function fromSourceCommand(): string {
  return `curl -sSL ${INSTALL_SCRIPT_URL} | bash -s -- --from-source`;
}

/**
 * The command that installs or updates AgentMark on `key`: the pre-built one
 * where it exists, else the from-source one. Undefined on Windows.
 */
export function installCommandFor(key: PlatformKey | undefined): string | undefined {
  if (key === "windows") return undefined;
  if (key === "mac_arm64") return cliCommand();
  return fromSourceCommand();
}

/** The prerequisites for a source build, with where to get them. */
export const SOURCE_PREREQUISITES: ReadonlyArray<{ name: string; url: string }> = [
  { name: "Rust", url: "https://rustup.rs" },
  { name: "Node.js", url: "https://nodejs.org" },
];
