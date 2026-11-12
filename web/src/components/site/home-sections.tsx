import type { ReactNode } from "react";
import Link from "next/link";
import HeroSaveDemo from "@/components/demos/hero-save-demo";
import FeedBadgesDemo from "@/components/demos/feed-badges-demo";
import BundleViewerDemo from "@/components/demos/bundle-viewer-demo";
import TerminalDemo from "@/components/demos/terminal-demo";
import SidepanelDemo from "@/components/demos/sidepanel-demo";
import FirstRunDemo from "@/components/demos/first-run-demo";
import { AgentMarkMark, BUNDLE_FILES, type MarkLook } from "@/components/demos/kit";
import { INSTALL_LINKS, cliCommand } from "@/lib/platforms";
import { REPO_URL } from "@/lib/site";
import { CopyCommand } from "./copy-command";
import { BUTTON_OUTLINE, BUTTON_SOLID, DownloadGlyph, ExtensionGlyph } from "./install-section";
import { ArrowIcon, GitHubIcon } from "./icons";
import { Code, Container, Eyebrow, Facts, Feature, H2, Lead, Section } from "./section";

/* ------------------------------------------------------------------ Hero */

export function Hero() {
  return (
    <section id="top" aria-labelledby="hero-title" className="pt-14 pb-20 sm:pt-20 sm:pb-28">
      <Container>
        <p className="font-mono text-[12px] uppercase tracking-[0.14em] text-subtle">
          Chrome extension · CLI · Agent skill
        </p>
        <h1
          id="hero-title"
          className="mt-5 max-w-[62rem] text-balance text-[2.75rem] font-semibold leading-[1.0] tracking-[-0.04em] text-ink sm:text-[4rem] lg:text-[5rem]"
        >
          Bookmarks your agent can actually read.
        </h1>
        <div className="mt-10 grid gap-14 xl:mt-14 xl:grid-cols-[minmax(0,5fr)_minmax(0,7fr)] xl:gap-14">
          <div className="min-w-0 xl:pt-1">
            <p className="max-w-[34rem] text-pretty text-[18px] leading-relaxed text-muted sm:text-[20px]">
              AgentMark saves each page as plain files on your own machine and indexes them for search. Claude Code and
              Codex find them and read them back when you ask.
            </p>
            {INSTALL_LINKS.webStore || INSTALL_LINKS.macApp ? (
              <div className="mt-9 flex flex-wrap items-center gap-x-3 gap-y-3">
                {INSTALL_LINKS.webStore ? (
                  <a href={INSTALL_LINKS.webStore} className={BUTTON_SOLID}>
                    <ExtensionGlyph />
                    Add to Chrome
                  </a>
                ) : null}
                {INSTALL_LINKS.macApp ? (
                  <a href={INSTALL_LINKS.macApp} className={INSTALL_LINKS.webStore ? BUTTON_OUTLINE : BUTTON_SOLID}>
                    <DownloadGlyph />
                    Download for Mac
                  </a>
                ) : null}
                <p className="basis-full pt-2">
                  <a href="#install" className="inline-flex items-center gap-1.5 text-[14px] font-medium text-accent hover:underline">
                    All install options
                    <ArrowIcon />
                  </a>
                </p>
              </div>
            ) : (
              <>
                <div className="mt-9 max-w-[40rem]">
                  <CopyCommand command={cliCommand()} label="Copy install command" size="lg" wrap />
                </div>
                <div className="mt-5 flex flex-wrap items-center gap-x-6 gap-y-3">
                  <a href={REPO_URL} className={BUTTON_SOLID}>
                    <GitHubIcon />
                    View on GitHub
                  </a>
                  <a href="#install" className="inline-flex items-center gap-1.5 text-[14px] font-medium text-accent hover:underline">
                    Install steps
                    <ArrowIcon />
                  </a>
                </div>
              </>
            )}
            <p className="mt-6 max-w-[34rem] text-[13.5px] leading-relaxed text-subtle">
              {INSTALL_LINKS.macApp && INSTALL_LINKS.macIntelApp
                ? "The Mac app sets up the rest on any Mac. Linux builds from source."
                : INSTALL_LINKS.macApp
                  ? "The Mac app is for Apple Silicon. Intel Macs and Linux build from source."
                  : "One command on macOS with Apple Silicon. Intel Macs and Linux build from source."}{" "}
              Needs Google Chrome. Setup happens in Chrome, not in a terminal.
            </p>
          </div>
          <div className="min-w-0">
            <HeroSaveDemo />
          </div>
        </div>
      </Container>
    </section>
  );
}

