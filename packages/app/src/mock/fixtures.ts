/** Sample command results, shaped exactly like the Rust side's JSON. */

import type { AgentInfo, ConnectionRecord, SetupState, StepReport } from "../types";

export const VERSION = "0.2.6";

export const BUILTIN_SITES = [
  "linkedin.com/in/",
  "linkedin.com/posts/",
  "linkedin.com/feed/update/",
  "linkedin.com/company/",
  "x.com",
  "twitter.com",
  "news.ycombinator.com/item",
  "reddit.com",
];

export function agents(found: { claude?: boolean; codex?: boolean; pi?: boolean } = { claude: true, codex: true }): AgentInfo[] {
  return [
    { id: "claude", name: "Claude Code", found: !!found.claude, path: found.claude ? "/Users/me/.local/bin/claude" : null },
    { id: "codex", name: "Codex", found: !!found.codex, path: found.codex ? "/opt/homebrew/bin/codex" : null },
    { id: "pi", name: "pi", found: !!found.pi, path: found.pi ? "/Users/me/.local/bin/pi" : null },
  ];
}

export function setupState(overrides: Partial<SetupState> = {}): SetupState {
  const list = overrides.agents ?? agents();
  const anyFound = list.some((a) => a.found);
  return {
    initialized: false,
    config_problem: null,
    version: VERSION,
    platform: { os: "macos", arch: "aarch64" },
    settings: {
      storage_path: "/Users/me/Documents/AgentMark",
      storage_path_display: "~/Documents/AgentMark",
      default_agent: list.find((a) => a.found)?.id ?? "claude",
      enrichment_enabled: anyFound,
      system_prompt: null,
      snapshot_sites: [],
    },
    default_storage_path: "/Users/me/Documents/AgentMark",
    default_storage_path_display: "~/Documents/AgentMark",
    agents: list,
    builtin_snapshot_sites: BUILTIN_SITES,
    bookmark_count: null,
    ...overrides,
  };
}

export function readyState(overrides: Partial<SetupState> = {}): SetupState {
  const base = setupState({ initialized: true, bookmark_count: 674, ...overrides });
  return { ...base, settings: { ...base.settings, snapshot_sites: ["example.com/app/"], ...overrides.settings } };
}

/** A connection made while the app is open (now, when the fixtures load). */
export const CONNECTION: ConnectionRecord = {
  connected_at: new Date().toISOString(),
  extension_id: "abcdefghijklmnopabcdefghijklmnop",
  extension_version: VERSION,
  host_version: VERSION,
};

/** A record left from an earlier Chrome session: not a live connection. */
export const STALE_CONNECTION: ConnectionRecord = { ...CONNECTION, connected_at: "2026-08-03T09:21:36Z" };

export const STEP_DONE: Record<"cli" | "native_host" | "skill", StepReport> = {
  cli: {
    id: "cli",
    status: "done",
    summary: `Installed agentmark ${VERSION}.`,
    detail: "~/.agentmark/bin/agentmark, linked from ~/.local/bin/agentmark",
  },
  native_host: { id: "native_host", status: "done", summary: "Registered with Chrome and Arc.", detail: null },
  skill: { id: "skill", status: "done", summary: "Added the agent skill.", detail: "For Agents (shared) and Claude Code" },
};

export const HOST_WAITING: StepReport = {
  id: "native_host",
  status: "waiting",
  summary: "Waiting for the Chrome extension's ID.",
  detail: "Development build: this build has no Chrome Web Store ID, so Chrome needs the ID of an extension you load yourself.",
};

export const CLI_FAILED: StepReport = {
  id: "cli",
  status: "failed",
  summary: "The agentmark command is missing from this copy of the app.",
  detail: "Expected it at /Applications/AgentMark.app/Contents/MacOS/agentmark. Download the app again.",
};
