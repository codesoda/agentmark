# Graphic assets

Sizes from developer.chrome.com ("Supplying images", "Product details"): store icon 128x128 PNG; 1 to 5 screenshots at 1280x800 (or 640x400), square corners, full bleed; small promo tile 440x280 PNG or JPEG (optional, but items without one are listed after items with one); marquee 1400x560 (optional, not made).

All UI in the screenshots is the real extension code (`packages/extension/src`, this working tree) rendered in a browser by a small harness that fakes the Chrome APIs and the app's answers. The bookmark titles, URLs (example.com/.org/.net), summary, note, bookmark count (42) and version (0.3.0) are sample data, not real people or real usage. No third-party logos. The mark is `packages/extension/icons/icon.svg`, unchanged.

Every file was opened and checked after rendering.

| File | Size | Bytes | Shows | Upload as |
|---|---|---|---|---|
| `assets/store-icon-128.png` | 128x128 | 1,305 | `icons/icon.svg` at 96x96 centred on a transparent 128x128 canvas (the 16 px padding the store asks for) | Store icon |
| `assets/screenshot-1-save.png` | 1280x800 | 221,302 | Popup after a save ("Saved!", title, ID, "More options") and the "Save with intent" form (tags, collection, note, action prompt). Caption: "Save the page you are on" | Screenshot 1 |
| `assets/screenshot-2-side-panel.png` | 1280x800 | 238,251 | Side panel bookmark list (All / Inbox / Processed / Archived) and a bookmark's detail (summary, tags, suggested tags, note, collection, "Mark Processed"). Caption: "Find it again in the side panel" | Screenshot 2 |
| `assets/screenshot-3-install-walkthrough.png` | 1280x800 | 228,747 | Screen A "Finish setting up AgentMark" with Mac Apple Silicon as "Your computer", and screen B "Install on Mac Apple Silicon" with the Terminal command and "Waiting for the app…". Caption: "Walks you through installing the app" | Screenshot 3 |
| `assets/screenshot-4-setup.png` | 1280x800 | 257,431 | Screen C "Set up AgentMark": folder, agent list with Found / Not installed, the summary switch with its provider notice, "Finish and save this page". Caption: "Two choices and you are done" | Screenshot 4 |
| `assets/screenshot-5-settings.png` | 1280x800 | 266,590 | Settings view in two parts: App, Storage, Summaries, Agent; then instructions, "Capture the page from the browser on" (built-in sites incl. reddit.com plus one user site), and the off-by-default toolbar icon switch. Caption: "Settings stay in the side panel" | Screenshot 5 |
| `assets/small-promo-tile-440x280.png` | 440x280 | 31,456 | The mark, "AgentMark", "Save web pages as files your coding agents can read", on dark indigo | Small promo tile |

## Known limits of these images

- Screenshot 3 uses the store ID (`WEB_STORE_EXTENSION_ID`) as the harness's extension ID, so it shows the short command a store install shows. A copy with any other ID adds `--extension-id <its ID>`.
- Version "0.3.0" in screenshots 4 and 5 is sample data from the harness (the branch targets 0.3.0; the working tree is 0.2.6).
- Rendered at 1x device pixels with the extension pages scaled up 1.0 to 1.4 times by CSS. Text stays sharp, but the store downscales to 640x400 for display; screenshot 5 has the smallest text.
- The images reflect the UI on the day of rendering, including the Reddit text another builder was adding. Re-render if the popup, side panel or Settings change before upload.
- Not shown: the in-page save buttons on LinkedIn, X, Hacker News and Reddit (showing them needs third-party pages and names), and Chrome's own permission prompt.

## How to re-render

Sources are in `assets/src/` (`main.tsx` harness with sample data, `compose.html` for the five 1280x800 layouts, `tile.html` for the promo tile, `index.html`, `vite.config.mjs`). `vite.config.mjs` uses absolute paths: `/tmp/am/cws-harness` for the harness and `/Users/chrisraethke/projects/agentmark/packages/extension` for the extension. Adjust them if either moves.

```sh
mkdir -p /tmp/am/cws-harness && cp docs/chrome-web-store/assets/src/* /tmp/am/cws-harness/
ln -sf "$PWD/packages/extension/node_modules" /tmp/am/cws-harness/node_modules
cd /tmp/am/cws-harness && npx vite --config vite.config.mjs   # serves http://localhost:5299
```

Then capture with a 1280x800 viewport at device scale 1, waiting about 3 seconds per page:
`http://localhost:5299/compose.html?shot=1` … `?shot=5`, and `http://localhost:5299/tile.html` with a 440x280 viewport.

The dashboard takes screenshots and the promo tile as JPEG or 24-bit PNG with no alpha. Check with `magick identify -format "%wx%h %[channels]\n" <file>` (expect `srgb 3.0`); if a capture has alpha, flatten it: `magick in.png -background white -alpha remove -alpha off PNG24:out.png`.

Store icon:

```sh
rsvg-convert -w 96 -h 96 packages/extension/icons/icon.svg -o /tmp/icon96.png
magick /tmp/icon96.png -background none -gravity center -extent 128x128 docs/chrome-web-store/assets/store-icon-128.png
```

Note: the packaged `icons/icon-128.png` fills the whole 128x128 with no padding. The store's guidance is 96x96 artwork with 16 px transparent padding, which is why the store icon above is a separate file. Whether to pad the packaged icon too is the extension builder's call.
