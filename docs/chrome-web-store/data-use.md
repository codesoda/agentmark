# Data use disclosure (Privacy practices tab)

Answers for the **Data usage** section of the Privacy practices tab, plus the certifications and the privacy policy field. Checked against the code in this working tree. Judgment calls are marked **JUDGMENT** with the reasoning, so the owner can decide.

## What actually moves, and where

| Data | From | To | When | Stored where |
|---|---|---|---|---|
| Page address and title | extension (popup, right-click menu, post button) | AgentMark app on the same computer, over Chrome native messaging | the user saves | bookmark folder and `~/.agentmark/index.db` |
| Rendered page HTML (`page_html`), up to 4,000,000 characters | extension (`chrome.scripting` on the saved tab, or the post's markup from a content script) | the app | the user saves | the bookmark folder (`source.html` and derived `article.md`) |
| Selected text, tags, collection, note, agent instructions | extension | the app | the user saves with "More options" or right-click on a selection | the bookmark folder and index |
| Addresses of posts visible on LinkedIn, X, Hacker News, Reddit | content scripts | the app (`lookup_urls`) | automatically while the user browses those sites | not stored; the app answers which are saved |
| Address of the current page on those sites | content scripts (`report_page`) | service worker, then the app (`lookup_urls`) | automatically on those sites | not stored |
| Address of every page the user opens | service worker (`toolbar-icon.ts`) | the app (`lookup_urls`) | only if the user turned on the optional `tabs` switch | not stored |
| Settings (folder, agent, summaries, instructions, extra sites) | extension | the app | setup and Settings | `~/.agentmark/config.toml` |
| Last tags and collection; setup draft; side panel view | extension | `chrome.storage.local` / `chrome.storage.session` | as used | the browser profile |
| The saved page, downloaded again | the app | the website at the saved address, from the user's computer (user agent `agentmark/<version>`, no browser cookies) | the user saves | bookmark folder |
| YouTube metadata and captions | the app via `yt-dlp`, if installed | YouTube | the user saves a YouTube address | bookmark folder |
| Page text (and for snapshot sites a simplified page skeleton), title, address, note, existing tags | the app starts the user's own agent CLI (`claude`, `codex` or `pi`) | that agent's provider, under the user's own account and settings | only when summaries are on (`enrichment.enabled`) | the provider's terms apply; the summary comes back into `bookmark.md` |
| Anything | — | AgentMark (the developer) | never today: AgentMark does not operate a server | — |

Sources: `src/shared/runtime.ts`, `src/shared/page-snapshot.ts`, `src/content/save-button.ts`, `src/content/saved-lookup.ts`, `src/content/page-report.ts`, `src/background/service-worker.ts`, `src/background/toolbar-icon.ts`, `packages/cli/src/fetch/mod.rs`, `packages/cli/src/youtube.rs`, `packages/cli/src/agent/{prompt,claude,codex,pi}.rs`, `packages/cli/src/commands/native_host.rs`. The extension itself makes no network requests (`grep` for `fetch(`/`XMLHttpRequest` in `src/` finds none).

Also local, not sent anywhere: the app writes log files to `~/.agentmark/logs/`. At the default level they include the address of each save. With `log_level = "debug"` they include whole messages, page HTML included.

## Form answers

The dashboard asks: "What user data do you plan to collect from users now or in the future?" The FAQ says local-only handling must be disclosed too ("Extensions are required to disclose how they handle user data, even when data is processed or stored locally on a user's device", User Data FAQ #3). So AgentMark's saving to the user's own disk counts.

Checkbox labels as listed by the store's public privacy pages; exact wording UNCONFIRMED until seen in the dashboard.

| Category (store description) | Tick? | Why |
|---|---|---|
| Personally identifiable information | **No** | AgentMark asks for no name, email, address or account. **JUDGMENT:** a saved page can contain other people's names (a LinkedIn profile, for example). That is the content of a page the user chose to save, covered by "Website content". If the owner wants the most cautious reading, tick it too; it is not wrong, just broader. |
| Health information | No | Not asked for or derived. Only present if it is on a page the user saves (covered by "Website content"). |
| Financial and payment information | No | Same. There is no payment today. When paid cloud sync arrives, revisit this box. |
| Authentication information | **No** | The extension reads no passwords, cookies or tokens. **JUDGMENT:** the rendered page HTML can incidentally contain values a site embeds in its markup (for example, a CSRF token in a meta tag). It stays on the user's disk and is not used. Ticking "No" is the usual reading; say so in the policy (it does). |
| Personal communications | **No** | The extension does not read email, chat or messages. Snapshot code skips LinkedIn's messaging overlay; Reddit post buttons are not added on inbox, chat and notification pages and chat elements are removed from post snapshots. **JUDGMENT:** if a user deliberately saves a web page that is itself a message (a webmail page), its HTML is saved like any page. That is user-chosen website content. |
| Location | No | No location API, no IP lookup. |
| Web history ("The list of web pages a user has visited, as well as associated data such as page title and time of visit") | **Yes** | Saved pages are a list of addresses, titles and save times. On LinkedIn, X, Hacker News and Reddit, the addresses of visible posts and of the current page are sent to the local app automatically to show saved marks; with the optional `tabs` switch, every page address is. **JUDGMENT:** the automatic lookups never leave the computer and are not stored; one could argue they are not "collected". Ticking is safer and matches FAQ #3. |
| User activity ("network monitoring, clicks, mouse position, scroll, or keystroke logging") | **No** | No input or network monitoring. A click on AgentMark's own save button is the user using the feature, not tracked activity. |
| Website content ("text, images, sounds, videos, or hyperlinks") | **Yes** | The rendered HTML, text and links of saved pages and posts. |

## Certifications (tick all three)

- [x] **I do not sell or transfer user data to third parties, outside of the approved use cases.** True. **JUDGMENT, the one to read carefully:** when summaries are on, page text goes to the provider of the user's own agent (Anthropic for Claude Code, OpenAI for Codex, whichever provider the user configured for pi). AgentMark does not choose that provider, has no account with it and receives nothing back; the user's own CLI sends it under the user's own account, and the user turned the feature on after the setup screen said "Sends the page text to your agent's provider." This is a transfer "necessary to provide ... the single purpose" at the user's request (Limited Use, "allowed transfer"), and arguably not a transfer by AgentMark at all. The setup screen, Settings and the privacy policy all disclose it.
- [x] **I do not use or transfer user data for purposes that are unrelated to my item's single purpose.** True. No ads, analytics, profiling or sale.
- [x] **I do not use or transfer user data to determine creditworthiness or for lending purposes.** True.

(Certification wording from public sources; confirm in the dashboard.)

## Privacy policy URL

`https://agentmark.app/privacy` (text in `privacy-policy.md`). The website must publish it before submission; the page does not exist in `web/` yet at the time of writing (another builder owns `web/`).

The policy contains the Limited Use statement the policy requires on a site that belongs to the extension: "The use of information received from the AgentMark extension complies with the Chrome Web Store User Data Policy, including the Limited Use requirements."

## Other judgment calls for the owner

1. **Prominent disclosure (User Data FAQ #10).** Needed when data handling is not closely related to the described feature. Summaries are disclosed inline next to the switch in setup and Settings ("Sends the page text to your agent's provider"). The automatic saved-mark lookups on LinkedIn, X, Hacker News and Reddit have no in-product notice; they stay on the computer and are part of the described save-button feature, so no extra consent screen is proposed. If a reviewer asks, the fix is one sentence on the setup screen.
2. **Secure transmission (FAQ #8/#9).** Native messaging to a program on the same computer needs no encryption (FAQ #16). The app's own downloads use whatever scheme the saved address has (`http` or `https`); that is the user's page, not data sent by AgentMark.
3. **"Not being sold / used for unrelated purposes" labels.** The listing then shows these. They are true today. Paid cloud sync, when it exists, will move bookmarks to a server AgentMark runs; at that point re-answer this form (at least Web history and Website content stay ticked, and the policy needs a sync section). The current wording ("AgentMark does not operate a server; nothing is sent to AgentMark") is a statement about today, so it does not need a rewrite of its basic stance, only an added section.
4. **Data retention and deletion.** All stored data is in the user's own folder, `~/.agentmark/` and the browser profile. Users delete it with `agentmark delete`, by deleting files, or by removing the extension (browser storage). AgentMark holds nothing to delete.
