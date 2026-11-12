import { AgentMarkMark } from "@/components/demos/kit";
import { REPO_URL } from "@/lib/site";
import { Container } from "./section";

export function SiteFooter() {
  return (
    <footer className="border-t border-line py-12">
      <Container className="flex flex-col gap-8 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <p className="flex items-center gap-2.5 text-[15px] font-semibold tracking-tight text-ink">
            <AgentMarkMark look="brand" size={20} />
            AgentMark
          </p>
          <p className="mt-2 max-w-sm text-[13.5px] leading-relaxed text-muted">
            Local bookmarks for you and your coding agent.
          </p>
        </div>
        <div className="flex flex-col gap-2 text-[13px] text-subtle sm:items-end">
          <ul className="flex flex-wrap gap-x-5 gap-y-2">
            <li>
              <a href={REPO_URL} className="text-muted hover:text-ink">
                Source on GitHub
              </a>
            </li>
            <li>
              <a href={`${REPO_URL}/issues`} className="text-muted hover:text-ink">
                Issues
              </a>
            </li>
            <li>
              <a href="/llms.txt" className="text-muted hover:text-ink">
                llms.txt
              </a>
            </li>
          </ul>
          <p>Bookmark icon from the Noun Project.</p>
        </div>
      </Container>
    </footer>
  );
}
