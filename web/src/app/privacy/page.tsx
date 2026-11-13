import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { Metadata } from "next";
import { SiteFooter } from "@/components/site/site-footer";
import { SiteHeader } from "@/components/site/site-header";
import { Container } from "@/components/site/section";
import { Inline, ownerPlaceholders, parsePolicy, slugify, type Block } from "@/lib/policy-markdown";

/*
 * The privacy policy. The text lives in src/content/privacy-policy.md, a copy
 * of docs/chrome-web-store/privacy-policy.md, read at build time.
 *
 * TODO(owner): the policy still has "OWNER:" placeholders (publish date,
 * hosting provider, contact). Replace them in src/content/privacy-policy.md;
 * the draft note below disappears once none are left.
 */

const DESCRIPTION =
  "What data AgentMark handles, where it goes and what you control. AgentMark does not operate a server; your bookmarks are files on your computer.";

export const metadata: Metadata = {
  title: "Privacy policy · AgentMark",
  description: DESCRIPTION,
  alternates: { canonical: "/privacy" },
  openGraph: { type: "article", url: "/privacy", title: "AgentMark privacy policy", description: DESCRIPTION },
};

type Section = { title: string; id: string; blocks: Block[] };

function loadPolicy() {
  const source = readFileSync(join(process.cwd(), "src/content/privacy-policy.md"), "utf8");
  const blocks = parsePolicy(source);
  let title = "AgentMark privacy policy";
  const intro: Block[] = [];
  const sections: Section[] = [];
  for (const block of blocks) {
    if (block.kind === "h1") title = block.text;
    else if (block.kind === "h2") sections.push({ title: block.text, id: slugify(block.text), blocks: [] });
    else if (sections.length) sections[sections.length - 1].blocks.push(block);
    else intro.push(block);
  }
  const updated = intro.find((b) => b.kind === "p" && b.text.startsWith("Last updated:"));
  return {
    title,
    updated: updated?.kind === "p" ? updated.text.slice("Last updated:".length).trim() : null,
    intro: intro.filter((b) => b !== updated),
    sections,
    placeholders: ownerPlaceholders(blocks),
  };
}

export default function PrivacyPage() {
  const policy = loadPolicy();
  const [summary, ...rest] = policy.sections;
  const short = summary?.title === "The short version" ? summary : null;
  const sections = short ? rest : policy.sections;

  return (
    <>
      <SiteHeader />
      <main id="main" className="pt-14 pb-24 sm:pt-20 sm:pb-32">
        <Container className="lg:grid lg:grid-cols-[minmax(0,44rem)_13rem] lg:justify-between lg:gap-16">
          <div className="min-w-0">
            <header>
              <p className="font-mono text-[12px] uppercase tracking-[0.14em] text-subtle">Privacy</p>
              <h1 className="mt-5 text-balance text-[2.4rem] font-semibold leading-[1.04] tracking-[-0.035em] text-ink sm:text-[3.2rem]">
                {policy.title}
              </h1>
              {policy.updated ? (
                <p className="mt-5 text-[14px] text-subtle">
                  Last updated: <Inline text={policy.updated} />
                </p>
              ) : null}
              {policy.placeholders.length > 0 ? (
                <p className="mt-6 rounded-lg border border-dashed border-line-strong bg-sunken px-4 py-3 text-[13.5px] leading-relaxed text-muted">
                  Draft. {policy.placeholders.length} {policy.placeholders.length === 1 ? "detail is" : "details are"} still
                  to be filled in, marked below.
                </p>
              ) : null}
              <div className="mt-8 space-y-5 text-pretty text-[17px] leading-relaxed text-muted sm:text-[18px]">
                {policy.intro.map((b, i) => (
                  <BlockView key={i} block={b} />
                ))}
              </div>
            </header>

            {short ? (
              <section aria-labelledby={short.id} className="mt-12 rounded-xl border border-line-strong bg-surface p-6 sm:p-8">
                <h2 id={short.id} className="font-mono text-[11.5px] uppercase tracking-[0.14em] text-subtle">
                  {short.title}
                </h2>
                <div className="mt-4 text-[15.5px] leading-relaxed text-ink">
                  {short.blocks.map((b, i) => (
                    <BlockView key={i} block={b} tight />
                  ))}
                </div>
              </section>
            ) : null}

            <div className="mt-14 divide-y divide-line border-t border-line">
              {sections.map((s) => (
                <section key={s.id} aria-labelledby={s.id} className="scroll-mt-24 py-9">
                  <h2 id={s.id} className="text-[21px] font-semibold tracking-[-0.015em] text-ink sm:text-[23px]">
                    {s.title}
                  </h2>
                  <div className="mt-4 space-y-4 text-pretty text-[15.5px] leading-relaxed text-muted sm:text-[16px]">
                    {s.blocks.map((b, i) => (
                      <BlockView key={i} block={b} />
                    ))}
                  </div>
                </section>
              ))}
            </div>
          </div>

          <nav aria-label="On this page" className="hidden lg:block">
            <div className="sticky top-24 pt-2">
              <p className="font-mono text-[11.5px] uppercase tracking-[0.14em] text-subtle">On this page</p>
              <ul className="mt-4 space-y-2 border-l border-line pl-4 text-[13px]">
                {sections.map((s) => (
                  <li key={s.id}>
                    <a href={`#${s.id}`} className="text-muted transition-colors hover:text-ink">
                      {s.title}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          </nav>
        </Container>
      </main>
      <SiteFooter />
    </>
  );
}

function BlockView({ block, tight = false }: { block: Block; tight?: boolean }) {
  if (block.kind === "ul") {
    return (
      <ul className={tight ? "space-y-2.5" : "space-y-2"}>
        {block.items.map((item, i) => (
          <li key={i} className="relative pl-5">
            <span aria-hidden className="absolute left-0 top-[0.72em] h-px w-2.5 bg-line-strong" />
            <Inline text={item} />
          </li>
        ))}
      </ul>
    );
  }
  if (block.kind === "p") {
    return (
      <p>
        <Inline text={block.text} />
      </p>
    );
  }
  return null;
}
