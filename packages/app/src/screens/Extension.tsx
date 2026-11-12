/**
 * Add the Chrome extension (spec §5). With a Web Store listing, one button.
 * Until then (`WEB_STORE_URL` empty) the development path: load the
 * extension unpacked. The unpacked copy carries the manifest key, so it has
 * the store ID that the launch step already registered; pasting an ID is
 * only for a copy with another ID (or a build with no store ID, where the
 * launch step waits for one). Then "Waiting for Chrome…"
 * turns to "Connected" when the native host writes its liveness file
 * (contract §9), polled every second while this screen is open.
 */

import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { api, errorText } from "../api";
import { isLiveConnection, lastSeenText } from "../connection";
import { StatusIcon } from "../components/StatusRow";
import Screen from "../components/Screen";
import { card, heading, inputBase, inputOk, lede, primaryButton, secondaryButton } from "../components/ui";
import { CONNECTION_POLL_MS, WEB_STORE_URL } from "../constants";
import type { StepState } from "../steps";
import type { ConnectionRecord, DevExtension, StepReport } from "../types";

export default function Extension({
  nativeHost,
  onNativeHost,
  onConnection,
  onDone,
  webStoreUrl = WEB_STORE_URL,
}: {
  nativeHost: StepState;
  onNativeHost: (report: StepReport) => void;
  onConnection: (record: ConnectionRecord | null) => void;
  onDone: () => void;
  /** Injected by tests; production uses the constant. */
  webStoreUrl?: string;
}) {
  const connection = useConnection(onConnection);
  // An old record (Chrome closed, extension removed) is not "Connected".
  const live = isLiveConnection(connection);

  return (
    <Screen
      testId="extension"
      footer={
        live ? (
          <button type="button" onClick={onDone} className={`${primaryButton} w-full`} data-testid="extension-done">
            Done
          </button>
        ) : (
          <div className="flex justify-end">
            <button type="button" onClick={onDone} className={secondaryButton} data-testid="extension-skip">
              Skip for now
            </button>
          </div>
        )
      }
    >
      <h1 className={heading}>Add the Chrome extension</h1>
      <p className={lede}>The extension adds the save button to Chrome.</p>

      {webStoreUrl ? <StoreStep url={webStoreUrl} /> : <DevelopmentSteps nativeHost={nativeHost} onNativeHost={onNativeHost} />}

      <ConnectionStatus connection={connection} live={live} />
    </Screen>
  );
}

/** Polls the liveness file while mounted, without overlapping requests. */
function useConnection(onChange: (record: ConnectionRecord | null) => void): ConnectionRecord | null {
  const [record, setRecord] = useState<ConnectionRecord | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    let live = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const poll = async () => {
      try {
        const next = await api.getConnection();
        if (!live) return;
        setRecord(next);
        if (next) onChangeRef.current(next);
      } catch {
        // Keep waiting; the next poll tries again.
      }
      if (live) timer = setTimeout(() => void poll(), CONNECTION_POLL_MS);
    };
    void poll();
    return () => {
      live = false;
      if (timer) clearTimeout(timer);
    };
  }, []);

  return record;
}

function StoreStep({ url }: { url: string }) {
  return (
    <div className={`${card} mt-5 p-4`} data-testid="store-step">
      <p className="text-[13px] text-neutral-700 dark:text-neutral-300">
        Open the listing and click <b className="font-semibold text-neutral-900 dark:text-neutral-100">Add to Chrome</b>.
      </p>
      <button type="button" onClick={() => void api.openUrl(url)} className={`${primaryButton} mt-3 w-full`} data-testid="open-store">
        Open the Chrome Web Store
      </button>
    </div>
  );
}

function Step({ n, done, title, children }: { n: number; done?: boolean; title: ReactNode; children?: ReactNode }) {
  return (
    <li className="flex gap-3 px-4 py-3">
      {done ? (
        <StatusIcon state="done" />
      ) : (
        <span
          aria-hidden="true"
          className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-accent-soft text-2xs font-semibold text-accent dark:bg-indigo-400/15 dark:text-indigo-300"
        >
          {n}
        </span>
      )}
      <div className="min-w-0 flex-1">
        <p className="pt-px text-[13px] leading-5 text-neutral-800 dark:text-neutral-200">{title}</p>
        {children}
      </div>
    </li>
  );
}

