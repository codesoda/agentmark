/**
 * Screens A and B (docs/setup-spec.md §4): the AgentMark app was not found.
 * A lists the platforms with the detected one first; B shows how to install
 * on the chosen one, driven by the table in `platforms.ts`. The surface keeps
 * probing while this shows (see `useAppState`), so B only waits.
 */

import { useState, type ReactNode } from "react";
import AgentMarkMark from "./AgentMarkMark";
import CopyCommand from "./CopyCommand";
import { SETUP_GUIDE_URL } from "./constants";
import {
  cliCommand,
  downloadUrl,
  fromSourceCommand,
  installMethod,
  METHOD_SUBLINE,
  orderPlatforms,
  platformByKey,
  SOURCE_PREREQUISITES,
  type Platform,
  type PlatformKey,
} from "./platforms";
import { runtimeExtensionId, usePlatformInfo } from "./usePlatformInfo";

interface AppMissingProps {
  onCheckAgain: () => void;
  /** The raw error, shown collapsed on the install screen for troubleshooting. */
  detail?: string;
}

const focusRing = "focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500 focus-visible:ring-offset-1";

export default function AppMissing({ onCheckAgain, detail }: AppMissingProps) {
  const info = usePlatformInfo();
  const [chosen, setChosen] = useState<PlatformKey | null>(null);

  if (chosen) {
    return (
      <InstallMethod
        platform={platformByKey(chosen)}
        onBack={() => setChosen(null)}
        onCheckAgain={onCheckAgain}
        detail={detail}
      />
    );
  }

  const { detected, others } = orderPlatforms(info ?? undefined);

  return (
    <section aria-labelledby="app-missing-heading" data-testid="app-missing">
      <div className="flex items-center gap-2">
        <AgentMarkMark look="idle" />
        <h1 id="app-missing-heading" className="text-[15px] font-semibold text-gray-900">
          Finish setting up AgentMark
        </h1>
      </div>
      <p className="mt-1.5 text-[13px] leading-snug text-gray-600">
        The extension saves pages through the AgentMark app on this computer.
      </p>

      {info !== undefined && (
        <>
          {detected && (
            <div className="mt-3">
              <PlatformRow platform={detected} detected onSelect={setChosen} />
            </div>
          )}
          <h2 className="mb-1.5 mt-3 text-xs font-medium text-gray-500" id="other-platforms">
            Other platforms
          </h2>
          <ul className="space-y-1.5" aria-labelledby="other-platforms" data-testid="other-platforms">
            {others.map((platform) => (
              <li key={platform.key}>
                <PlatformRow platform={platform} onSelect={setChosen} />
              </li>
            ))}
          </ul>
        </>
      )}

      <p className="mt-3 border-t border-gray-100 pt-2.5 text-xs text-gray-500">
        Already installed?{" "}
        <button
          type="button"
          onClick={onCheckAgain}
          className={`rounded font-medium text-indigo-600 hover:text-indigo-800 ${focusRing}`}
          data-testid="install-retry"
        >
          Check again
        </button>
      </p>
    </section>
  );
}

function PlatformRow({
  platform,
  detected = false,
  onSelect,
}: {
  platform: Platform;
  detected?: boolean;
  onSelect: (key: PlatformKey) => void;
}) {
  const subline = METHOD_SUBLINE[installMethod(platform.key)];
  return (
    <button
      type="button"
      onClick={() => onSelect(platform.key)}
      className={`flex w-full items-center gap-2.5 rounded-lg border px-2.5 py-2 text-left transition-colors ${focusRing} ${
        detected
          ? "border-indigo-500 bg-indigo-50/70 hover:bg-indigo-50"
          : "border-gray-200 bg-white hover:border-gray-300 hover:bg-gray-50"
      }`}
      data-testid={`platform-${platform.key}`}
      data-detected={detected ? "true" : undefined}
    >
      <PlatformGlyph platformKey={platform.key} />
      <span className="min-w-0 flex-1">
        <span className="block text-[13px] font-semibold text-gray-900">{platform.name}</span>
        <span className="block text-xs text-gray-500">
          {detected ? `Your computer · ${subline}` : subline}
        </span>
      </span>
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true" className={detected ? "text-indigo-500" : "text-gray-400"}>
        <path d="M6 3.5 10.5 8 6 12.5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    </button>
  );
}

/** Neutral line glyphs, not vendor logos. */
function PlatformGlyph({ platformKey }: { platformKey: PlatformKey }) {
  const stroke = { fill: "none", stroke: "currentColor", strokeWidth: 1.4, strokeLinecap: "round", strokeLinejoin: "round" } as const;
  let body: ReactNode;
  switch (platformKey) {
    case "mac_arm64":
    case "mac_x64":
      // A laptop.
      body = (
        <>
          <rect x="4" y="5" width="12" height="8.5" rx="1.2" {...stroke} />
          <path d="M2.5 15.5h15" {...stroke} />
        </>
      );
      break;
    case "linux":
      // A terminal prompt.
      body = (
        <>
          <rect x="3" y="4.5" width="14" height="11" rx="1.5" {...stroke} />
          <path d="m6 8.5 2 1.75-2 1.75M10 12.25h3.5" {...stroke} />
        </>
      );
      break;
    case "windows":
      // A desktop monitor.
      body = (
        <>
          <rect x="3" y="4" width="14" height="9.5" rx="1.2" {...stroke} />
          <path d="M8 16.5h4M10 13.5v3" {...stroke} />
        </>
      );
      break;
  }
  return (
    <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md border border-gray-200 bg-white text-gray-500">
      <svg viewBox="0 0 20 20" width="16" height="16" aria-hidden="true">
        {body}
      </svg>
    </span>
  );
}

