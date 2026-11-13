/** About (spec §5): version, links, and updates when the build has them. */

import { useEffect, useState } from "react";
import { api, errorText } from "../api";
import AgentMarkMark from "../components/AgentMarkMark";
import { SectionLabel } from "../components/controls";
import Screen from "../components/Screen";
import { card, focusRing, secondaryButton } from "../components/ui";
import { REPO_URL, SITE_URL } from "../constants";
import type { AppInfo, UpdateInfo } from "../types";

type UpdateState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "current" }
  | { status: "available"; update: UpdateInfo }
  | { status: "installing" }
  | { status: "error"; message: string };

const LINKS = [
  { label: "Website", url: SITE_URL },
  { label: "Source on GitHub", url: REPO_URL },
  { label: "Report a problem", url: `${REPO_URL}/issues` },
];

function hostOf(url: string): string {
  return url.replace(/^https:\/\//, "");
}

export default function About() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [update, setUpdate] = useState<UpdateState>({ status: "idle" });

  useEffect(() => {
    let live = true;
    api
      .appInfo()
      .then((next) => live && setInfo(next))
      .catch(() => live && setInfo(null));
    return () => {
      live = false;
    };
  }, []);

  const check = async () => {
    setUpdate({ status: "checking" });
    try {
      const found = await api.checkForUpdate();
      setUpdate(found ? { status: "available", update: found } : { status: "current" });
    } catch (err) {
      setUpdate({ status: "error", message: errorText(err) });
    }
  };

  const install = async () => {
    setUpdate({ status: "installing" });
    try {
      await api.installUpdate();
    } catch (err) {
      setUpdate({ status: "error", message: errorText(err) });
    }
  };

  return (
    <Screen testId="about">
      <div className="flex flex-col items-center pt-3 text-center">
        <AgentMarkMark size={56} />
        <h1 className="mt-4 text-[20px] font-semibold tracking-[-0.01em] text-neutral-900 dark:text-neutral-50">AgentMark</h1>
        <p className="mt-1 text-[13px] text-neutral-600 dark:text-neutral-400">Bookmarks your agent can read.</p>
        <p className="mt-2 rounded-full bg-neutral-200/60 px-2.5 py-0.5 font-mono text-2xs text-neutral-600 dark:bg-white/[0.07] dark:text-neutral-300" data-testid="about-version">
          {info ? `Version ${info.version}` : "Version …"}
        </p>
      </div>

      <section className="mt-8" aria-label="Links">
        <SectionLabel>Links</SectionLabel>
        <ul className={`${card} mt-2 divide-y divide-neutral-100 dark:divide-white/[0.06]`}>
          {LINKS.map((link) => (
            <li key={link.label}>
              <button
                type="button"
                onClick={() => void api.openUrl(link.url)}
                className={`flex h-10 w-full items-center justify-between gap-3 rounded-xl px-3.5 text-left text-[13px] hover:bg-neutral-50 dark:hover:bg-white/[0.04] ${focusRing}`}
                data-testid={`link-${link.label}`}
              >
                <span className="text-neutral-900 dark:text-neutral-100">{link.label}</span>
                <span className="flex min-w-0 items-center gap-1.5 text-xs text-neutral-500 dark:text-neutral-400">
                  <span className="truncate">{hostOf(link.url)}</span>
                  <svg viewBox="0 0 12 12" className="h-3 w-3 shrink-0" fill="none" aria-hidden="true">
                    <path d="M4.5 2.5h5v5M9.5 2.5 3 9" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                </span>
              </button>
            </li>
          ))}
        </ul>
      </section>

      <section className="mt-6" aria-label="Updates">
        <SectionLabel>Updates</SectionLabel>
        <div className={`${card} mt-2 flex min-h-[52px] items-center justify-between gap-3 px-3.5 py-2.5`} data-testid="updates">
          {info && !info.updates_enabled ? (
            <p className="text-xs leading-snug text-neutral-500 dark:text-neutral-400" data-testid="updates-off">
              Automatic updates are off in this build. Download a new copy to update.
            </p>
          ) : (
            <>
              <p className="text-xs leading-snug text-neutral-600 dark:text-neutral-400" aria-live="polite" data-testid="update-status">
                {update.status === "checking"
                  ? "Checking…"
                  : update.status === "current"
                    ? "AgentMark is up to date."
                    : update.status === "available"
                      ? `Version ${update.update.version} is available.`
                      : update.status === "installing"
                        ? "Installing. AgentMark restarts when done."
                        : update.status === "error"
                          ? update.message
                          : "Updates come from GitHub Releases."}
              </p>
              {update.status === "available" ? (
                <button type="button" onClick={() => void install()} className={secondaryButton} data-testid="install-update">
                  Install and restart
                </button>
              ) : (
                <button
                  type="button"
                  onClick={() => void check()}
                  disabled={!info || update.status === "checking" || update.status === "installing"}
                  className={secondaryButton}
                  data-testid="check-update"
                >
                  Check for updates
                </button>
              )}
            </>
          )}
        </div>
      </section>
    </Screen>
  );
}
