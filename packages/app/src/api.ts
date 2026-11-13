/**
 * The app's Tauri commands, typed. Every screen goes through here, so tests
 * mock one IPC layer (`mockIPC`) and the dev preview does the same.
 */

import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  AppInfo,
  ConnectionRecord,
  DevExtension,
  FolderResult,
  SaveResult,
  SettingsChanges,
  SetupState,
  StepId,
  StepReport,
  UpdateInfo,
} from "./types";

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  runStep: (step: StepId) => invoke<StepReport>("run_step", { step }),
  getSetupState: () => invoke<SetupState>("get_setup_state"),
  saveSettings: (changes: SettingsChanges) => invoke<SaveResult>("save_settings", { changes }),
  chooseFolder: (startPath?: string) => invoke<FolderResult>("choose_folder", { startPath: startPath ?? null }),
  getConnection: () => invoke<ConnectionRecord | null>("get_connection"),
  showDevExtension: () => invoke<DevExtension>("show_dev_extension"),
  addExtensionId: (id: string) => invoke<StepReport>("add_extension_id", { id }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  openChromeExtensions: () => invoke<void>("open_chrome_extensions"),
  checkForUpdate: () => invoke<UpdateInfo | null>("check_for_update"),
  installUpdate: () => invoke<void>("install_update"),
  /** Closing the window quits the app: it does not stay running. */
  closeWindow: () => getCurrentWindow().close(),
};

/** Commands reject with the Rust error string; anything else becomes one. */
export function errorText(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
