# Store listing draft

Paste into the developer dashboard, **Store listing** tab. Every claim below was checked against the code in this branch (extension 0.2.6 working tree, contract `docs/setup-contract.md`). If the code changes before upload, re-check the lines marked with a source.

## Name

```
AgentMark
```

9 characters (limit 75, from the manifest `name`; the store reads it from the uploaded package).

## Short description (summary)

The store shows the manifest `description` as the summary. It is set in `packages/extension/manifest.json`, so it changes only with a new upload. Today it is `Agent-first bookmarking for local AI workflows` (46 characters). It does not say that the app is needed. Proposed replacement (the extension builder or the owner edits the manifest; this pack does not touch it):

```
Save web pages as plain files on your computer for your coding agents to search and read. Needs the AgentMark app on Mac or Linux.
```

130 characters (limit 132, from the manifest reference). Counted with `python3 -c 'print(len(s))'`.

Shorter fallback, 110 characters:

```
Save web pages as plain files your coding agents can search and read. Needs the AgentMark app on Mac or Linux.
```

## Detailed description

Plain text. The store does not render Markdown, so the list markers below are plain hyphens. 3,711 characters (limit 16,000, confirmed in the dashboard).

```
AgentMark saves the page you are reading into a folder on your computer, as plain files that your coding agents can search and read. It works with Claude Code, Codex and pi.

The extension needs the AgentMark app on your computer. The app does the saving; the extension cannot save anything without it. The app runs on Mac and Linux. Windows is not supported yet.

WHAT YOU CAN DO

- Save the current page from the toolbar icon, the right-click menu, or the keyboard (Cmd+Shift+S on Mac, Ctrl+Shift+S on Linux).
- Add tags, a collection, a note, or instructions for your agent with "More options". Text you selected on the page is saved too when you use "More options" or the right-click menu.
- Save single posts on LinkedIn, X, Hacker News and Reddit with the save button AgentMark adds to each post (and to each comment on Hacker News and Reddit). Posts you already saved are marked.
- Open the side panel (Cmd+Shift+B or Ctrl+Shift+B) to browse what you saved, read summaries, edit tags, notes and collections, and move bookmarks between Inbox, Processed and Archived.
- Turn on summaries if you want them. After each save, the coding agent you choose writes a short summary and suggests tags.

HOW A PAGE IS SAVED

Each saved page gets its own folder with the article as Markdown, a bookmark file with the details and summary, the page metadata, the page HTML and a small event log. A search index on your computer lets you and your agents find pages again.

The app downloads the page itself. The extension also sends the page as your browser shows it. AgentMark uses that copy for LinkedIn, X, Reddit and Hacker News discussions, and for other sites you add in Settings. On every other site it uses that copy only when its own download fails, for example when a site needs you to be logged in.

If the yt-dlp program is installed, YouTube videos are saved with their description, chapters and captions.

FIRST RUN

1. Add the extension and click its icon.
2. If the app is not installed, the extension shows the install steps for your computer. On a Mac with Apple Silicon this is one command that you paste into Terminal. On an Intel Mac or on Linux you build the app from source, which needs Rust and Node.js. The extension waits and moves on by itself when the app answers.
3. Choose the folder for your bookmarks (the default is Documents/AgentMark) and the agent that writes summaries. Click "Finish and save this page".

You can change these choices later in Settings, in the side panel. The installer also adds an AgentMark skill, so agents that support skills know how to search and read your bookmarks.

OPTIONAL: SHOW SAVED PAGES ON EVERY SITE

The toolbar icon turns blue when a page is saved. Without extra permissions it does this after you save, and on LinkedIn, X, Hacker News and Reddit. To see it on every site, turn on "Show when the current page is already saved" in Settings. Chrome then asks for permission to read your browsing history. It is off by default.

WHERE YOUR DATA GOES

- The extension sends the pages you save, and the addresses of pages it checks, only to the AgentMark app on the same computer. Your bookmarks stay in the folder you chose.
- The app downloads the pages you save directly from their websites.
- AgentMark does not operate a server. Nothing is sent to AgentMark.
- Summaries are optional. When they are on, the app asks the agent you chose (Claude Code, Codex or pi), which is already installed on your computer, to write the summary. That agent sends the page text to its own provider under your account. Turn summaries off in Settings and no page text is sent to an agent.

Privacy policy: https://agentmark.app/privacy
Website: https://agentmark.app
```

Sources for the claims (for the reviewer of this draft, not for the dashboard):

| Claim | Where it is true |
|---|---|
| App required, extension has no network code | `src/shared/native-messaging.ts` is the only transport; no `fetch`/`XMLHttpRequest` in `src/` |
| Mac and Linux, Windows not supported | contract §7.3 platform table, `src/shared/platforms.ts` |
| Apple Silicon one command; Intel and Linux from source | contract §7.3 (while `MAC_APP_DOWNLOAD_URL` is empty) |
| Shortcuts | `manifest.json` `commands` |
| Tags, collection, note, agent instructions, selected text | `src/popup/SaveForm.tsx`; selection read only for "More options" (`Popup.tsx` `handleMoreOptions`) and the context menu (`info.selectionText`) |
| Right-click save | `service-worker.ts` `ensureContextMenu` (`contexts: ["page","selection"]`) |
| Post save buttons, saved marks | `src/content/*`, `saved-lookup.ts` |
| Side panel states Inbox / Processed / Archived | `src/sidepanel/` |
| Bundle files | `packages/cli/src/bundle/` (`article.md`, `bookmark.md`, `metadata.json`, `source.html`, `events.jsonl`) |
| Browser copy rules | `AGENTS.md` "Browser snapshots", `packages/cli/src/snapshot_rules.rs` |
| yt-dlp | `packages/cli/src/youtube.rs` |
| Default folder | contract §5.1 `default_storage_path` |
| Optional `tabs`, off by default | `manifest.json` `optional_permissions`, `src/sidepanel/SettingsView.tsx` |
| Summaries and the provider | contract §5.4, `packages/cli/src/agent/` |

Reddit was being added by another builder while this draft was written (manifest matches `www.reddit.com` and `old.reddit.com`; `snapshot_rules.rs` has `reddit.com`). The three Reddit mentions above assume that work ships. If it does not, remove Reddit from them (see `checklist.md`).

## Category

**Developer Tools** (offered in the dashboard drop-down and chosen).

## Language

**English**. The extension has no `_locales`, so only one listing language applies.

## Other listing fields

| Field | Value |
|---|---|
| Homepage URL | `https://agentmark.app` |
| Support URL | `https://github.com/codesoda/agentmark/issues` (OWNER: confirm the repo is public, or use a contact page) |
| Official URL | `agentmark.app`, only after the domain is verified in Google Search Console |
| Mature content | Off |
| Graphic assets | see `assets.md` |
