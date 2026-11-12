import type { ReactNode } from "react";
import {
  INSTALL_LINKS,
  METHOD_SUBLINE,
  PLATFORMS,
  SOURCE_PREREQUISITES,
  cliCommand,
  downloadUrl,
  fromSourceCommand,
  installMethod,
  type Platform,
} from "@/lib/platforms";
import { REPO_URL } from "@/lib/site";
import { CopyCommand } from "./copy-command";
import { GitHubIcon } from "./icons";
import { Code, Eyebrow, H2, Lead, Section } from "./section";

/*
 * The install section: two doors, "Add to Chrome" and "Download for Mac",
 * driven by the URL constants in src/lib/site.ts. While a URL is empty its
 * door shows the path that works today instead of a link.
 */

/* --------------------------------------------------------------- Install */

export const BUTTON_SOLID =
  "inline-flex items-center gap-2 rounded-lg bg-ink px-4 py-2.5 text-[14px] font-medium text-bg transition-opacity hover:opacity-85";
export const BUTTON_OUTLINE =
  "inline-flex items-center gap-2 rounded-lg border border-line-strong bg-surface px-4 py-2.5 text-[14px] font-medium text-ink transition-colors hover:bg-raised";

/** A puzzle piece: the generic browser-extension glyph, not a browser logo. */
export function ExtensionGlyph() {
  return (
    <svg viewBox="0 0 16 16" className="size-4" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" aria-hidden>
      <path d="M6.5 2.5h3v2a1 1 0 0 0 2 0v0h2v3h-1.5a1.25 1.25 0 0 0 0 2.5H13.5v3.5h-3.5v-1.5a1.25 1.25 0 0 0-2.5 0v1.5H3.5V10h1.5a1.25 1.25 0 0 0 0-2.5H3.5v-3h3z" />
    </svg>
  );
}

export function DownloadGlyph() {
  return (
    <svg viewBox="0 0 16 16" className="size-4" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M8 2.5v7.5M4.75 7 8 10.25 11.25 7M3 13.5h10" />
    </svg>
  );
}

function Pill({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1.5 rounded-full border border-line-strong px-2.5 py-0.5 text-[12px] text-muted">
      <span className="size-1.5 rounded-full bg-line-strong" aria-hidden />
      {children}
    </span>
  );
}

function NextSteps({ title = "What happens next", steps }: { title?: string; steps: ReactNode[] }) {
  return (
    <div className="mt-7 border-t border-line pt-6">
      <h4 className="font-mono text-[11.5px] uppercase tracking-[0.14em] text-subtle">{title}</h4>
      <ol className="mt-4 space-y-3.5">
        {steps.map((step, i) => (
          <li key={i} className="grid grid-cols-[1.75rem_minmax(0,1fr)] text-[14.5px] leading-relaxed text-muted">
            <span className="pt-[0.2em] font-mono text-[12.5px] text-accent" aria-hidden>
              {String(i + 1).padStart(2, "0")}
            </span>
            <div className="min-w-0 space-y-3">{step}</div>
          </li>
        ))}
      </ol>
    </div>
  );
}

function Door({
  id,
  kind,
  title,
  status,
  children,
}: {
  id: string;
  kind: string;
  title: string;
  status?: string;
  children: ReactNode;
}) {
  return (
    <article
      id={id}
      aria-labelledby={`${id}-title`}
      className="flex min-w-0 scroll-mt-24 flex-col rounded-xl border border-line-strong bg-surface p-6 sm:p-8"
    >
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
        <p className="font-mono text-[11.5px] uppercase tracking-[0.14em] text-subtle">{kind}</p>
        {status ? <Pill>{status}</Pill> : null}
      </div>
      <h3 id={`${id}-title`} className="mt-4 text-[26px] font-semibold tracking-[-0.025em] text-ink sm:text-[30px]">
        {title}
      </h3>
      {children}
    </article>
  );
}

const APP_DOES = "It installs the agentmark command, connects Chrome and adds the agent skill.";
const EXTENSION_ASKS = "It asks where bookmarks live and which agent writes summaries, then saves the page you are on.";

