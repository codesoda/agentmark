import Link from "next/link";
import { REPO_URL } from "@/lib/site";
import { Container } from "./section";
import { GitHubIcon } from "./icons";
import { HeaderBrand } from "./header-brand";

const LINKS = [
  { href: "/#capture", label: "Capture" },
  { href: "/#bundle", label: "On disk" },
  { href: "/#agents", label: "Agents" },
  { href: "/#local", label: "Privacy" },
  { href: "/#install", label: "Install" },
];

export function SiteHeader() {
  return (
    <header className="sticky top-0 z-40 border-b border-line bg-bg">
      <Container className="flex h-15 items-center gap-6">
        <HeaderBrand />
        <nav aria-label="Sections" className="ml-auto hidden md:block">
          <ul className="flex items-center gap-1">
            {LINKS.map((l) => (
              <li key={l.href}>
                <Link href={l.href} className="rounded-md px-3 py-2 text-[13.5px] text-muted transition-colors hover:text-ink">
                  {l.label}
                </Link>
              </li>
            ))}
          </ul>
        </nav>
        <a
          href={REPO_URL}
          className="ml-auto inline-flex items-center gap-2 rounded-md border border-line-strong bg-surface px-3 py-1.5 text-[13px] font-medium text-ink transition-colors hover:bg-raised md:ml-2"
        >
          <GitHubIcon />
          GitHub
        </a>
      </Container>
    </header>
  );
}
