# Single purpose

Paste into **Privacy practices → Single purpose description**. The dashboard limits the field to 1,000 characters. The text below is 599 characters.

```
AgentMark saves web pages the user chooses into a local folder on the user's computer, through the AgentMark app installed on that computer, so the user and their coding agents can search and read them later. Every feature serves that purpose: saving the current page (toolbar, right-click menu, shortcut), saving single posts on LinkedIn, X and Hacker News, showing whether a page is already saved, browsing and editing saved bookmarks in the side panel, and setting up and configuring the companion app. The extension has no other function and does not send data anywhere except to that local app.
```

Why this holds (for the owner, not the dashboard): every UI surface in `packages/extension/src` is one of: popup (save, setup, install walk-through), side panel (bookmark list, detail, Settings), content scripts (save buttons and saved marks on supported sites), service worker (native messaging bridge, context menu, toolbar icon state). There is no unrelated feature such as ads, search changes, new-tab override or analytics.
