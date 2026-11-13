# AgentMark website

The marketing site for AgentMark: a static page at `/`, the privacy policy at `/privacy`, plus `llms.txt`, `robots.txt`, `sitemap.xml`, icons and an OpenGraph image.

Stack: Next.js 16 (App Router), React 19, TypeScript (strict), Tailwind CSS 4, pnpm. No UI kit, no animation library.

## Run

```sh
pnpm install
pnpm dev          # http://localhost:3000
pnpm typecheck
pnpm lint
pnpm build && pnpm start
```

## Deploy (Vercel)

- Import the repo in Vercel and set **Root Directory** to `web`.
- Framework preset: Next.js. Install command: `pnpm install`. Build command: `pnpm build`.
- Every route is static. There are no API routes and no required environment variables.
- The canonical URL is `https://agentmark.app` (`SITE_URL` in `src/lib/site.ts`). Change it there if the domain changes.
- The extension's "Setup guide" link opens `https://agentmark.app/#install` (`SETUP_GUIDE_URL` in `packages/extension/src/shared/constants.ts`), so keep the `#install` anchor.

## Install links

The install section has two doors, "Add to Chrome" and "Download for Mac". They read three constants in `src/lib/site.ts`, named as in `docs/setup-contract.md` §7.1 and the extension's `constants.ts`:

- `WEB_STORE_URL`: the Chrome Web Store listing.
- `MAC_APP_DOWNLOAD_URL`: the Mac app for Apple Silicon.
- `MAC_INTEL_APP_DOWNLOAD_URL`: the Mac app for Intel.

An empty string means the thing does not exist yet. The door then shows the path that works today (the Terminal command, the unpacked extension) instead of a link. Set a URL and rebuild; nothing else changes.

To preview the page with all three set, start or build with `NEXT_PUBLIC_AGENTMARK_PREVIEW_INSTALL_URLS=1` (for example `NEXT_PUBLIC_AGENTMARK_PREVIEW_INSTALL_URLS=1 pnpm dev`). It swaps in placeholder addresses. Never set it in a deploy.

`src/lib/platforms.ts` is the site's copy of the extension's platform table (`packages/extension/src/shared/platforms.ts`, canonical in `docs/setup-contract.md` §7.3). Change both together.

## Privacy policy

`/privacy` renders `src/content/privacy-policy.md`, a copy of `docs/chrome-web-store/privacy-policy.md`. Keep the two the same. Text that starts with `OWNER:` is a placeholder; the page shows it as a visible "To fill in" marker and a draft note until it is replaced.

## Rules

- `web/` is self-contained. Do not import anything from outside it.
- Copy must match the repo. Check every claim against the README, `install.sh`, the CLI, the extension and the app. Never say "open source": the repo has no licence. Do not invent numbers, quotes, users or features.
- Themes follow `prefers-color-scheme`. Tokens live in `src/app/globals.css`; both themes are written out by hand.
- The mark in `public/mark/` and `src/components/demos/kit/agentmark-mark.tsx` is copied from `packages/extension/icons/`. Do not redraw it.

## Layout

- `src/app/` — layout, home page, `privacy/page.tsx`, metadata routes (`robots.ts`, `sitemap.ts`, `icon.svg`, `apple-icon.tsx`, `opengraph-image.tsx`).
- `src/components/site/` — header, footer, sections, copy-command block.
- `src/components/demos/` — the six product demos and their shared kit. See `src/components/demos/README.md`.
- `src/lib/` — `site.ts` (URLs and constants), `platforms.ts` (install table and commands), `policy-markdown.tsx` (the small renderer for the policy).
- `src/content/privacy-policy.md` — the policy text.
- `public/llms.txt` — plain-text summary for agents.
