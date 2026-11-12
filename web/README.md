# AgentMark website

The marketing site for AgentMark: one static page at `/`, plus `llms.txt`, `robots.txt`, `sitemap.xml`, icons and an OpenGraph image.

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
- Every route is static. There are no API routes and no environment variables.
- The canonical URL is `https://agentmark.vercel.app` (`src/lib/site.ts`). Change it there if the domain changes.
- The extension links to `https://agentmark.vercel.app/#install` (`packages/extension/src/shared/connection.ts`), so keep the `#install` anchor.

## Rules

- `web/` is self-contained. Do not import anything from outside it.
- Copy must match the repo. Check every claim against the README, `install.sh`, the CLI and the extension. Do not invent numbers, quotes, users or features.
- Themes follow `prefers-color-scheme`. Tokens live in `src/app/globals.css`; both themes are written out by hand.
- The mark in `public/mark/` and `src/components/demos/kit/agentmark-mark.tsx` is copied from `packages/extension/icons/`. Do not redraw it.

## Layout

- `src/app/` — layout, page, metadata routes (`robots.ts`, `sitemap.ts`, `icon.svg`, `apple-icon.tsx`, `opengraph-image.tsx`).
- `src/components/site/` — header, footer, sections, copy-command block.
- `src/components/demos/` — the five product demos and their shared kit. See `src/components/demos/README.md`.
- `public/llms.txt` — plain-text summary for agents.
