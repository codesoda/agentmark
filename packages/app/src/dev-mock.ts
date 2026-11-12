/**
 * Browser preview of the app (`npm run dev`, then open localhost:1430 in a
 * browser). Only loaded in development when Tauri is absent. Pick a state
 * with `?scenario=` (fresh, ready, unconnected, failed, noagent,
 * working, invalid, problem, nostoreid) and a screen with `?view=` (home, setup, extension, settings,
 * about). `&chrome` draws the macOS window controls; `&updates` turns the
 * About screen's update check on; `&connected` starts connected;
 * `&stale` shows a liveness record left from an earlier Chrome session.
 */

import type { View } from "./components/Header";
import { CLI_FAILED, CONNECTION, HOST_WAITING, STALE_CONNECTION, STEP_DONE, agents, readyState, setupState } from "./mock/fixtures";
import { installHandlers } from "./mock/handlers";
import type { FieldError, SettingsChanges, SetupState, StepId } from "./types";

const VIEWS: View[] = ["home", "setup", "extension", "settings", "about"];

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

export function installDevMock(): View | undefined {
  const params = new URLSearchParams(location.search);
  const scenario = params.get("scenario") ?? "fresh";
  const view = params.get("view") as View | null;

  let state: SetupState =
    scenario === "ready" || scenario === "unconnected" ? readyState() : setupState(scenario === "noagent" ? { agents: agents({}) } : {});
  if (scenario === "problem") state = { ...state, config_problem: "invalid TOML at line 3: expected `=`" };
  // The store ID is always registered; `nostoreid` previews a build without
  // one, where the launch step waits for a pasted ID.
  let registered = scenario !== "nostoreid";
  let connected = scenario === "ready" || params.has("connected");

  installHandlers({
    app_info: () => ({ version: state.version, updates_enabled: params.has("updates") }),
    run_step: async ({ step }) => {
      const id = step as StepId;
      if (scenario === "working" && id !== "cli") return new Promise(() => {});
      await sleep(scenario === "ready" ? 150 : 450);
      if (scenario === "failed" && id === "cli") return CLI_FAILED;
      if (id === "native_host" && !registered) return HOST_WAITING;
      return STEP_DONE[id];
    },
    get_setup_state: () => state,
    save_settings: async ({ changes }) => {
      await sleep(300);
      const c = changes as SettingsChanges;
      if (scenario === "invalid") {
        const errors: FieldError[] = [
          { field: "storage_path", index: null, code: "not_writable", message: "AgentMark cannot write to /Volumes/Backup." },
        ];
        return { type: "invalid", errors };
      }
      const path = c.storage_path ?? state.settings.storage_path_display;
      state = {
        ...state,
        initialized: true,
        bookmark_count: state.bookmark_count ?? 0,
        settings: {
          ...state.settings,
          storage_path: path.replace(/^~/, "/Users/me"),
          storage_path_display: path,
          default_agent: c.default_agent ?? state.settings.default_agent,
          enrichment_enabled: c.enrichment_enabled ?? state.settings.enrichment_enabled,
          system_prompt: c.system_prompt === undefined ? state.settings.system_prompt : c.system_prompt,
          snapshot_sites: c.snapshot_sites ?? state.settings.snapshot_sites,
        },
      };
      return { type: "saved", state };
    },
    choose_folder: async () => {
      await sleep(400);
      return { outcome: "chosen", path: "/Users/me/Notes/AgentMark", path_display: "~/Notes/AgentMark", reason: null };
    },
    get_connection: () => (connected ? CONNECTION : params.has("stale") ? STALE_CONNECTION : null),
    show_dev_extension: () => ({ path: "/Users/me/.agentmark/extension", path_display: "~/.agentmark/extension" }),
    add_extension_id: async ({ id }) => {
      await sleep(300);
      if (!/^[a-p]{32}$/.test(String(id).trim())) {
        throw "That is not an extension ID. It is 32 letters from a to p, shown on the extension's card.";
      }
      registered = true;
      setTimeout(() => (connected = true), 2500);
      return STEP_DONE.native_host;
    },
    open_url: () => undefined,
    open_chrome_extensions: () => undefined,
    check_for_update: async () => {
      await sleep(600);
      return null;
    },
    install_update: () => undefined,
    "plugin:window|close": () => undefined,
  });

  if (params.has("chrome")) drawTrafficLights();
  return view && VIEWS.includes(view) ? view : undefined;
}

/** The macOS window controls, so preview screenshots show the header as it really sits. */
function drawTrafficLights() {
  const box = document.createElement("div");
  box.setAttribute("aria-hidden", "true");
  box.style.cssText = "position:fixed;top:20px;left:20px;display:flex;gap:8px;z-index:50;pointer-events:none";
  for (const color of ["#ff5f57", "#febc2e", "#28c840"]) {
    const dot = document.createElement("span");
    dot.style.cssText = `width:12px;height:12px;border-radius:50%;background:${color};box-shadow:inset 0 0 0 0.5px rgba(0,0,0,.18)`;
    box.appendChild(dot);
  }
  document.body.appendChild(box);
}