/* --------------------------------------------------------------- Problem */

function Specimen({ children }: { children: ReactNode }) {
  return (
    <div className="flex flex-col justify-center gap-2 rounded-lg border border-line bg-surface px-4 py-5 font-mono text-[12px] md:h-[7.5rem] md:py-0">
      {children}
    </div>
  );
}

export function Problem() {
  return (
    <Section id="why" labelledBy="why-title">
      <Eyebrow index="01">The problem</Eyebrow>
      <H2 id="why-title" className="max-w-[48rem]">
        A bookmark is a link. Your agent needs the page.
      </H2>
      <Lead className="max-w-[40rem]">
        Browser bookmarks keep a title and a URL, and you seldom open them again. Read-later services keep the text, but
        on their servers, behind their account. Neither puts the page where your agent works.
      </Lead>

      <div className="mt-14 grid gap-5 md:grid-cols-3">
        <figure className="flex flex-col gap-4">
          <Specimen>
            <p className="truncate text-ink">Plain text outlives every app</p>
            <p className="truncate text-subtle">https://notes.example.com/plain-te…</p>
          </Specimen>
          <figcaption>
            <p className="text-[15px] font-medium text-ink">Browser bookmark</p>
            <p className="mt-1 text-[14.5px] leading-relaxed text-muted">
              A title and a URL. If the page changes or goes away, so does what you saved.
            </p>
          </figcaption>
        </figure>
        <figure className="flex flex-col gap-4">
          <Specimen>
            <p className="truncate text-ink">readlater.example/your-list</p>
            <p className="truncate text-subtle">stored on their servers</p>
          </Specimen>
          <figcaption>
            <p className="text-[15px] font-medium text-ink">Read-later service</p>
            <p className="mt-1 text-[14.5px] leading-relaxed text-muted">
              The text is kept, but in someone else&apos;s cloud and behind an account with them.
            </p>
          </figcaption>
        </figure>
        <figure className="flex flex-col gap-4">
          <div className="flex flex-col justify-center gap-1 rounded-lg border border-accent-line bg-accent-soft px-4 py-5 font-mono text-[12px] md:h-[7.5rem] md:py-0">
            <p className="truncate text-accent">~/Documents/AgentMark/2026/11/12/plain-text-…/</p>
            <p className="truncate text-muted">
              ├ article.md <span className="text-subtle">· bookmark.md</span>
            </p>
            <p className="truncate text-muted">
              └ source.html <span className="text-subtle">· metadata.json …</span>
            </p>
          </div>
          <figcaption>
            <p className="flex items-center gap-2 text-[15px] font-medium text-ink">
              <AgentMarkMark look="saved" size={16} />
              AgentMark
            </p>
            <p className="mt-1 text-[14.5px] leading-relaxed text-muted">
              The page itself, as markdown and HTML in a folder you own. You and your agent search it the same way.
            </p>
          </figcaption>
        </figure>
      </div>
    </Section>
  );
}

/* --------------------------------------------------------------- Capture */

const LOOKS: Array<{ look: MarkLook; label: string }> = [
  { look: "idle", label: "Not saved" },
  { look: "saving", label: "Saving" },
  { look: "saved-fresh", label: "Just saved" },
  { look: "saved", label: "Saved" },
  { look: "error", label: "Failed" },
];

export function Capture() {
  return (
    <Section id="capture" labelledBy="capture-title">
      <Feature
        copy={
          <>
            <Eyebrow index="02">Capture</Eyebrow>
            <H2 id="capture-title">Save what you see, even behind a login.</H2>
            <Lead>
              The extension sends the page as Chrome shows it to you. A LinkedIn profile, which blocks a plain fetch
              with HTTP 999, saves with its real content.
            </Lead>
            <Facts
              items={[
                {
                  title: "A button per post",
                  body: "On LinkedIn, X, Hacker News and Reddit every post gets its own save button. Posts you saved before show as saved.",
                },
                {
                  title: "An honest icon",
                  body: "The toolbar icon turns blue when you save. One optional switch lets it show pages you saved before, on every site. Blue only ever means saved.",
                },
                {
                  title: "Videos, with words",
                  body: (
                    <>
                      YouTube videos save with their chapters and captions when <Code>yt-dlp</Code> is installed.
                    </>
                  ),
                },
                {
                  title: "One shortcut",
                  body: (
                    <>
                      <Code>Cmd+Shift+S</Code> saves the current tab. <Code>Cmd+Shift+B</Code> opens the side panel.
                    </>
                  ),
                },
              ]}
            />
            <ul className="mt-8 flex max-w-[34rem] flex-wrap gap-x-5 gap-y-3" aria-label="Toolbar icon states">
              {LOOKS.map((l) => (
                <li key={l.look} className="flex items-center gap-2 text-[12.5px] text-muted">
                  <AgentMarkMark look={l.look} size={18} />
                  {l.label}
                </li>
              ))}
            </ul>
          </>
        }
        demo={<FeedBadgesDemo />}
      />
    </Section>
  );
}

