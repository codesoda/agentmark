import { createRoot } from "react-dom/client";
import "@ext/shared/styles.css";
import { WEB_STORE_EXTENSION_ID } from "@ext/shared/constants";

// The store copy's ID, so the install command is the short one users see.
const EXT_ID = WEB_STORE_EXTENSION_ID;
const params = new URLSearchParams(location.search);
const screen = params.get("s") ?? "a-mac";

const agentsAll = [
  { id: "claude", name: "Claude Code", found: true, path: "/Users/me/.local/bin/claude" },
  { id: "codex", name: "Codex", found: true, path: "/opt/homebrew/bin/codex" },
  { id: "pi", name: "pi", found: false, path: null },
];
const agentsNone = agentsAll.map((a) => ({ ...a, found: false, path: null }));
function state(initialized: boolean, noAgent = false) {
  return {
    initialized, config_problem: null, version: "0.3.0", platform: { os: "macos", arch: "aarch64" },
    settings: {
      storage_path: "/Users/me/Documents/AgentMark", storage_path_display: "~/Documents/AgentMark",
      default_agent: "claude", enrichment_enabled: !noAgent, system_prompt: null,
      snapshot_sites: initialized ? ["docs.example.org/app/"] : [],
    },
    default_storage_path: "/Users/me/Documents/AgentMark", default_storage_path_display: "~/Documents/AgentMark",
    agents: noAgent ? agentsNone : agentsAll,
    builtin_snapshot_sites: ["linkedin.com/in/","linkedin.com/posts/","linkedin.com/feed/update/","linkedin.com/company/","x.com","twitter.com","news.ycombinator.com/item","reddit.com"],
    bookmark_count: initialized ? 42 : null,
  };
}

const BOOKMARKS = [
  { id: "am_1", url: "https://notes.example.com/plain-text", title: "Plain text outlives every app", state: "inbox", user_tags: ["notes"], suggested_tags: ["plain-text", "markdown"], saved_at: "2026-11-12T09:00:00Z" },
  { id: "am_2", url: "https://blog.example.org/sqlite-fts", title: "Full-text search with SQLite FTS5", state: "inbox", user_tags: ["sqlite"], suggested_tags: ["search"], saved_at: "2026-11-12T07:40:00Z" },
  { id: "am_3", url: "https://docs.example.net/native-messaging", title: "Talking to a local program from a browser extension", state: "processed", user_tags: ["chrome"], suggested_tags: ["extensions"], saved_at: "2026-11-11T16:10:00Z" },
  { id: "am_4", url: "https://example.com/rust-errors", title: "Error handling patterns in Rust", state: "processed", user_tags: ["rust"], suggested_tags: ["errors"], saved_at: "2026-11-10T11:25:00Z" },
  { id: "am_5", url: "https://example.org/agents-context", title: "Giving coding agents the right context", state: "archived", user_tags: [], suggested_tags: ["agents"], saved_at: "2026-11-08T08:05:00Z" },
];
const DETAIL = { id: "am_1", url: "https://notes.example.com/plain-text", title: "Plain text outlives every app", summary: "Argues that notes kept as plain text files stay readable long after the apps that made them are gone. Covers folder layouts, Markdown front matter and searching with ordinary tools.", saved_at: "2026-11-12T09:00:00Z", capture_source: "extension", state: "inbox", user_tags: ["notes"], suggested_tags: ["plain-text", "markdown"], collections: ["reading"], note: "Compare with how we store project docs." };
const notFound = { success: false, error: "Specified native messaging host not found." };
const platforms: Record<string, [string, string]> = { mac: ["mac", "arm64"], intel: ["mac", "x86-64"], linux: ["linux", "x86-64"], win: ["win", "x86-64"], cros: ["cros", "x86-64"] };
const plat = platforms[params.get("p") ?? (screen.split("-")[1] ?? "mac")] ?? platforms.mac;

const isSetup = screen.startsWith("c");
const ready = !screen.startsWith("a") && !isSetup && !screen.startsWith("sp-missing") && screen !== "forbidden" && screen !== "update";
function respond(msg: { type: string }) {
  switch (msg.type) {
    case "status":
      if (screen === "forbidden") return { success: false, error: "Access to the specified native messaging host is forbidden." };
      if (screen === "update") return { success: true, data: { type: "status_result", ok: true, version: "0.2.6", initialized: false } };
      if (!ready && !isSetup) return notFound;
      return { success: true, data: { type: "status_result", ok: true, version: "0.3.0", initialized: !isSetup } };
    case "get_setup_state":
      if (screen === "update") return { success: false, error: "unknown message type: get_setup_state" };
      return { success: true, data: { type: "setup_state_result", state: state(!isSetup, screen === "c-noagent") } };
    case "save_settings":
      if (screen === "c-error" || screen === "d-error") return { success: true, data: { type: "settings_invalid", errors: [
        { field: "storage_path", index: null, code: "not_writable", message: "AgentMark cannot write to /Volumes/Backup/Bookmarks." },
        { field: "snapshot_sites", index: 1, code: "invalid_site", message: "\"exa mple.com\" is not a host/path rule." } ] } };
      return { success: true, data: { type: "save_settings_result", state: state(true) } };
    case "choose_folder":
      return { success: true, data: { type: "choose_folder_result", outcome: "unavailable", path: null, path_display: null, reason: "No folder dialog is available on this system." } };
    case "save":
      return new Promise((r) => setTimeout(() => r({ success: true, data: { type: "save_result", id: "am_01JCDX4Q2W", path: "/tmp", status: "created" } }), screen === "saving" ? 1e9 : 50));
    case "take_pending_save": return { pending: null };
    case "list": return { success: true, data: { type: "list_result", bookmarks: BOOKMARKS } };
    case "show": return { success: true, data: { type: "show_result", bookmark: DETAIL } };
    case "list_collections": return { success: true, data: { type: "list_collections_result", collections: ["reading", "research"] } };
    default: return { success: false, error: "unhandled " + msg.type };
  }
}
const session: Record<string, unknown> = screen.startsWith("d") ? { agentmark_sidepanel_view: "settings" } : {};
const ev = () => ({ addListener() {}, removeListener() {} });
(window as any).chrome = {
  runtime: { id: EXT_ID, sendMessage: async (m: any) => respond(m), getPlatformInfo: async () => ({ os: plat[0], arch: plat[1] }), getManifest: () => ({ version: "0.3.0" }) },
  tabs: { query: async () => [{ id: 1, url: "https://notes.example.com/plain-text", title: "Plain text outlives every app" }] },
  scripting: { executeScript: async () => [{ result: "" }] },
  storage: {
    local: { get: async () => ({ agentmark_last_intent: { tags: ["notes", "plain-text"], collection: "reading" } }), set: async () => {} },
    session: { get: async (k: string) => (k in session ? { [k]: session[k] } : {}), set: async (o: any) => Object.assign(session, o), remove: async (k: string) => { delete session[k]; } },
    onChanged: ev(),
  },
  permissions: { contains: async () => screen === "d-granted", request: async () => true, remove: async () => true, onAdded: ev(), onRemoved: ev() },
};
(window as any).close = () => {};

const panel = screen.startsWith("sp") || screen.startsWith("d");
const { default: Comp } = panel ? await import("@ext/sidepanel/SidePanel") : await import("@ext/popup/Popup");
document.body.style.margin = "0";
document.body.style.width = panel ? "380px" : "320px";
document.body.style.background = "#fff";
createRoot(document.getElementById("root")!).render(<Comp />);
