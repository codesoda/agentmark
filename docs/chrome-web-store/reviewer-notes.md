# Notes for the store reviewer

The dashboard's **Test instructions** tab has a Username, a Password and an **Additional instructions** field. Additional instructions is limited to 500 characters (seen in the dashboard), so the full block does not fit. Paste the short dashboard version; the long version is the reference for the owner and for any reply to the reviewer.

Before submitting, check the preconditions in `checklist.md` (section 6): the release with the new app is published and `install.sh` on `main` is the new one. Without them step 3 fails. The Web Store ID constants are already set.

## Dashboard version (Additional instructions, 500-character limit)

This is the text saved in the dashboard on 2026-11-12 (492 characters):

```
No account or login is needed. AgentMark saves pages through a companion app on the computer (native messaging). Without the app, clicking the icon shows install steps. To test saving on a Mac with Apple Silicon: click the icon, pick your platform, copy the Terminal command, run it, return to Chrome, click the icon, then "Finish and save this page". Files appear in ~/Documents/AgentMark. Intel Mac and Linux build from source. Windows is not supported. More: https://agentmark.app/#install
```

Leave Username and Password empty: there is no account.

## Reference version (full steps, 3,651 characters)

```
AgentMark needs a companion program on the computer. The extension saves pages through the AgentMark app (a program named "agentmark") over Chrome native messaging. It has no server and no account, so no test credentials are needed.

SUPPORTED PLATFORMS
- macOS on Apple Silicon: one Terminal command installs the app (steps below, about one minute).
- macOS on Intel and Linux: the app must be built from source (needs Rust and Node.js; takes several minutes). The extension shows these steps.
- Windows and ChromeOS: not supported yet. On these systems you can review the extension's install guidance (step 1-2) but cannot save pages.

1. WITHOUT THE APP (any platform)
Open any normal web page, for example https://example.com, and click the AgentMark toolbar icon.
Expected: the popup shows "Finish setting up AgentMark" and a list of platforms. Your computer's platform is the first row, marked "Your computer". "Already installed? Check again" is at the bottom. The side panel (right-click the toolbar icon, "Open AgentMark Side Panel") shows the same screen.

2. INSTALL STEPS
Click your platform's row.
Expected on a Mac with Apple Silicon: "Install on Mac Apple Silicon", a Terminal command with a Copy button, and "Waiting for the app..." below it. Windows shows "AgentMark does not run on Windows yet."

3. INSTALL THE APP (Mac with Apple Silicon)
Click Copy, open Terminal, paste and press Return. The script downloads the app from the project's GitHub releases into ~/.agentmark/bin, links it as ~/.local/bin/agentmark, registers it with Chrome for this extension, and installs an agent skill. It asks no questions and ends with "Done. Go back to Chrome: AgentMark finishes setting up there."

4. SET UP (in Chrome)
Click the toolbar icon again (the popup may have closed while you used Terminal).
Expected within about 2 seconds: "Set up AgentMark", with a folder field (default ~/Documents/AgentMark) and a list of agents (Claude Code, Codex, pi), each marked "Found" or "Not installed". If none is installed, the summary switch is off and the screen says why. You do not need any agent to test saving.
Click "Finish and save this page".
Expected: "Saved!" with the page title. The toolbar icon turns blue with a tick, then plain blue.

5. CHECK THE RESULT
In Finder, open ~/Documents/AgentMark/<year>/<month>/<day>/. The page's folder contains article.md, bookmark.md, metadata.json, source.html and events.jsonl.
Open the side panel: the page is in the list. Click it to see its details.

6. MORE FEATURES
- Right-click a page and choose "Save to AgentMark": a system notification reports the result.
- Open https://news.ycombinator.com and any discussion. Each comment gets a small AgentMark button. Clicking it saves that comment's thread; the button shows saving, then saved.
- Right-click the toolbar icon, choose "Settings": the side panel opens on Settings (folder, summaries, sites, toolbar icon).
- In Settings, turn on "Show when the current page is already saved". Chrome asks for "Read your browsing history". This is the only optional permission and is off by default.

7. REMOVE EVERYTHING
In Terminal:
  rm -rf ~/.agentmark ~/.local/bin/agentmark ~/.agents/skills/agentmark ~/.claude/skills/agentmark
  rm -f "$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts/com.agentmark.native.json"
  rm -rf ~/Documents/AgentMark
Then remove the extension.

Privacy: the extension sends data only to the app on the same computer. Summaries are off unless an agent is installed and the user leaves the switch on; then the user's own agent sends the page text to its provider. Details: https://agentmark.app/privacy
```

## Notes for the owner (not for the dashboard)

- The step 3 command comes from `INSTALL_SCRIPT_URL` (raw `install.sh` on `main`) and downloads `releases/latest`. Today `releases/latest` is v0.1.2 (checked with `gh release list`), which has no setup messages. A reviewer would see "This AgentMark app is too old for this extension" (contract §4.2 `update_required`). Publish a release built from this branch before submitting.
- `WEB_STORE_EXTENSION_ID` is set (`oepdfndmpcpimbkaaocgolenncdkljeo`), so a store install shows the short command, and the CLI registers the store ID with no flags. This needs a published release built with the constant: an older `agentmark` does not know the ID. A copy with another ID still gets `--extension-id <its ID>` in the command.
- The remove commands match the filesystem map in contract §10 (Chrome on macOS only). The installer may also write host manifests for Chromium, Edge, Brave or Arc if their profile folders exist; reviewers normally use Chrome only.
- Hacker News is used in step 6 because it needs no login. LinkedIn, X and Reddit buttons also exist but need an account (Reddit works logged out on www.reddit.com).
- If review is slow because the reviewer cannot run the app, a short screen recording of steps 1 to 5 linked from the instructions helps. That is the owner's call.