function Step({ n, children }: { n: number; children: ReactNode }) {
  return (
    <li className="flex min-w-0 gap-2">
      <span
        aria-hidden="true"
        className="mt-px flex h-[18px] w-[18px] shrink-0 items-center justify-center rounded-full bg-indigo-50 text-[11px] font-semibold text-indigo-600"
      >
        {n}
      </span>
      <div className="min-w-0 flex-1 text-[13px] leading-snug text-gray-800">{children}</div>
    </li>
  );
}

function InstallMethod({
  platform,
  onBack,
  onCheckAgain,
  detail,
}: {
  platform: Platform;
  onBack: () => void;
  onCheckAgain: () => void;
  detail?: string;
}) {
  const method = installMethod(platform.key);
  const extensionId = runtimeExtensionId();
  const terminal = platform.key === "linux" ? "a terminal" : "Terminal";
  const comeBack = <Step n={2}>Come back here. This updates by itself.</Step>;

  let body: ReactNode;
  switch (method) {
    case "app_download":
      body = (
        <>
          <a
            href={downloadUrl(platform.key)}
            target="_blank"
            rel="noreferrer"
            className={`mt-3 flex w-full items-center justify-center rounded-lg bg-indigo-600 px-4 py-2 text-sm font-semibold text-white hover:bg-indigo-700 ${focusRing}`}
            data-testid="download-app"
          >
            Download AgentMark for Mac
          </a>
          <p className="mt-2 text-xs text-gray-500">Open the app, then come back here.</p>
          <p className="mt-3 text-xs font-medium text-gray-600">Or use Terminal instead:</p>
          <CopyCommand
            command={platform.key === "mac_arm64" ? cliCommand(extensionId) : fromSourceCommand(extensionId)}
            label="install command"
          />
        </>
      );
      break;
    case "cli_command":
      body = (
        <ol className="mt-3 space-y-3">
          <Step n={1}>
            Open <strong className="font-semibold">Terminal</strong> and paste this:
            <CopyCommand command={cliCommand(extensionId)} label="install command" />
          </Step>
          {comeBack}
        </ol>
      );
      break;
    case "build_from_source":
      body = (
        <>
          <p className="mt-1.5 text-[13px] leading-snug text-gray-600">
            There is no ready-made build for {platform.name} yet. AgentMark builds from source, which needs{" "}
            {SOURCE_PREREQUISITES.map((tool, i) => (
              <span key={tool.name}>
                {i > 0 && " and "}
                <a href={tool.url} target="_blank" rel="noreferrer" className={`rounded font-medium text-indigo-600 hover:text-indigo-800 ${focusRing}`}>
                  {tool.name}
                </a>
              </span>
            ))}
            .
          </p>
          <ol className="mt-3 space-y-3">
            <Step n={1}>
              Open {terminal} and paste this:
              <CopyCommand command={fromSourceCommand(extensionId)} label="install command" />
            </Step>
            {comeBack}
          </ol>
        </>
      );
      break;
    case "unsupported":
      body = (
        <p className="mt-2 text-[13px] leading-snug text-gray-600" data-testid="unsupported">
          AgentMark does not run on Windows yet.
        </p>
      );
      break;
  }

  return (
    <section aria-labelledby="install-method-heading" data-testid="install-method" data-method={method}>
      <button
        type="button"
        onClick={onBack}
        className={`-ml-0.5 inline-flex items-center gap-0.5 rounded text-xs font-medium text-indigo-600 hover:text-indigo-800 ${focusRing}`}
        data-testid="all-platforms"
      >
        <svg viewBox="0 0 16 16" width="12" height="12" aria-hidden="true">
          <path d="M10 3.5 5.5 8l4.5 4.5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
        All platforms
      </button>
      <h1 id="install-method-heading" className="mt-1.5 text-[15px] font-semibold text-gray-900">
        Install on {platform.name}
      </h1>
      {body}

      {method !== "unsupported" && <WaitingForApp onCheckAgain={onCheckAgain} />}

      <div className="mt-3 flex items-center justify-between gap-3 text-[11px] text-gray-400">
        {detail ? (
          <details className="min-w-0">
            <summary className="cursor-pointer rounded hover:text-gray-600">Error details</summary>
            <p className="mt-1 break-words" data-testid="install-prompt-detail">{detail}</p>
          </details>
        ) : (
          <span />
        )}
        <a
          href={SETUP_GUIDE_URL}
          target="_blank"
          rel="noreferrer"
          className={`shrink-0 self-start rounded hover:text-gray-600 ${focusRing}`}
          data-testid="setup-guide-link"
        >
          Setup guide
        </a>
      </div>
    </section>
  );
}

/** The "Waiting for the app…" strip shown while the surface polls (contract §4.3). */
export function WaitingForApp({ onCheckAgain }: { onCheckAgain: () => void }) {
  return (
    <div
      className="mt-4 flex items-center gap-2 rounded-lg border border-dashed border-indigo-200 bg-indigo-50/40 px-3 py-2"
      data-testid="waiting-for-app"
    >
      <span className="relative flex h-2 w-2 shrink-0" aria-hidden="true">
        <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-indigo-400 opacity-60 motion-reduce:animate-none" />
        <span className="relative inline-flex h-2 w-2 rounded-full bg-indigo-500" />
      </span>
      <span className="flex-1 text-[13px] text-indigo-700" role="status">
        Waiting for the app…
      </span>
      <button
        type="button"
        onClick={onCheckAgain}
        className={`rounded px-1 text-xs font-medium text-indigo-600 hover:text-indigo-800 ${focusRing}`}
        data-testid="install-retry"
      >
        Check again
      </button>
    </div>
  );
}