function DevelopmentSteps({ nativeHost, onNativeHost }: { nativeHost: StepState; onNativeHost: (report: StepReport) => void }) {
  const ids = useId();
  const [folder, setFolder] = useState<DevExtension | null>(null);
  const [folderError, setFolderError] = useState<string | null>(null);
  const [chromeNote, setChromeNote] = useState<string | null>(null);
  const report = typeof nativeHost === "string" ? null : nativeHost;
  const registered = report !== null && (report.status === "done" || report.status === "warning");
  // Only a build with no store ID waits for an ID; then pasting it is a step,
  // and stays one (ticked) after the ID is accepted.
  const [askedForId, setAskedForId] = useState(false);
  const waiting = report?.status === "waiting";
  useEffect(() => {
    if (waiting) setAskedForId(true);
  }, [waiting]);
  const needsId = waiting || askedForId;

  const showFolder = async () => {
    setFolderError(null);
    try {
      setFolder(await api.showDevExtension());
    } catch (err) {
      setFolderError(errorText(err));
    }
  };

  const openChrome = async () => {
    setChromeNote(null);
    try {
      await api.openChromeExtensions();
    } catch {
      setChromeNote("Open Chrome and type chrome://extensions in the address bar.");
    }
  };

  return (
    <section className="mt-5" aria-labelledby={`${ids}-dev`} data-testid="dev-steps">
      <div className="flex items-center gap-2">
        <h2 id={`${ids}-dev`} className="text-[13px] font-semibold text-neutral-900 dark:text-neutral-100">
          Load the extension unpacked
        </h2>
        <span className="rounded-md bg-amber-50 px-1.5 py-0.5 text-2xs font-medium text-amber-800 ring-1 ring-inset ring-amber-600/20 dark:bg-amber-400/10 dark:text-amber-200 dark:ring-amber-400/20">
          Development build
        </span>
      </div>
      <p className="mt-1 text-xs leading-snug text-neutral-500 dark:text-neutral-400">
        AgentMark is not in the Chrome Web Store yet, so Chrome loads it from a folder on this computer.
        {registered && !needsId && " Chrome already accepts this copy, so there is nothing to paste."}
      </p>

      <ol className={`${card} mt-3 divide-y divide-neutral-100 dark:divide-white/[0.06]`}>
        <Step n={1} done={folder !== null} title="Put the extension in a folder.">
          <div className="mt-2 flex flex-wrap items-center gap-2">
            <button type="button" onClick={() => void showFolder()} className={secondaryButton} data-testid="show-folder">
              {folder ? "Show again" : "Show the folder"}
            </button>
            {folder && (
              <code className="truncate font-mono text-xs text-neutral-600 dark:text-neutral-400" data-testid="dev-folder">
                {folder.path_display}
              </code>
            )}
          </div>
          {folderError && <p className="mt-1.5 text-xs text-red-600 dark:text-red-400">{folderError}</p>}
        </Step>
        <Step
          n={2}
          title={
            <>
              In Chrome, open <code className="font-mono text-[12px]">chrome://extensions</code> and turn on{" "}
              <b className="font-semibold">Developer mode</b>.
            </>
          }
        >
          <button type="button" onClick={() => void openChrome()} className={`${secondaryButton} mt-2`} data-testid="open-chrome">
            Open in Chrome
          </button>
          {chromeNote && <p className="mt-1.5 text-xs text-neutral-500 dark:text-neutral-400">{chromeNote}</p>}
        </Step>
        <Step
          n={3}
          title={
            <>
              Click <b className="font-semibold">Load unpacked</b> and choose that folder.
            </>
          }
        />
        {needsId && (
          <Step n={4} done={registered} title="Copy the ID from the AgentMark card and paste it here.">
            <IdForm nativeHost={nativeHost} onNativeHost={onNativeHost} alwaysShowRegistered />
          </Step>
        )}
      </ol>

      {!needsId && (
        <details className="group mt-3" data-testid="other-id">
          <summary className="cursor-pointer select-none text-xs text-neutral-500 hover:text-neutral-700 dark:text-neutral-400 dark:hover:text-neutral-200">
            Loaded a copy with a different ID?
          </summary>
          <p className="mt-1.5 text-xs leading-snug text-neutral-500 dark:text-neutral-400">
            A copy built without AgentMark's key, such as an older build, gets its own ID. Copy it from the AgentMark card and paste it here.
          </p>
          <IdForm nativeHost={nativeHost} onNativeHost={onNativeHost} />
        </details>
      )}
    </section>
  );
}

