/**
 * Site-agnostic in-page "Save to AgentMark" button.
 *
 * A site adapter says where posts are, where the button goes and how to find a
 * post's permalink; this module does the rest. Clicking the button hands the
 * post (its permalink plus its markup as the page snapshot) to the toolbar
 * popup, so the user gets the usual saving → saved view with tags and
 * collections. If the popup cannot be opened, the service worker saves directly.
 * After each pass, the posts' URLs are looked up so bookmarked posts show as
 * saved from the start.
 *
 * Content scripts must never click or navigate the host page.
 */

import { removePrivateElements } from "./private-content";
import { cachedSaved, lookupSavedUrls, rememberSaved } from "./saved-lookup";

export { resetSavedCache } from "./saved-lookup";

export interface SiteAdapter {
  /** Matches one element per post. */
  postSelector: string;
  /** Icon colour while idle; defaults to the surrounding text colour. */
  idleColor?: string;
  /** Outline colour while saving; defaults to a mid indigo that reads on white pages. */
  savingColor?: string;
  /** Button size in CSS pixels; defaults to 32. Use a small value on dense, text-only layouts. */
  size?: number;
  /**
   * Vertical placement within the host's flex row; defaults to "center".
   * Use "start" where the neighbouring buttons sit at the top of a taller row.
   */
  align?: "center" | "start";
  /**
   * A neighbouring host control to line up with. When given, the button is
   * shifted so its vertical centre matches that element's, whatever padding
   * or margins the host applies to it.
   */
  alignWith?(post: Element): Element | null;
  /**
   * Posts nest inside each other (comment threads). A post then counts as
   * having a button only when the button's nearest post is that post, not a
   * reply inside it.
   */
  nested?: boolean;
  /**
   * Where to put the button: inserted into `parent` before `before`, or
   * appended when `before` is null. `slot` names the shadow-DOM slot of
   * `parent` to show it in, for hosts whose controls live in a shadow root.
   * Return null for things that are not saveable posts (ads, placeholders).
   */
  findSlot(post: Element): { parent: Element; before: Element | null; slot?: string } | null;
  /** The post's permalink, or undefined when it cannot be determined. */
  resolveUrl(post: Element, location: Location): string | undefined;
  /** Bookmark title to use until the agent writes a better one. */
  title(post: Element): string;
  /** The element to snapshot: the post, or a wrapper that also holds its comments. */
  snapshotRoot(post: Element): Element;
  /** Attributes added by AgentMark's helper scripts, removed from snapshots. */
  scratchAttributes?: string[];
  /**
   * Private elements (chat, messages) are always cut out of snapshots; "reddit"
   * adds Reddit's own rules (see `private-content.ts`).
   */
  privacy?: "reddit";
}

const BUTTON_ATTR = "data-agentmark-save";
/** Set once a button's URL has been looked up (or answered from the cache). */
const CHECKED_ATTR = "data-agentmark-checked";
/** How many passes could not resolve the button's URL for a lookup. */
const ATTEMPTS_ATTR = "data-agentmark-lookups";
/**
 * LinkedIn's URL comes from a MAIN-world helper that may not be installed, or
 * the post's data not attached, on the first pass; give it a few passes.
 */
const MAX_LOOKUP_ATTEMPTS = 3;

type ButtonState = "idle" | "saving" | "saved" | "error";

const STATE_STYLE: Record<ButtonState, { color?: string; label: string }> = {
  idle: { label: "Save to AgentMark" },
  saving: { color: "#818cf8", label: "Saving to AgentMark…" },
  saved: { color: "#4f46e5", label: "Saved to AgentMark" },
  error: { color: "#b91c1c", label: "AgentMark save failed" },
};

/** Label for a post that was already bookmarked when the page showed it. */
const ALREADY_SAVED_LABEL = "Saved to AgentMark — click to update";

/** How long a fresh save shows its tick before settling to the plain saved mark. */
export const SAVED_TICK_MS = 5_000;