function ChromeDoor() {
  const store = INSTALL_LINKS.webStore;
  if (store) {
    return (
      <Door id="add-to-chrome" kind="Chrome extension" title="Add to Chrome">
        <p className="mt-3 text-[15px] leading-relaxed text-muted">Start in Chrome. The extension brings you the rest.</p>
        <div className="mt-6">
          <a href={store} className={BUTTON_SOLID}>
            <ExtensionGlyph />
            Add to Chrome
          </a>
        </div>
        <NextSteps
          steps={[
            "Click the AgentMark icon. If the app is missing, it shows how to install it on your computer.",
            "Install the app the way it shows, then click the icon again. The extension finds the app by itself.",
            EXTENSION_ASKS,
          ]}
        />
      </Door>
    );
  }
  return (
    <Door id="add-to-chrome" kind="Chrome extension" title="Add to Chrome" status="Not in the Chrome Web Store yet">
      <p className="mt-3 text-[15px] leading-relaxed text-muted">
        Until the listing is live, load the extension yourself. This is the developer path, and it works today.
      </p>
      <NextSteps
        title="Today"
        steps={[
          <p key="open">
            Open <Code>chrome://extensions</Code> and turn on <span className="text-ink">Developer mode</span>.
          </p>,
          <p key="load">
            Click <span className="text-ink">Load unpacked</span> and choose <Code>~/.agentmark/extension</Code>. The
            install command put it there.
          </p>,
          <p key="click">Click the AgentMark icon. {EXTENSION_ASKS}</p>,
        ]}
      />
    </Door>
  );
}

function MacDoor() {
  const app = INSTALL_LINKS.macApp;
  if (app) {
    return (
      <Door id="download-for-mac" kind="Mac app" title="Download for Mac">
        <p className="mt-3 text-[15px] leading-relaxed text-muted">Start with the app. It sets up everything outside Chrome.</p>
        <div className="mt-6 flex flex-wrap items-center gap-x-5 gap-y-3">
          <a href={app} className={INSTALL_LINKS.webStore ? BUTTON_OUTLINE : BUTTON_SOLID}>
            <DownloadGlyph />
            Download for Mac
          </a>
          <span className="text-[13px] text-subtle">
            Apple Silicon
            {INSTALL_LINKS.macIntelApp ? (
              <>
                {" · "}
                <a href={INSTALL_LINKS.macIntelApp} className="text-accent hover:underline">
                  Intel Mac
                </a>
              </>
            ) : null}
          </span>
        </div>
        <NextSteps
          steps={[
            `Open the app. ${APP_DOES}`,
            "It asks where bookmarks live and which agent writes summaries.",
            INSTALL_LINKS.webStore
              ? "It sends you to the Chrome Web Store for the extension. Add it, and you can save."
              : "Add the extension in Chrome with the developer path, then you can save.",
          ]}
        />
      </Door>
    );
  }
  return (
    <Door id="download-for-mac" kind="Mac app" title="Download for Mac" status="No download yet">
      <p className="mt-3 text-[15px] leading-relaxed text-muted">
        Until the app is published, one Terminal command installs the same pieces on a Mac with Apple Silicon.
      </p>
      <NextSteps
        title="Today"
        steps={[
          <>
            <p>Open Terminal and paste this:</p>
            <CopyCommand command={cliCommand()} label="Copy install command" wrap />
          </>,
          <p key="does">
            It installs the agentmark command to <Code>~/.agentmark/bin</Code>, unpacks the extension to{" "}
            <Code>~/.agentmark/extension</Code> and adds the agent skill. It asks no questions.
          </p>,
          <p key="then">
            {INSTALL_LINKS.webStore ? (
              <>Go back to Chrome. AgentMark finishes setting up there.</>
            ) : (
              <>
                Then add the extension in Chrome, as the{" "}
                <a href="#add-to-chrome" className="text-accent hover:underline">
                  Chrome card
                </a>{" "}
                shows.
              </>
            )}
          </p>,
        ]}
      />
    </Door>
  );
}