/* ---------------------------------------------------------------- Bundle */

export function Bundle() {
  return (
    <Section id="bundle" labelledBy="bundle-title">
      <Feature
        flip
        copy={
          <>
            <Eyebrow index="03">On disk</Eyebrow>
            <H2 id="bundle-title">Every save is a folder of plain files.</H2>
            <Lead>
              Each bookmark gets its own dated folder in the storage path you choose. Open it in your editor, grep it,
              back it up. A local SQLite full-text index makes the whole collection searchable.
            </Lead>
            <dl className="mt-10 max-w-[34rem] divide-y divide-line border-y border-line">
              {BUNDLE_FILES.map((f) => (
                <div key={f.name} className="flex items-baseline justify-between gap-6 py-3">
                  <dt className="font-mono text-[13.5px] text-ink">{f.name}</dt>
                  <dd className="text-right text-[14px] text-muted">{f.note}</dd>
                </div>
              ))}
            </dl>
            <p className="mt-5 max-w-[34rem] text-[14px] leading-relaxed text-subtle">
              Path pattern: <Code>{"<storage>/YYYY/MM/DD/<slug>-<id>/"}</Code>
            </p>
          </>
        }
        demo={<BundleViewerDemo />}
      />
    </Section>
  );
}

/* ---------------------------------------------------------------- Agents */

const COMMANDS: Array<[string, string]> = [
  ['agentmark search "query"', "Full-text search across everything you saved"],
  ["agentmark show <id> --full", "Summary, tags and the extracted article"],
  ["agentmark list --tag <tag>", "Filter by tag, collection or state"],
  ["agentmark save <url>", "Save a link from the conversation"],
  ["agentmark tag <id> <tags…>", "Add or remove tags"],
];

export function Agents() {
  return (
    <Section id="agents" labelledBy="agents-title">
      <Feature
        copy={
          <>
            <Eyebrow index="04">For your agent</Eyebrow>
            <H2 id="agents-title">Ask your agent what you saved.</H2>
            <Lead>
              Installing AgentMark adds an agent skill for Claude Code and Codex. Mention your bookmarks and the agent
              uses the CLI: it searches, picks a match and reads the article.
            </Lead>
            <ul className="mt-10 max-w-[34rem] divide-y divide-line border-y border-line">
              {COMMANDS.map(([cmd, what]) => (
                <li key={cmd} className="grid gap-1 py-3.5 sm:grid-cols-[15rem_1fr] sm:gap-5">
                  <code className="font-mono text-[13px] text-ink">{cmd}</code>
                  <span className="text-[14px] leading-relaxed text-muted">{what}</span>
                </li>
              ))}
            </ul>
            <p className="mt-5 max-w-[34rem] text-[14px] leading-relaxed text-subtle">
              The same commands work for you in any terminal.
            </p>
          </>
        }
        demo={<TerminalDemo />}
      />
    </Section>
  );
}

/* --------------------------------------------------------------- Writeup */

export function Writeup() {
  return (
    <Section id="writeup" labelledBy="writeup-title">
      <Feature
        flip
        copy={
          <>
            <Eyebrow index="05">Write-up</Eyebrow>
            <H2 id="writeup-title">The summary arrives after you move on.</H2>
            <Lead>
              After a save, AgentMark runs the agent CLI you already use, Claude Code, Codex or pi, in the background.
              It writes a summary and suggests tags. The popup never waits on a model.
            </Lead>
            <Facts
              items={[
                {
                  title: "No API key",
                  body: "AgentMark runs your local agent CLI. It needs no key of its own.",
                },
                {
                  title: "Hard pages, clean text",
                  body: "Pages saved from the browser's copy, like LinkedIn posts, X threads, Hacker News discussions and Reddit threads, get turned into readable markdown.",
                },
                {
                  title: "Tags stay yours",
                  body: "Suggested tags wait in the side panel until you accept or reject them.",
                },
                {
                  title: "Your rules",
                  body: "Give summaries your own instructions in the side panel's Settings, or turn them off with one switch.",
                },
              ]}
            />
          </>
        }
        demo={<SidepanelDemo />}
      />
    </Section>
  );
}

