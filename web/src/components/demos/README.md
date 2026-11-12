# Demos

Five demo slots, each a default-exported client component with no props. The page renders them as-is; build a demo by replacing the file's body, not by editing the page.

| File | Story | Root size (keep it) | Column width |
| --- | --- | --- | --- |
| `hero-save-demo.tsx` | Toolbar icon idle, click, popup "Saving" (hollow light blue), "Saved" with tick, icon settles to plain blue | `h-[380px] sm:h-[460px] xl:h-[500px]` | 100% below 1280px; ~650px at 1440 |
| `feed-badges-demo.tsx` | Generic feed, badge per post, saved posts already blue, one gets saved | `h-[440px] sm:h-[480px]` | 100% below 1024px; ~600px at 1440 |
| `bundle-viewer-demo.tsx` | One bundle's file tree, preview each file | `h-[460px] sm:h-[480px]` | same |
| `terminal-demo.tsx` | `agentmark search` then `agentmark show`, or an agent using the skill | `h-[460px] sm:h-[480px]` | same |
| `sidepanel-demo.tsx` | Side panel: summary and suggested tags arrive after the save | `h-[460px] sm:h-[500px]` | same |

Narrowest layout is 360px viewport, so the demo gets about 320px. Test there.

## Kit (`./kit`)

Import from `./kit`:

- `BrowserFrame` — browser window. Props: `url`, `tabTitle`, `markLook`, `extension` (replace the toolbar icon, e.g. with a button), `overlay` (popup/panel layer anchored top-right), `label`, `bodyClassName`. Fills the parent height.
- `WindowFrame` — app window with a title bar. Props: `title`, `actions`, `label`, `bodyClassName`.
- `TerminalFrame` — dark terminal in both themes. Props: `title`, `label`, `bodyClassName`. Body is mono 12.5/13px.
- `AgentMarkMark` — the mark. `look`: `"idle" | "saving" | "saved-fresh" | "saved" | "error" | "brand"`, `size` (px), `label` (string, or `true` for the default state name; omit to hide from assistive tech). Paths are copied from `packages/extension/icons/`. Never redraw or recolour them. Blue only ever means saved.
- `useInView({ threshold, rootMargin, once })` — `{ ref, inView }`. Pause loops when off screen.
- `usePrefersReducedMotion()` — true when the user asks for reduced motion. It is false during server render, so the server markup must be the settled, final state.
- `SAMPLE`, `BUNDLE_FILES` — shared fictional content (example.com, invented names) so demos tell one story.
- `MockArticle` (`./kit/mock-article`) — the article page used in browser demos.
- `cx(...)` — class name join.

## Tokens

Tailwind utilities map to CSS variables in `src/app/globals.css`. Use these, never raw colours (the mark is the one exception).

| Utility | Variable | Use |
| --- | --- | --- |
| `bg-bg` | `--bg` | Page |
| `bg-surface` | `--surface` | Window body, cards |
| `bg-raised` | `--raised` | Side panels, secondary panes |
| `bg-sunken` | `--sunken` | Wells, skeleton lines, feed background |
| `bg-code-bg` | `--code-bg` | Inline code |
| `text-ink` / `text-muted` / `text-subtle` | `--ink` / `--muted` / `--subtle` | Text, strongest to weakest (all AA on bg/surface) |
| `border-line` / `border-line-strong` | `--line` / `--line-strong` | Hairlines, frame borders |
| `text-accent` | `--accent` | Accent text and links (#4f46e5 light, #9ea4ff dark) |
| `bg-accent-solid` + `text-accent-solid-ink` | `--accent-solid` | Filled accent (#4f46e5 both themes) |
| `bg-accent-soft` / `border-accent-line` | `--accent-soft` / `--accent-line` | Tinted selection, chips |
| `text-danger` | `--danger` | Errors only |
| `bg-chrome` / `text-chrome-ink` | `--chrome` / `--chrome-ink` | Window title bars |
| `bg-term-bg`, `text-term-ink`, `text-term-muted`, `text-term-accent`, `text-term-ok`, `border-term-line` | `--term-*` | Terminal |
| `shadow-frame` | `--shadow-frame` | Window shadow |

## Rules

- Client components only, no props, default export. Keep the root size from the table so nothing shifts when the demo mounts.
- No new dependencies. CSS transitions plus React state and timers are enough.
- Theme-aware: tokens only. Check both colour schemes.
- Reduced motion: show the final state, no looping, no movement. Start animating only when `inView` is true and stop when it is false.
- Keyboard: anything clickable is a real `button` with a visible focus ring. A demo that loops needs a pause control or must stop after one run.
- Accessible: give the frame a `label` describing what the demo shows, or mark decorative motion `aria-hidden` and keep a text equivalent.
- Content stays generic: no third-party logos, layouts that copy a real site, or real people. Formats (IDs, list rows, `show` output, bundle files) must match the real CLI.
