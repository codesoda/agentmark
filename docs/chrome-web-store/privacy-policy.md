# Privacy policy draft

Text for `https://agentmark.app/privacy`. Copy everything below the line. Replace the two OWNER placeholders before publishing. Set "Last updated" to the publish date.

---

# AgentMark privacy policy

Last updated: OWNER: publish date

AgentMark is a Chrome extension and an app that runs on your computer. Together they save web pages you choose into a folder on your computer, so you and your coding agents can search and read them. This policy explains what data AgentMark handles, where it goes, and what you control.

## The short version

- Your bookmarks are files in a folder you choose on your computer.
- AgentMark does not operate a server. Nothing you save or browse is sent to AgentMark.
- AgentMark has no accounts, no analytics, no ads and no tracking.
- If you turn on summaries, the coding agent you chose sends the page text to its own provider under your account.

## What the extension handles

The extension works only with the AgentMark app on the same computer. It sends data to that app through Chrome's native messaging, which stays on your computer. The extension itself does not send data over the internet.

When you save a page (toolbar icon, keyboard shortcut, right-click menu, or a save button on a post), the extension sends the app:

- the page address and title,
- a copy of the page as your browser shows it (its HTML),
- anything you add: tags, a collection, a note, instructions for your agent, and text you selected.

To show which pages and posts you already saved, the extension sends page addresses to the app and asks whether they are saved:

- On LinkedIn, X, Hacker News and Reddit, it sends the addresses of the posts on the page and of the page itself.
- On other sites, only if you turn on "Show when the current page is already saved" in Settings. Chrome asks for your permission first ("Read your browsing history"). You can turn it off at any time.

The app answers these questions and does not record the addresses.

The extension also keeps a few small things in your browser: the tags and collection you used last, unfinished setup answers, and which side panel view to open.

## What the app does

- **Saves pages to your computer.** Each saved page becomes a folder in the location you chose (by default, Documents/AgentMark). It holds the article as Markdown, a bookmark file, page metadata, the page HTML and an event log. A search index and your settings are kept in the `.agentmark` folder in your home folder.
- **Downloads the page you save.** The app fetches the saved address directly from the website, from your computer. It does not use your browser's cookies. For YouTube addresses, if you installed the yt-dlp program, the app uses it to get the video's description, chapters and captions from YouTube.
- **Notes when the extension connects.** The app keeps a small file in `.agentmark` with the time the extension last connected and the extension's ID and version, so the AgentMark app can tell that setup worked.
- **Writes local log files** in `.agentmark/logs`. They include the addresses of pages you save. If you raise the log level to "debug", they include full messages from the extension, page HTML included.

## Summaries (optional)

If you turn on "Write a summary and tags after each save", the app starts the coding agent you chose (Claude Code, Codex or pi), which is already installed on your computer. It gives that agent the page text, title, address, your note and existing tags, and asks for a summary and tags. For some sites it also gives the agent a simplified copy of the page so the agent can turn it into readable text.

That agent sends this data to its own provider (for example Anthropic for Claude Code, or OpenAI for Codex) using your account and that provider's terms. AgentMark does not choose the provider, does not receive the data, and has no access to your agent account.

If summaries are off, the app does not start an agent and no page text goes to any provider. You can change this in Settings at any time.

When you later ask your coding agent to search or read your bookmarks, that agent reads the files on your computer and handles them under its own terms.

## What AgentMark receives

None of your data. AgentMark does not operate a server that receives data from the extension or the app, and it does not collect analytics or crash reports.

Two things happen outside the extension and app that you should know about:

- **Installing the app** downloads it from GitHub. GitHub's privacy statement applies. AgentMark sees only the download counts GitHub shows to repository owners.
- **This website** (agentmark.app) has no analytics code and sets no cookies of its own. OWNER: name the hosting provider, which may keep standard request logs.

## Sharing and selling

AgentMark does not sell, rent or share your data, and does not use it for advertising, profiling or credit decisions. The only transfer of your data to a third party is the optional summary step above, which your own agent performs at your request.

The use of information received from the AgentMark extension complies with the [Chrome Web Store User Data Policy](https://developer.chrome.com/docs/webstore/program-policies/policies), including the Limited Use requirements.

## Your control

- Everything AgentMark stores is on your computer. You can read, move or delete it like any other files.
- Delete one bookmark with `agentmark delete <id>`, or delete its folder.
- Turn summaries off, or turn the "already saved" switch off, in Settings.
- Removing the extension deletes what it kept in your browser. Your saved files stay until you delete them.

## Children

AgentMark is a tool for developers and is not directed at children.

## Changes

If AgentMark changes how it handles data, for example by adding an optional sync service, this page will be updated before the change ships, and the date above will change.

## Contact

OWNER: contact email or contact page
