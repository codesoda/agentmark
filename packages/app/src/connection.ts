/**
 * Whether the liveness file (contract §9) shows a connection made while this
 * app is open. The file stays after Chrome quits or the extension is removed,
 * so an older record only says when the extension was last seen.
 */

import type { ConnectionRecord } from "./types";

/** When this app started. A record written before it is not a live connection. */
export const APP_STARTED_AT = Date.now();

/**
 * `connected_at` has whole seconds, so a record written in the same second
 * the app started can read up to a second early. Allow a little more.
 */
const CLOCK_SLACK_MS = 2000;

/** True when the extension connected after `since` (default: the app's start). */
export function isLiveConnection(record: ConnectionRecord | null, since: number = APP_STARTED_AT): boolean {
  if (!record) return false;
  const at = Date.parse(record.connected_at);
  return Number.isFinite(at) && at >= since - CLOCK_SLACK_MS;
}

/** "Last connected 12 Nov, 19:21" in the user's locale, plus the extension version when known. */
export function lastSeenText(record: ConnectionRecord): string {
  const at = Date.parse(record.connected_at);
  const version = record.extension_version ? ` · version ${record.extension_version}` : "";
  if (!Number.isFinite(at)) return `Connected before${version}`;
  const when = new Date(at).toLocaleString(undefined, { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
  return `Last connected ${when}${version}`;
}