// The AgentMark mark: a bookmark on a rounded badge, drawn in the button's
// colour on a 24x24 grid. The badge keeps it distinct from the bare bookmark
// glyph host sites use for their own "Save".
export const BADGE = "M4.284 1.5H19.716A2.784 2.784 0 0 1 22.5 4.284V19.716A2.784 2.784 0 0 1 19.716 22.5H4.284A2.784 2.784 0 0 1 1.5 19.716V4.284A2.784 2.784 0 0 1 4.284 1.5Z";
export const BOOKMARK =
  "M9.057 5.795H14.943A1.67 1.67 0 0 1 16.614 7.466V17.695Q16.614 18.205 16.105 18.205H15.834Q15.58 18.205 15.401 18.026L12 14.625L8.599 18.026Q8.42 18.205 8.166 18.205H7.895Q7.386 18.205 7.386 17.695V7.466A1.67 1.67 0 0 1 9.057 5.795Z";
// Solid badge with the bookmark cut out, so the page shows through it.
const SOLID = `<path data-part="badge" fill="currentColor" fill-rule="evenodd" d="${BADGE}${BOOKMARK}"/>`;
// Hollow badge around a solid bookmark.
const OUTLINE =
  '<rect data-part="outline" x="2.3" y="2.3" width="19.4" height="19.4" rx="1.984" fill="none" stroke="currentColor" stroke-width="1.6"/>' +
  `<path fill="currentColor" d="${BOOKMARK}"/>`;
const GLYPH_STROKE = 'fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"';
export const PLUS_PATH = "M12 8.86V11.56M10.65 10.21H13.35";
export const TICK_PATH = "M10.4 10.31L11.55 11.41L13.7 9.06";
const TICK = `<path data-part="tick" ${GLYPH_STROKE} d="${TICK_PATH}"/>`;
const BANG =
  `<path data-part="bang" ${GLYPH_STROKE} d="M12 8.61V10.56"/>` +
  '<circle cx="12" cy="11.96" r="0.72" fill="currentColor"/>';

type IconLook = ButtonState | "saved-fresh";

const ICON_BODY: Record<IconLook, string> = {
  // Hollow until the bookmark exists: grey at rest, light blue while the save
  // is in flight, then it fills in. Outline to solid reads as progress.
  idle: OUTLINE,
  saving: OUTLINE,
  "saved-fresh": SOLID + TICK,
  saved: SOLID,
  error: SOLID + BANG,
};

const AGENTMARK_ICON =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true"></svg>';

function drawIcon(button: HTMLButtonElement, look: IconLook): void {
  const icon = button.querySelector("svg");
  if (!icon) return;
  icon.innerHTML = ICON_BODY[look];
  icon.setAttribute("data-look", look);
}

/**
 * Serialize one post (and whatever its snapshot root holds) as a standalone
 * page. Undefined when private elements cannot be removed for certain: then
 * no snapshot is sent at all (fail closed).
 */
export function buildSnapshot(post: Element, title: string, adapter: SiteAdapter): string | undefined {
  const clone = adapter.snapshotRoot(post).cloneNode(true) as Element;
  clone.querySelectorAll(`[${BUTTON_ATTR}]`).forEach((el) => el.remove());
  try {
    removePrivateElements(clone, adapter.privacy === "reddit");
  } catch {
    return undefined;
  }
  for (const attr of adapter.scratchAttributes ?? []) {
    clone.removeAttribute(attr);
    clone.querySelectorAll(`[${attr}]`).forEach((el) => el.removeAttribute(attr));
  }
  const safeTitle = title.replace(/&/g, "&amp;").replace(/</g, "&lt;");
  // A bare table row is dropped by HTML parsers; give it back its table.
  const body = clone.tagName === "TR" ? `<table><tbody>${clone.outerHTML}</tbody></table>` : clone.outerHTML;
  return `<!DOCTYPE html><html><head><title>${safeTitle}</title></head><body>${body}</body></html>`;
}

interface StateOptions {
  /** Appended to the label, e.g. an error message. */
  detail?: string;
  /** For "saved": an existing bookmark, shown plain at once rather than with a fresh save's tick. */
  settled?: boolean;
}

function setState(button: HTMLButtonElement, state: ButtonState, options: StateOptions = {}): void {
  const style = STATE_STYLE[state];
  button.dataset.state = state;
  const savingColor = state === "saving" ? button.dataset.savingColor : undefined;
  button.style.color = savingColor ?? style.color ?? button.dataset.idleColor ?? "currentColor";
  button.disabled = state === "saving";
  const settledSave = state === "saved" && options.settled;
  const base = settledSave ? ALREADY_SAVED_LABEL : style.label;
  const label = options.detail ? `${base}: ${options.detail}` : base;
  button.title = label;
  button.setAttribute("aria-label", label);
  if (settledSave) {
    drawIcon(button, "saved");
  } else if (state === "saved") {
    // A fresh save shows a tick, then settles to the plain mark.
    drawIcon(button, "saved-fresh");
    setTimeout(() => {
      if (button.dataset.state === "saved") drawIcon(button, "saved");
    }, SAVED_TICK_MS);
  } else {
    drawIcon(button, state);
  }
}