function PlatformDetail({ platform }: { platform: Platform }) {
  const method = installMethod(platform.key);
  if (method === "unsupported") {
    return <p>AgentMark does not run on Windows yet.</p>;
  }
  if (method === "app_download") {
    const command = platform.key === "mac_arm64" ? cliCommand() : fromSourceCommand();
    return (
      <>
        <p>
          <a href={downloadUrl(platform.key)} className="text-accent hover:underline">
            Download AgentMark for Mac
          </a>
          . Open the app; it does the rest.{" "}
          {platform.key === "mac_arm64" ? "Or use Terminal instead:" : "Or build it in Terminal, which needs Rust and Node.js:"}
        </p>
        <CopyCommand command={command} label={`Copy install command for ${platform.name}`} wrap />
      </>
    );
  }
  if (method === "cli_command") {
    return <CopyCommand command={cliCommand()} label={`Copy install command for ${platform.name}`} wrap />;
  }
  return (
    <>
      <p>
        There is no ready-made build for {platform.name} yet. The command builds AgentMark on your computer, which needs{" "}
        {SOURCE_PREREQUISITES.map((tool, i) => (
          <span key={tool.name}>
            {i > 0 ? " and " : null}
            <a href={tool.url} className="text-accent hover:underline">
              {tool.name}
            </a>
          </span>
        ))}
        .
      </p>
      <CopyCommand command={fromSourceCommand()} label={`Copy build command for ${platform.name}`} wrap />
    </>
  );
}

export function Install() {
  const storeFirst = Boolean(INSTALL_LINKS.webStore);
  const today = !INSTALL_LINKS.webStore && !INSTALL_LINKS.macApp;
  return (
    <Section id="install" labelledBy="install-title" className="bg-raised">
      <Eyebrow index="08">Install</Eyebrow>
      <H2 id="install-title" className="max-w-[44rem]">
        {today ? "Two ways in. Today, start with one command." : "Two ways in. Both end in Chrome."}
      </H2>
      <Lead className="max-w-[40rem]">
        {today
          ? "The two front doors are the Chrome Web Store and a Mac app. Neither is published yet, so today you install the app from Terminal and load the extension yourself. Setup still happens in Chrome."
          : "Start from the browser or from the app. Either way you end up with the extension, the app and the agent skill, set up without a terminal."}
      </Lead>

      <div className="mt-14 grid gap-5 lg:grid-cols-2">
        {storeFirst ? (
          <>
            <ChromeDoor />
            <MacDoor />
          </>
        ) : (
          <>
            <MacDoor />
            <ChromeDoor />
          </>
        )}
      </div>

      <div className="mt-20 grid gap-12 lg:grid-cols-[minmax(0,7fr)_minmax(0,4fr)] lg:gap-16">
        <div className="min-w-0">
          <h3 id="platforms-title" className="text-[19px] font-semibold tracking-tight text-ink">
            By computer
          </h3>
          <ul aria-labelledby="platforms-title" className="mt-5 divide-y divide-line border-y border-line">
            {PLATFORMS.map((p) => (
              <li key={p.key} className="grid gap-x-8 gap-y-3 py-6 sm:grid-cols-[11rem_minmax(0,1fr)]">
                <div>
                  <p className="text-[15px] font-medium text-ink">{p.name}</p>
                  <p className="mt-0.5 text-[13.5px] text-subtle">{METHOD_SUBLINE[installMethod(p.key)]}</p>
                </div>
                <div className="min-w-0 space-y-3 text-[14.5px] leading-relaxed text-muted">
                  <PlatformDetail platform={p} />
                </div>
              </li>
            ))}
          </ul>
        </div>

        <aside aria-labelledby="req-title" className="self-start rounded-xl border border-line-strong bg-surface p-6 sm:p-7">
          <h3 id="req-title" className="text-[15px] font-semibold text-ink">
            What you need
          </h3>
          <ul className="mt-4 space-y-3 text-[14.5px] leading-relaxed text-muted">
            <li>Google Chrome.</li>
            <li>For summaries: Claude Code, Codex or pi, signed in. Optional.</li>
            <li>
              For YouTube captions: <Code>yt-dlp</Code>. Optional.
            </li>
          </ul>
          <div className="mt-6 border-t border-line pt-5">
            <h3 className="text-[15px] font-semibold text-ink">Prefer the terminal?</h3>
            <p className="mt-2 text-[14.5px] leading-relaxed text-muted">
              <Code>agentmark init</Code> still asks the same questions in a terminal. Every CLI command works without
              the extension.
            </p>
          </div>
          <a
            href={REPO_URL}
            className="mt-6 inline-flex items-center gap-2 text-[14px] font-medium text-accent hover:underline"
          >
            <GitHubIcon />
            Read the README on GitHub
          </a>
        </aside>
      </div>
    </Section>
  );
}
