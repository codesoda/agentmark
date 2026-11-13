# Owner checklist: Chrome Web Store submission

Steps only the owner can do, in order. The drafts this list points to are in this folder. Items marked UNCONFIRMED were not found on an official page; check them in the dashboard.

Dashboard: https://chrome.google.com/webstore/devconsole

**Item:** `oepdfndmpcpimbkaaocgolenncdkljeo`, status Draft (created 2026-11-12, not published). There is no working listing URL yet, so `WEB_STORE_URL` stays empty everywhere.

## Dashboard blockers (seen 2026-11-12, "Why can't I submit?")

- [ ] **Privacy policy link not reachable.** Publish `https://agentmark.app/privacy` (section 2).
- [ ] **Data-use certifications not ticked.** The three certifications on the Privacy tab (`data-use.md`). Left for the owner to tick.
- [ ] **Publisher contact email not set and not verified.** Set it and verify it on the dashboard's **Settings** page (section 1).
- [ ] **Distribution choices not made.** Free, visibility and regions (section 7).

## 0. Decide before you start

- [x] **Short description.** The store uses the manifest `description` as the summary. `packages/extension/manifest.json` already has the line from `listing.md`, which says the app is needed.
- [ ] **`notifications`.** Used only for right-click save results, and it adds the "Display notifications" install warning. Keep it or remove it (`permissions.md`, "Notes for the owner").
- [ ] **Data form judgment calls.** Read the **JUDGMENT** notes in `data-use.md` (PII, authentication data in page HTML, the summary provider transfer, the automatic saved-mark lookups) and accept or change the proposed answers.
- [ ] **Reddit (pending confirmation).** When this pack was finished, the manifest already matched `https://www.reddit.com/*` and `https://old.reddit.com/*`, and `snapshot_rules.rs` had `reddit.com`; another builder was still working on it. The listing, permissions, data-use, privacy policy and screenshot 5 all include Reddit. If Reddit does not ship, remove it from those files (search for "Reddit") and re-render screenshot 5.

## 1. Developer account

- [x] Register at the dashboard with the account that will own AgentMark. You must accept the developer agreement and pay a one-time registration fee (amount not stated on the official page; commonly US$5, UNCONFIRMED). The account email cannot be changed later; the docs suggest a dedicated address.
- [ ] Fill in the publisher name and contact email, and verify the email (dashboard **Settings** page; still a blocker on 2026-11-12). Complete any trader / non-trader declaration the dashboard asks for (UNCONFIRMED what it asks for your region).
- [ ] Optional: verify `agentmark.app` in Google Search Console so it can be the listing's Official URL.

## 2. Publish the privacy policy

- [ ] Fill the two OWNER placeholders in `privacy-policy.md` (publish date, contact; also name the website host).
- [ ] Publish it at `https://agentmark.app/privacy` (the website builder adds the route; check it is live). The dashboard needs the URL, and the policy must match the data form.

## 3. First upload (to get the item ID) — done

- [x] Make sure the versions agree in every file `AGENTS.md` lists (the zip script checks `manifest.json`, `package.json` and `packages/cli/Cargo.toml` and stops on a mismatch).
- [x] Build the zip: `cd packages/extension && npm run build:store-zip`. It prints the path, `target/chrome-web-store/agentmark-extension-<version>.zip`.
- [x] Dashboard → **New item** → upload the zip. Do not submit. The draft item has the **Item ID** `oepdfndmpcpimbkaaocgolenncdkljeo`. It is the extension ID every store user will have.
- [x] Recorded the item's public key (dashboard → **Package** → **View public key**). It is the manifest `key` (section 5); SHA-256 of it gives the item ID (a test checks this).

## 4. Put the ID in the code — ID done, URL waits for publishing

Contract §7.1 names the constants.

| Constant | File | Value |
|---|---|---|
| `WEB_STORE_EXTENSION_ID` (done) | `packages/extension/src/shared/constants.ts` | `oepdfndmpcpimbkaaocgolenncdkljeo` |
| `WEB_STORE_EXTENSION_ID` (done) | `packages/cli/src/setup/extension_ids.rs` | the same ID (tests fail if the two differ, if it is not 32 letters a–p, or if the manifest `key` gives another ID) |
| `WEB_STORE_URL` (not yet: set only after publishing, it turns on "Add to Chrome" links) | `packages/extension/src/shared/constants.ts`, `packages/app/src/constants.ts`, `web/src/lib/site.ts` (the web constant did not exist yet when this was written) | `https://chromewebstore.google.com/detail/<item ID>` (format UNCONFIRMED; after publishing, copy the address from the live listing and use that) |

- [ ] Bump the patch version (all version files), run the checks, and merge. (Not done with the ID change.)