/** Buttons whose save is being carried out by the popup, by post URL. */
const awaitingPopup = new Map<string, HTMLButtonElement>();
/** If the popup is dismissed before saving, stop showing "saving" after this long. */
const POPUP_RESULT_TIMEOUT_MS = 60_000;

function awaitPopupResult(url: string, button: HTMLButtonElement): void {
  awaitingPopup.set(url, button);
  setTimeout(() => {
    if (awaitingPopup.get(url) === button) {
      awaitingPopup.delete(url);
      if (button.dataset.state === "saving") setState(button, "idle");
    }
  }, POPUP_RESULT_TIMEOUT_MS);
}

/** Apply a save result reported by the service worker to the button that started it. */
export function handleSaveResult(message: unknown): void {
  const notice = message as { type?: string; url?: string; success?: boolean; error?: string } | null;
  if (notice?.type !== "agentmark:save-result" || typeof notice.url !== "string") return;
  const button = awaitingPopup.get(notice.url);
  if (notice.success) rememberSaved(notice.url);
  if (!button) return;
  awaitingPopup.delete(notice.url);
  if (notice.success) {
    setState(button, "saved");
  } else {
    setState(button, "error", { detail: notice.error ?? "save failed" });
  }
}

/** Save one post and reflect the outcome on the button. */
export async function savePost(
  post: Element,
  button: HTMLButtonElement,
  adapter: SiteAdapter,
): Promise<void> {
  const url = adapter.resolveUrl(post, window.location);
  if (!url) {
    setState(button, "error", { detail: "could not find this post's link" });
    return;
  }
  const title = adapter.title(post);

  setState(button, "saving");
  try {
    // Ask for the popup, so the save gets the usual tags/collection options.
    const response: unknown = await chrome.runtime.sendMessage({
      type: "open_save_popup",
      url,
      title,
      page_html: buildSnapshot(post, title, adapter),
    });
    const result = response as
      | { success?: boolean; error?: string; popup_opened?: boolean }
      | undefined;
    if (result?.success && result.popup_opened) {
      // The popup does the save; the service worker reports back when it finishes.
      awaitPopupResult(url, button);
    } else if (result?.success) {
      rememberSaved(url);
      setState(button, "saved");
    } else {
      setState(button, "error", { detail: result?.error ?? "no response from AgentMark" });
    }
  } catch (err) {
    setState(button, "error", { detail: err instanceof Error ? err.message : String(err) });
  }
}

/** The post each button belongs to, for the saved lookup. */
const postOf = new WeakMap<HTMLButtonElement, Element>();

function createButton(post: Element, adapter: SiteAdapter): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.setAttribute(BUTTON_ATTR, "");
  if (adapter.idleColor) button.dataset.idleColor = adapter.idleColor;
  if (adapter.savingColor) button.dataset.savingColor = adapter.savingColor;
  postOf.set(button, post);
  button.innerHTML = AGENTMARK_ICON;
  const size = adapter.size ?? 32;
  const icon = button.querySelector("svg");
  icon?.setAttribute("width", String(Math.min(24, size)));
  icon?.setAttribute("height", String(Math.min(24, size)));
  Object.assign(button.style, {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    verticalAlign: "middle",
    // Host rows are flex rows of varying height (X's action bar is taller on a
    // post's own page), so the button states its own placement.
    alignSelf: adapter.align === "start" ? "flex-start" : "center",
    width: `${size}px`,
    height: `${size}px`,
    padding: "0",
    margin: "0 4px",
    border: "none",
    borderRadius: "50%",
    background: "transparent",
    cursor: "pointer",
    flex: "0 0 auto",
    // Positioned, so it paints above a card-wide link the host lays over the
    // post (Reddit's feed cards), as the host's own buttons do.
    position: "relative",
  });
  setState(button, "idle");
  button.addEventListener("click", (event) => {
    // Posts are wrapped in click targets; keep the click from reaching the host page.
    event.preventDefault();
    event.stopPropagation();
    if (button.dataset.state === "saving") return;
    void savePost(post, button, adapter);
  });
  return button;
}