/** Paste an extension ID to allow it (contract §8.1 development IDs). */
function IdForm({
  nativeHost,
  onNativeHost,
  alwaysShowRegistered = false,
}: {
  nativeHost: StepState;
  onNativeHost: (report: StepReport) => void;
  /** Show the registration even before an ID is pasted here. */
  alwaysShowRegistered?: boolean;
}) {
  const [extensionId, setExtensionId] = useState("");
  const [registering, setRegistering] = useState(false);
  const [idError, setIdError] = useState<string | null>(null);
  const [added, setAdded] = useState(false);
  const report = typeof nativeHost === "string" ? null : nativeHost;
  const registered = report !== null && (report.status === "done" || report.status === "warning");

  const register = async () => {
    if (!extensionId.trim() || registering) return;
    setRegistering(true);
    setIdError(null);
    try {
      const next = await api.addExtensionId(extensionId);
      onNativeHost(next);
      if (next.status === "failed") setIdError(next.detail ?? next.summary);
      else {
        setExtensionId("");
        setAdded(true);
      }
    } catch (err) {
      setIdError(errorText(err));
    } finally {
      setRegistering(false);
    }
  };

  return (
    <>
      <form
        className="mt-2 flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void register();
        }}
      >
        <input
          type="text"
          value={extensionId}
          onChange={(e) => setExtensionId(e.target.value)}
          placeholder="32 letters, a to p"
          aria-label="Extension ID"
          aria-invalid={idError ? true : undefined}
          spellCheck={false}
          autoComplete="off"
          autoCorrect="off"
          autoCapitalize="off"
          className={`${inputBase} h-8 font-mono ${idError ? "border-red-400" : inputOk}`}
          data-testid="extension-id"
        />
        <button type="submit" disabled={!extensionId.trim() || registering} className={secondaryButton} data-testid="register-id">
          {registering ? "Saving…" : "Connect"}
        </button>
      </form>
      {idError && (
        <p className="mt-1.5 text-xs leading-snug text-red-600 dark:text-red-400" data-testid="id-error">
          {idError}
        </p>
      )}
      {(added || alwaysShowRegistered) && registered && !idError && report && (
        <p className="mt-1.5 text-xs text-emerald-700 dark:text-emerald-300" data-testid="id-registered">
          {report.summary}
        </p>
      )}
    </>
  );
}

function ConnectionStatus({ connection, live }: { connection: ConnectionRecord | null; live: boolean }) {
  if (connection && live) {
    return (
      <div
        className="mt-4 flex items-center gap-3 rounded-xl border border-emerald-600/20 bg-emerald-50 px-4 py-3 dark:border-emerald-400/20 dark:bg-emerald-400/[0.08]"
        role="status"
        data-testid="connected"
      >
        <StatusIcon state="done" />
        <div>
          <p className="text-[13px] font-semibold text-emerald-900 dark:text-emerald-200">Connected</p>
          <p className="text-xs text-emerald-800/80 dark:text-emerald-300/80">
            {connection.extension_version ? `Chrome extension ${connection.extension_version}. ` : ""}Save a page to try it.
          </p>
        </div>
      </div>
    );
  }
  return (
    <div className="mt-4 flex items-center gap-3 rounded-xl border border-dashed border-neutral-300 px-4 py-3 dark:border-white/15" role="status" data-testid="waiting">
      <span className="relative flex h-5 w-5 items-center justify-center" aria-hidden="true">
        <span className="absolute h-2.5 w-2.5 animate-ping rounded-full bg-accent/40" />
        <span className="h-2 w-2 rounded-full bg-accent" />
      </span>
      <div>
        <p className="text-[13px] font-medium text-neutral-900 dark:text-neutral-100">Waiting for Chrome…</p>
        <p className="text-xs text-neutral-500 dark:text-neutral-400">This changes by itself once the extension is added.</p>
        {connection && (
          <p className="mt-0.5 text-xs text-neutral-500 dark:text-neutral-400" data-testid="last-seen">
            {lastSeenText(connection)}.
          </p>
        )}
      </div>
    </div>
  );
}
