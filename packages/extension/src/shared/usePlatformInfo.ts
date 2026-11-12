import { useEffect, useState } from "react";
import type { PlatformInfoLike } from "./platforms";

/** `chrome.runtime.getPlatformInfo()`, or null when it fails. Never throws. */
export async function getPlatformInfo(): Promise<PlatformInfoLike | null> {
  try {
    const info = await chrome.runtime.getPlatformInfo();
    if (info && typeof info.os === "string" && typeof info.arch === "string") {
      return { os: info.os, arch: info.arch };
    }
  } catch {
    // Not available: no platform is highlighted.
  }
  return null;
}

/** The browser's OS and architecture: undefined while loading, null when unknown. */
export function usePlatformInfo(): PlatformInfoLike | null | undefined {
  const [info, setInfo] = useState<PlatformInfoLike | null | undefined>(undefined);
  useEffect(() => {
    let live = true;
    void getPlatformInfo().then((result) => {
      if (live) setInfo(result);
    });
    return () => {
      live = false;
    };
  }, []);
  return info;
}

/** This extension's ID, for the install commands. */
export function runtimeExtensionId(): string | undefined {
  return typeof chrome !== "undefined" ? chrome.runtime?.id : undefined;
}