/**
 * Shift `button` vertically so its centre matches `reference`'s.
 * Measured after layout; returns the offset applied, in CSS pixels.
 */
export function alignVertically(button: HTMLElement, reference: Element): number {
  button.style.position = "relative";
  button.style.top = "0px";
  const own = button.getBoundingClientRect();
  const target = reference.getBoundingClientRect();
  // Not laid out (hidden or detached): leave the button where it is.
  if (own.height === 0 || target.height === 0) return 0;
  const offset = Math.round(target.top + target.height / 2 - (own.top + own.height / 2));
  button.style.top = `${offset}px`;
  return offset;
}

/** Whether `post` already shows a save button of its own. */
function hasButton(post: Element, adapter: SiteAdapter): boolean {
  if (!adapter.nested) return post.querySelector(`[${BUTTON_ATTR}]`) !== null;
  return Array.from(post.querySelectorAll(`[${BUTTON_ATTR}]`)).some(
    (button) => button.closest(adapter.postSelector) === post,
  );
}

/** Add a save button to every post under `root` that does not have one yet. Returns how many were added. */
export function injectSaveButtons(root: ParentNode, adapter: SiteAdapter): number {
  let added = 0;
  for (const post of Array.from(root.querySelectorAll(adapter.postSelector))) {
    if (hasButton(post, adapter)) continue;
    const slot = adapter.findSlot(post);
    if (!slot) continue;
    const button = createButton(post, adapter);
    if (slot.slot) button.slot = slot.slot;
    slot.parent.insertBefore(button, slot.before);
    const reference = adapter.alignWith?.(post);
    if (reference) {
      // After the browser has laid the new button out.
      requestAnimationFrame(() => alignVertically(button, reference));
    }
    added++;
  }
  void checkSavedPosts(root, adapter);
  return added;
}

/** Show an already-bookmarked post as saved, unless a click has moved the button on. */
function paintIfSaved(button: HTMLButtonElement, url: string): void {
  if (cachedSaved(url) === true && button.dataset.state === "idle") {
    setState(button, "saved", { settled: true });
  }
}

/**
 * Look up whether the posts under `root` whose buttons have not been checked
 * yet are already bookmarked, in one batch, and show those as saved. Answers
 * already cached are painted at once without asking again. Never rejects:
 * on any failure the buttons stay idle.
 */
export async function checkSavedPosts(root: ParentNode, adapter: SiteAdapter): Promise<void> {
  try {
    const waiting: Array<{ button: HTMLButtonElement; url: string }> = [];
    for (const button of Array.from(
      root.querySelectorAll<HTMLButtonElement>(`[${BUTTON_ATTR}]:not([${CHECKED_ATTR}])`),
    )) {
      const post = postOf.get(button);
      const attempts = Number(button.getAttribute(ATTEMPTS_ATTR) ?? "0");
      if (!post || attempts >= MAX_LOOKUP_ATTEMPTS) continue;
      const url = adapter.resolveUrl(post, window.location);
      if (!url) {
        // Not resolvable yet; a later pass tries again.
        button.setAttribute(ATTEMPTS_ATTR, String(attempts + 1));
        continue;
      }
      // Marked now, so later passes do not ask again while this lookup is in flight.
      button.setAttribute(CHECKED_ATTR, "");
      if (cachedSaved(url) === undefined) {
        waiting.push({ button, url });
      } else {
        paintIfSaved(button, url);
      }
    }
    if (waiting.length === 0) return;
    await lookupSavedUrls(waiting.map((item) => item.url));
    for (const { button, url } of waiting) paintIfSaved(button, url);
  } catch {
    // Never let a lookup problem reach the host page.
  }
}

/**
 * Keep buttons present as the site loads and re-renders posts. `onPass` runs
 * before every injection pass, the first one included.
 */
export function watchPosts(doc: Document, adapter: SiteAdapter, onPass?: () => void): MutationObserver {
  let scheduled = false;
  const run = () => {
    scheduled = false;
    onPass?.();
    injectSaveButtons(doc, adapter);
  };
  const observer = new MutationObserver(() => {
    if (scheduled) return;
    scheduled = true;
    setTimeout(run, 250);
  });
  onPass?.();
  injectSaveButtons(doc, adapter);
  chrome.runtime.onMessage.addListener(handleSaveResult);
  observer.observe(doc.documentElement, { childList: true, subtree: true });
  return observer;
}