What the ID changes (done): the native host manifest always allows the store extension (`install-extension` and the app's "Register with Chrome" step need no ID), and the install command shown in the extension drops `--extension-id` when the running copy has the store ID. What `WEB_STORE_URL` will change (after publishing): the app and website show "Add to Chrome" instead of the unpacked development steps.

## 5. The manifest `key` — decided: committed

- `packages/extension/manifest.json` has `"key"` (the item's public key, base64 DER). An unpacked copy, including the development copy `install-extension` and the app write to `~/.agentmark/extension`, loads with the store ID, which the native host already allows. No ID to paste.
- `npm run build:store-zip` removes `key` from the store zip (checked: the 0.3.0 zip's manifest has no `key`). Whether the store would accept or ignore a package that still has `key` is UNCONFIRMED; the script makes it moot.
- Side effect: the unpacked copy and the store copy share one ID, so they cannot both be installed in one Chrome profile. A copy built without the key gets its own ID and still works with `agentmark install-extension --extension-id <ID>` (contract §8.1).

## 6. Release the app the reviewer will install

The extension's install steps run `install.sh` from `main` and download `releases/latest`. Today `releases/latest` is **v0.1.2** (`gh release list`), which has no setup support; a reviewer would get "This AgentMark app is too old for this extension".

- [ ] Merge this branch so `install.sh` on `main` is the new one.
- [ ] Tag and publish (not draft) a release built from the commit that has the store ID, so the pre-built `agentmark` registers the store ID.
- [ ] On a clean Apple Silicon Mac (or a fresh user account): add the extension from the zip as unpacked, follow `reviewer-notes.md` steps 1 to 5, and confirm they work as written.
- [ ] While testing, install the packed zip and compare the Chrome install prompt with `permissions.md` ("Install-time warning"). Exact host list and order are UNCONFIRMED until this test.
- [ ] Also confirm the toolbar icon turns blue on a saved LinkedIn/X/HN/Reddit page **without** the `tabs` permission. It relies on `sender.url` being present for content-script messages, which contract §13 lists as unconfirmed.

## 7. Second upload and the listing

- [ ] Rebuild the zip with the ID set (`npm run build:store-zip`). Upload it on the item's **Package** tab. Use a higher version than the first upload (whether the store requires that for a draft is UNCONFIRMED; bumping is safe).
- [x] Re-render screenshot 3 so its command has no placeholder ID (`assets.md`, "How to re-render"). Done 2026-11-12 with the store ID.
- [ ] **Store listing** tab: description, category (the drop-down offers "Developer Tools"; chosen), language and URLs from `listing.md`; store icon, screenshots 1–5 in order and the small promo tile from `assets/` (see `assets.md`).
- [ ] **Privacy practices** tab: single purpose from `single-purpose.md`; one justification per permission and the host justification from `permissions.md` (each box: 1,000 characters); data types and the three certifications from `data-use.md` (certifications still to tick, a blocker); there is no remote-code question on this form; privacy policy URL `https://agentmark.app/privacy`.
- [ ] **Distribution** tab: free; visibility (Public); regions (all, or as you choose). Not made yet (a blocker).
- [ ] **Test instructions** tab: "Additional instructions" takes at most 500 characters. Use the dashboard version in `reviewer-notes.md` (a 492-character version was saved 2026-11-12); the long block there is the reference.

## 8. Submit

- [ ] Click **Submit for review**. Consider unchecking "publish automatically" so you publish by hand after approval. After approval you have 30 days to publish, then it reverts to a draft (developer.chrome.com, "Publish").
- [ ] Expect a review of "a few days" up to "a few weeks". New developers, new extensions and powerful permissions can take longer. Contact support after three weeks (developer.chrome.com, "Review process"). Results arrive by email.
- [ ] Likely questions from review: why `nativeMessaging` and content scripts are needed (answered in `permissions.md`) and how to test without the app (answered in `reviewer-notes.md`). If rejected, the email names the policy; fix and resubmit. Reviews after a rejection can be slower.
- [ ] A new publisher can have at most two published extensions at first (developer.chrome.com, "Publish").

## 9. After it is live

- [ ] Install from the store in a clean Chrome profile. Check the ID equals `WEB_STORE_EXTENSION_ID` and the install prompt matches `permissions.md`.
- [ ] Copy the live listing URL into `WEB_STORE_URL` if it differs from the guessed format, and redeploy the website.
- [ ] Later updates: adding a host to `content_scripts.matches` or a new required permission changes the install warning and disables the extension for existing users until they accept. Prefer optional permissions.
- [ ] When paid cloud sync is added, update `privacy-policy.md` and re-answer the data form before that version ships.

## UNCONFIRMED numbers and facts to check in the dashboard

| Item | Value used in this pack | Status |
|---|---|---|
| Name limit | 75 characters | Confirmed (manifest reference) |
| Short description (manifest `description`) limit | 132 characters | Confirmed (manifest reference) |
| Detailed description limit | 16,000 characters (draft is 3,711) | Confirmed in the dashboard |
| Single purpose field limit | 1,000 characters (draft is 599; the dashboard counter showed 607) | Confirmed in the dashboard |
| Permission justification field limit | 1,000 characters per box (drafts are 151–542) | Confirmed in the dashboard |
| Test instructions "Additional instructions" limit | 500 characters | Confirmed in the dashboard; the long block does not fit (see `reviewer-notes.md`) |
| Remote code question | not on the form | Confirmed in the dashboard |
| Screenshots | 1–5, 1280x800 or 640x400 | Confirmed |
| Small promo tile | 440x280 | Confirmed |
| Store icon | 128x128 PNG, 96x96 artwork with 16 px padding | Confirmed |
| Category names | Developer Tools | Confirmed: offered in the drop-down and chosen |
| Data-type checkbox labels and the three certification sentences | as in `data-use.md` | UNCONFIRMED wording; labels match public listing pages and secondary sources |
| Install prompt for more than three hosts | "Read and change your data on a number of websites" with hosts listed below | Rule confirmed in Chromium source (`HostListFormatter`, more than 3 hosts); exact display and order UNCONFIRMED until a packed test install |
| Registration fee | US$5 | UNCONFIRMED; official page says only "one-time registration fee" |
| `key` in an uploaded package | stripped by the zip script (checked in the 0.3.0 zip) | UNCONFIRMED whether the store rejects or ignores it |
| Listing URL format | `https://chromewebstore.google.com/detail/<id>` | UNCONFIRMED; use the live URL |
| Re-upload of a draft needs a higher version | bump anyway | UNCONFIRMED |
| Trader declaration | whatever the dashboard asks | UNCONFIRMED |
