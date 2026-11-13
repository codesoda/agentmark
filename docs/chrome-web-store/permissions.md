# Permission justifications

Paste into **Privacy practices → Permission justification**. The dashboard shows one box per permission it finds in the uploaded manifest, plus one **Host permission justification** box that covers the content-script match patterns. The dashboard limits each box to 1,000 characters. Every text below is under 600 characters.

Manifest checked: `packages/extension/manifest.json` in this working tree (version 0.2.6), re-read at the end of drafting. If the manifest changes, re-check this file.

## Audit result

Every declared permission is used by the code. None is unused.

| Permission | Kind | Used in | Install warning |
|---|---|---|---|
| `activeTab` | required | `src/shared/runtime.ts` (`queryActiveTab`, `querySelectedText`, `queryPageHtml`) on the tab where the user clicked the icon, pressed the shortcut or used the context menu | none |
| `contextMenus` | required | `service-worker.ts` `ensureContextMenu`, `handleContextMenuClick` | none |
| `sidePanel` | required | `service-worker.ts` `openSidePanel`, `openSettings`; manifest `side_panel` | none |
| `nativeMessaging` | required | `src/shared/native-messaging.ts` (`connectNative`, `sendNativeMessage`) | "Communicate with cooperating native applications" |
| `notifications` | required | `service-worker.ts` `showNotification`, only for right-click saves (success or failure) | "Display notifications" |
| `storage` | required | `chrome.storage.local` (last-used tags and collection, `runtime.ts`); `chrome.storage.session` (setup draft, `setup-draft.ts`; side panel view, `SidePanel.tsx`) | none |
| `scripting` | required | `runtime.ts` `executeScript` for the selected text and the page snapshot | none |
| `tabs` | optional, off by default | `toolbar-icon.ts` (URL of every navigation and tab switch, to look it up); requested from `SettingsView.tsx` | "Read your browsing history" (only when the user turns it on) |
| content scripts on LinkedIn, X, Twitter, Hacker News, Reddit | host access | `src/content/*` | one combined host line, below |

Notes for the owner:

- `notifications` is the thinnest one: it is used only to report the result of a save started from the right-click menu, where there is no popup. It does add the "Display notifications" warning. Keeping it is defensible. Removing it would mean right-click saves give no feedback beyond the toolbar icon. Decide before upload; this pack does not change the manifest.
- `commands` (keyboard shortcuts) is a manifest key, not a permission, so it gets no box.
- No `host_permissions`, no `<all_urls>`, no remote code.

## Install-time warning

Built from Chromium's warning rules (`chrome/common/extensions/permissions/chrome_permission_message_rules.cc`, read for this draft) and the warning strings in `chrome/app/generated_resources.grd` (from `/tmp/am/research.md` B5). With more than three hosts Chrome does not name them in the main line; it shows "a number of websites" and lists the hosts underneath. The exact host list text and order are UNCONFIRMED until a test install of the packed zip.

```
Add "AgentMark"?

It can:
  Read and change your data on a number of websites
    (www.linkedin.com, x.com, twitter.com, news.ycombinator.com,
     www.reddit.com, old.reddit.com)
  Display notifications
  Communicate with cooperating native applications
```

"Read your browsing history" does not appear at install. Chrome shows it only when the user turns on the Settings switch that requests `tabs`.

Updates: adding a host to `content_scripts.matches` later changes the warning, and Chrome disables the extension until the user accepts it (developer.chrome.com, "Declare permissions"). Adding Reddit before the first store release avoids that.

## Text to paste

### activeTab

```
Used only after the user clicks the toolbar icon, presses the save shortcut or picks "Save to AgentMark" from the right-click menu. It gives access to that one tab so the extension can read its address and title, read any text the user selected, and take a copy of the page as shown. That copy is sent to the AgentMark app on the user's computer, which saves the page. No other tab is touched.
```

### contextMenus

```
Adds "Save to AgentMark" to the right-click menu on pages and selected text, and "Open AgentMark Side Panel" and "Settings" to the toolbar icon's menu.
```

### sidePanel

```
The side panel shows the user's saved bookmarks (list, details, tags, notes, collections) and the Settings view. It opens from the toolbar icon's menu or the Cmd/Ctrl+Shift+B shortcut.
```

### nativeMessaging

```
AgentMark stores bookmarks as files on the user's computer through the AgentMark app installed there. Native messaging is the only way the extension talks to that app: to save a page, list and edit bookmarks, check whether pages are already saved, and run first-time setup and Settings. The extension has no server and makes no other network requests.
```

### notifications

```
Shows a system notification with the result (saved or failed) when the user saves a page from the right-click menu. That path has no popup, so the notification is the only feedback. No other notifications are sent.
```

### storage

```
chrome.storage.local remembers the tags and collection the user last chose, to prefill the save form. chrome.storage.session keeps unfinished setup answers while the popup is closed for the folder dialog, and which side panel view to open. Nothing else is stored, and nothing leaves the browser through storage.
```

### scripting

```
After the user saves a page (toolbar icon, shortcut or right-click menu), the extension runs a small function in that tab to read the selected text and the page's rendered HTML. This lets AgentMark save pages that only render for a logged-in browser. It never runs on pages the user did not save. It does not change page content; on LinkedIn profile and company pages it scrolls down to load every section, then scrolls back.
```

### tabs (optional permission)

If the dashboard shows a box for optional permissions, paste:

```
Optional and off by default. Requested only when the user turns on "Show when the current page is already saved" in Settings. It lets the toolbar icon read the address of each page the user opens, so the extension can ask the local AgentMark app whether that page is already saved and color the icon. Addresses go only to the app on the same computer and are not stored by the lookup. The user can turn it off in Settings.
```

### Host permission justification (content scripts)

```
Content scripts run on www.linkedin.com, x.com, twitter.com, news.ycombinator.com, www.reddit.com and old.reddit.com only. They add a save button to each post or comment so the user can save one post instead of the whole feed, and mark posts that are already saved. On a click, the script sends that post's link and markup to the AgentMark app on the user's computer. It also asks the app which visible posts are already saved, and reports the page address so the toolbar icon is correct. The scripts never click, scroll or navigate the page.
```

Per-host notes (if the dashboard asks per host, reuse the sentence for the site):

| Match pattern | Feature |
|---|---|
| `https://www.linkedin.com/*` (two scripts) | Save buttons on posts. The second script runs in the page's own context only to read the post ID, which LinkedIn keeps in its page data (`linkedin-urn.ts`). The snapshot never reads LinkedIn's messaging overlay (`page-snapshot.ts`). |
| `https://x.com/*`, `https://twitter.com/*` | Save buttons on posts. twitter.com is the same site's older address. |
| `https://news.ycombinator.com/*` | Save buttons on comments in discussions. |
| `https://www.reddit.com/*`, `https://old.reddit.com/*` | Save buttons on posts and comments. No buttons on inbox, notification and chat pages; chat elements are kept out of snapshots (`reddit-posts.ts`). Pending: Reddit was being added while this was drafted; confirm it ships. |

## Remote code

The Privacy tab seen on 2026-11-12 has no remote-code question. If one appears, answer **"No, I am not using remote code."** All JavaScript is in the package. The built `dist/` has no `eval` or `new Function` (checked with grep on the build used for the zip).
