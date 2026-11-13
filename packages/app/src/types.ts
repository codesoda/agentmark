/**
 * Mirrors of the Rust types the app's commands return (snake_case, as in
 * `agentmark::setup` and `src-tauri/src/steps.rs`). Keep in step with both.
 */

export type AgentId = "claude" | "codex" | "pi";

export interface AgentInfo {
  id: string;
  name: string;
  found: boolean;
  path: string | null;
}

export interface Settings {
  storage_path: string;
  storage_path_display: string;
  default_agent: string;
  enrichment_enabled: boolean;
  system_prompt: string | null;
  snapshot_sites: string[];
}

export interface SetupState {
  initialized: boolean;
  config_problem: string | null;
  version: string;
  platform: { os: string; arch: string };
  settings: Settings;
  default_storage_path: string;
  default_storage_path_display: string;
  agents: AgentInfo[];
  builtin_snapshot_sites: string[];
  bookmark_count: number | null;
}

/** Absent = unchanged; `system_prompt: null` clears it (contract §5.2). */
export interface SettingsChanges {
  storage_path?: string;
  default_agent?: string;
  enrichment_enabled?: boolean;
  system_prompt?: string | null;
  snapshot_sites?: string[];
}

export type SettingsField = "storage_path" | "default_agent" | "enrichment_enabled" | "system_prompt" | "snapshot_sites";

export interface FieldError {
  field: SettingsField;
  index: number | null;
  code: string;
  message: string;
}

export type SaveResult = { type: "saved"; state: SetupState } | { type: "invalid"; errors: FieldError[] };

export interface FolderResult {
  outcome: "chosen" | "cancelled" | "unavailable";
  path: string | null;
  path_display: string | null;
  reason: string | null;
}

export type StepId = "cli" | "native_host" | "skill";
export type StepStatus = "done" | "warning" | "waiting" | "failed";

export interface StepReport {
  id: StepId;
  status: StepStatus;
  summary: string;
  detail: string | null;
}

export interface ConnectionRecord {
  connected_at: string;
  extension_id: string | null;
  extension_version: string | null;
  host_version: string;
}

export interface DevExtension {
  path: string;
  path_display: string;
}

export interface AppInfo {
  version: string;
  updates_enabled: boolean;
}

export interface UpdateInfo {
  version: string;
  notes: string | null;
}
