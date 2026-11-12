/** The Settings form's values and the partial update they make (contract §5.2). */

import type { Settings, SettingsChanges } from "../shared/types";

export interface SettingsFormValues {
  storage_path: string;
  default_agent: string;
  enrichment_enabled: boolean;
  system_prompt: string;
  snapshot_sites: string[];
}

export function formFromSettings(settings: Settings): SettingsFormValues {
  return {
    storage_path: settings.storage_path_display,
    default_agent: settings.default_agent,
    enrichment_enabled: settings.enrichment_enabled,
    system_prompt: settings.system_prompt ?? "",
    snapshot_sites: [...settings.snapshot_sites],
  };
}

/** Only what differs from `saved`; an empty object means nothing to save. */
export function buildChanges(saved: Settings, form: SettingsFormValues): SettingsChanges {
  const changes: SettingsChanges = {};
  const path = form.storage_path.trim();
  if (path !== saved.storage_path_display && path !== saved.storage_path) changes.storage_path = path;
  if (form.default_agent !== saved.default_agent) changes.default_agent = form.default_agent;
  if (form.enrichment_enabled !== saved.enrichment_enabled) changes.enrichment_enabled = form.enrichment_enabled;
  const prompt = form.system_prompt.trim() === "" ? null : form.system_prompt;
  if ((prompt ?? null) !== (saved.system_prompt ?? null)) changes.system_prompt = prompt;
  const sites = form.snapshot_sites;
  if (sites.length !== saved.snapshot_sites.length || sites.some((site, i) => site !== saved.snapshot_sites[i])) {
    changes.snapshot_sites = [...sites];
  }
  return changes;
}