/* ----------------------------------------------------------------- Local */

const LOCAL_POINTS: Array<{ title: string; body: ReactNode }> = [
  { title: "No account", body: "Install it and save. There is nothing to sign up for." },
  { title: "No sync", body: "Bundles stay in the folder you pick. Copy or back them up like any other files." },
  { title: "No API key", body: "Summaries run through the agent CLI you already have signed in." },
  {
    title: "Your agent, your terms",
    body: "With summaries on, your agent CLI sends the page text to its model under your account. Turn them off and no model sees it.",
  },
];

export function Local() {
  return (
    <Section id="local" labelledBy="local-title">
      <div className="grid gap-12 lg:grid-cols-[minmax(0,5fr)_minmax(0,6fr)] lg:gap-16">
        <div>
          <Eyebrow index="06">Privacy</Eyebrow>
          <H2 id="local-title">There is no AgentMark server.</H2>
          <Lead>
            The app, the index and every bundle live on your computer. The extension talks only to the AgentMark app,
            through Chrome&apos;s native messaging, and the app answers on the same machine.
          </Lead>
          <Link href="/privacy" className="mt-8 inline-flex items-center gap-1.5 text-[14px] font-medium text-accent hover:underline">
            Read the privacy policy
            <ArrowIcon />
          </Link>
        </div>
        <aside
          aria-labelledby="permission-title"
          className="self-start rounded-xl border border-line-strong bg-surface p-6 sm:p-7"
        >
          <p className="font-mono text-[11.5px] uppercase tracking-[0.14em] text-subtle">About one permission</p>
          <h3 id="permission-title" className="mt-3 text-[19px] font-semibold tracking-tight text-ink">
            &ldquo;Read your browsing history&rdquo; is optional, and off until you ask for it.
          </h3>
          <p className="mt-3 text-[15px] leading-relaxed text-muted">
            Out of the box, the toolbar icon turns blue when you save, and on saved LinkedIn, X, Hacker News and Reddit
            pages. Installing the extension does not ask for your browsing history.
          </p>
          <p className="mt-3 text-[15px] leading-relaxed text-muted">
            To see saved pages on every site, turn on{" "}
            <span className="text-ink">&ldquo;Show when the current page is already saved&rdquo;</span> in Settings.
            Chrome then asks for the <Code>tabs</Code> permission. The extension sends the current tab&apos;s address to
            the app on your computer, which answers and does not record it. Turn it off at any time.
          </p>
        </aside>
      </div>
      <ul className="mt-14 grid gap-px overflow-hidden rounded-xl border border-line bg-line sm:grid-cols-2 lg:grid-cols-4">
        {LOCAL_POINTS.map((p) => (
          <li key={p.title} className="bg-bg p-6">
            <p className="text-[15px] font-medium text-ink">{p.title}</p>
            <p className="mt-2 text-pretty text-[14.5px] leading-relaxed text-muted">{p.body}</p>
          </li>
        ))}
      </ul>
    </Section>
  );
}

/* ----------------------------------------------------------------- Setup */

export function Setup() {
  return (
    <Section id="setup" labelledBy="setup-title">
      <Feature
        copy={
          <>
            <Eyebrow index="07">First run</Eyebrow>
            <H2 id="setup-title">Setup happens in Chrome, not in a terminal.</H2>
            <Lead>
              Click the AgentMark icon. If the app is not on your computer yet, the extension shows how to install it,
              waits for it, then asks two questions and saves the page you are on.
            </Lead>
            <Facts
              items={[
                {
                  title: "Your computer first",
                  body: "The platform Chrome runs on sits at the top, with the one way to install there. The others are listed below it.",
                },
                {
                  title: "It notices the app",
                  body: "While the popup is open it checks for the app every two seconds. There is a Check again button too.",
                },
                {
                  title: "Two choices",
                  body: (
                    <>
                      A folder, <Code>~/Documents/AgentMark</Code> unless you pick another, and the agent that writes
                      summaries. Summaries are one switch.
                    </>
                  ),
                },
                {
                  title: "Change it later",
                  body: "Settings in the side panel holds the folder, the agent and your summary instructions. Changing the folder does not move bookmarks you already saved.",
                },
              ]}
            />
          </>
        }
        demo={<FirstRunDemo />}
      />
    </Section>
  );
}

/* --------------------------------------------------------------- Install */

export { Install } from "./install-section";
