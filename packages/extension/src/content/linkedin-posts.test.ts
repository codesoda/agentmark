/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, beforeEach, vi } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";
import { alignVertically, checkSavedPosts, resetSavedCache, SAVED_TICK_MS } from "./save-button";
import {
  buildPostSnapshot,
  handleSaveResult,
  injectButtons,
  linkedinAdapter,
  postAuthor,
  postUrl,
  resolvePostUrl,
  resolvePostUrn,
  RESOLVE_EVENT,
  URN_ATTR,
} from "./linkedin-posts";

const SAVE_BUTTON = "[data-agentmark-save]";

function feedHtml(): string {
  return `
    <div componentkey="expandedAAAFeedType_">
      <div componentkey="update-card-focusAAAFeedType_MAIN_FEED_RELEVANCE" id="post-a">
        <div class="hdr"><button aria-label="Open control menu for post by Ada Lovelace"></button></div>
        <span data-testid="expandable-text-box">The engine weaves algebraic patterns.</span>
      </div>
      <div componentkey="replaceableComment_urn:li:comment:(activity:1,2)"><p>Nice post</p></div>
    </div>
    <div componentkey="update-card-focusBBBFeedType_MAIN_FEED_RELEVANCE" id="ad">
      <span>Promoted content without a post menu</span>
    </div>`;
}

let listeners = new AbortController();

/** Stand in for the MAIN-world script (removed again before the next test). */
function answerUrnRequests(urnFor: (el: Element) => string | null): void {
  document.addEventListener(
    RESOLVE_EVENT,
    (event) => {
      const el = event.target as Element;
      const urn = urnFor(el);
      if (urn) el.setAttribute(URN_ATTR, urn);
    },
    { signal: listeners.signal },
  );
}

function fakeLocation(href: string): Location {
  return new URL(href) as unknown as Location;
}

async function flush(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

/** The messages sent to the service worker with this `type`. */
function sent(sendMessage: ReturnType<typeof vi.fn>, type: string): Array<Record<string, unknown>> {
  return sendMessage.mock.calls
    .map((call) => call[0] as Record<string, unknown>)
    .filter((message) => message.type === type);
}

describe("LinkedIn post save button", () => {
  let sendMessage: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    listeners.abort();
    listeners = new AbortController();
    document.body.innerHTML = feedHtml();
    sendMessage = resetChromeMock().runtime.sendMessage;
    resetSavedCache();
  });

  it("builds the permalink for a URN", () => {
    expect(postUrl("urn:li:share:42")).toBe("https://www.linkedin.com/feed/update/urn:li:share:42/");
  });

  it("reads the author from the post menu label", () => {
    expect(postAuthor(document.getElementById("post-a")!)).toBe("Ada Lovelace");
    expect(postAuthor(document.getElementById("ad")!)).toBeUndefined();
  });

  it("adds one button per real post, next to the post menu, and is idempotent", () => {
    expect(injectButtons(document)).toBe(1);
    expect(injectButtons(document)).toBe(0);

    const button = document.querySelector<HTMLButtonElement>(SAVE_BUTTON)!;
    expect(button.closest("#post-a")).not.toBeNull();
    expect(button.nextElementSibling?.getAttribute("aria-label")).toBe(
      "Open control menu for post by Ada Lovelace",
    );
    expect(button.getAttribute("aria-label")).toBe("Save to AgentMark");
    // Level with the post menu at the top of the header, not centred in it.
    expect(button.style.alignSelf).toBe("flex-start");
    // The outlined AgentMark badge, not a bare bookmark glyph like LinkedIn's own Save.
    expect(button.querySelector('[data-part="outline"]')).not.toBeNull();
    expect(button.querySelector('[data-part="badge"]')).toBeNull();
    expect(document.querySelector(`#ad ${SAVE_BUTTON}`)).toBeNull();
  });

  it("lines the button up with the post menu's vertical centre", () => {
    const rect = (top: number, height: number) =>
      ({ top, height, bottom: top + height, left: 0, right: 0, width: 32, x: 0, y: top, toJSON: () => ({}) }) as DOMRect;
    const button = document.createElement("button");
    const menu = document.createElement("button");
    button.getBoundingClientRect = () => rect(184, 32); // centre 200
    menu.getBoundingClientRect = () => rect(196, 24); // centre 208

    expect(alignVertically(button, menu)).toBe(8);
    expect(button.style.position).toBe("relative");
    expect(button.style.top).toBe("8px");

    // Not laid out yet: left untouched rather than moved by a bogus measurement.
    menu.getBoundingClientRect = () => rect(0, 0);
    expect(alignVertically(button, menu)).toBe(0);
    expect(button.style.top).toBe("0px");
  });

  it("accepts only well-formed URNs from the page", () => {
    const post = document.getElementById("post-a")!;
    expect(resolvePostUrn(post)).toBeUndefined();

    post.setAttribute(URN_ATTR, "urn:li:activity:stale");
    answerUrnRequests(() => "javascript:alert(1)");
    expect(resolvePostUrn(post)).toBeUndefined();
  });

  it("falls back to the address bar only on a post's own page", () => {
    const post = document.getElementById("post-a")!;
    expect(resolvePostUrl(post, fakeLocation("https://www.linkedin.com/feed/foryou/"))).toBeUndefined();
    expect(
      resolvePostUrl(post, fakeLocation("https://www.linkedin.com/posts/ada_a-post-activity-1-x/")),
    ).toBe("https://www.linkedin.com/posts/ada_a-post-activity-1-x/");
  });

  it("snapshots the post with its comments and without AgentMark's own markup", () => {
    injectButtons(document);
    const post = document.getElementById("post-a")!;
    post.setAttribute(URN_ATTR, "urn:li:share:42");

    const html = buildPostSnapshot(post, "LinkedIn post by Ada <Lovelace>");

    expect(html).toContain("<title>LinkedIn post by Ada &lt;Lovelace></title>");
    expect(html).toContain("The engine weaves algebraic patterns.");
    expect(html).toContain("Nice post");
    expect(html).not.toContain("data-agentmark");
    expect(html).not.toContain("Promoted content");
  });

  it("hands the post to the popup and shows the result reported back", async () => {
    answerUrnRequests(() => "urn:li:share:42");
    sendMessage.mockResolvedValue({ success: true, popup_opened: true });
    injectButtons(document);
    const button = document.querySelector<HTMLButtonElement>(SAVE_BUTTON)!;
    const reachedLinkedIn = vi.fn();
    document.getElementById("post-a")!.addEventListener("click", reachedLinkedIn);

    button.click();
    expect(button.dataset.state).toBe("saving");
    // Still hollow, now light blue, while the save is in flight.
    expect(button.querySelector('[data-part="outline"]')).not.toBeNull();
    expect(button.querySelector('[data-part="plus"]')).toBeNull();
    // LinkedIn is white: the default, stronger indigo.
    expect(button.style.color).toBe("rgb(129, 140, 248)");
    await flush();

    expect(reachedLinkedIn).not.toHaveBeenCalled();
    expect(sent(sendMessage, "lookup_urls")).toHaveLength(1);
    expect(sent(sendMessage, "open_save_popup")).toHaveLength(1);
    expect(sendMessage).toHaveBeenCalledTimes(2);
    const message = sent(sendMessage, "open_save_popup")[0] as Record<string, string>;
    expect(message.type).toBe("open_save_popup");
    expect(message.url).toBe("https://www.linkedin.com/feed/update/urn:li:share:42/");
    expect(message.title).toBe("LinkedIn post by Ada Lovelace");
    expect(message.page_html).toContain("The engine weaves algebraic patterns.");
    // Still saving: the popup is doing the work.
    expect(button.dataset.state).toBe("saving");

    handleSaveResult({ type: "agentmark:save-result", url: "https://other.example/", success: true });
    expect(button.dataset.state).toBe("saving");

    vi.useFakeTimers();
    try {
      handleSaveResult({ type: "agentmark:save-result", url: message.url, success: true });
      expect(button.dataset.state).toBe("saved");
      expect(button.getAttribute("aria-label")).toBe("Saved to AgentMark");
      // Solid brand blue with a tick at first...
      expect(button.style.color).toBe("rgb(79, 70, 229)");
      expect(button.querySelector('[data-part="badge"]')).not.toBeNull();
      expect(button.querySelector('[data-part="tick"]')).not.toBeNull();
      // ...then the plain saved mark.
      vi.advanceTimersByTime(SAVED_TICK_MS);
      expect(button.querySelector('[data-part="badge"]')).not.toBeNull();
      expect(button.querySelector('[data-part="tick"]')).toBeNull();
      expect(button.querySelector('[data-part="plus"]')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("shows a failure reported back from the popup's save", async () => {
    answerUrnRequests(() => "urn:li:share:43");
    sendMessage.mockResolvedValue({ success: true, popup_opened: true });
    injectButtons(document);
    const button = document.querySelector<HTMLButtonElement>(SAVE_BUTTON)!;

    button.click();
    await flush();
    handleSaveResult({
      type: "agentmark:save-result",
      url: "https://www.linkedin.com/feed/update/urn:li:share:43/",
      success: false,
      error: "Native host has exited",
    });

    expect(button.dataset.state).toBe("error");
    expect(button.title).toBe("AgentMark save failed: Native host has exited");
  });

  it("shows success when the service worker saved directly without a popup", async () => {
    answerUrnRequests(() => "urn:li:share:42");
    sendMessage.mockResolvedValue({ success: true, data: { type: "save_result" } });
    injectButtons(document);
    const button = document.querySelector<HTMLButtonElement>(SAVE_BUTTON)!;

    button.click();
    await flush();

    expect(button.dataset.state).toBe("saved");
  });

  it("shows the error when the save fails", async () => {
    answerUrnRequests(() => "urn:li:share:42");
    sendMessage.mockResolvedValue({ success: false, error: "Native host has exited" });
    injectButtons(document);
    const button = document.querySelector<HTMLButtonElement>(SAVE_BUTTON)!;

    button.click();
    await flush();

    expect(button.dataset.state).toBe("error");
    expect(button.title).toBe("AgentMark save failed: Native host has exited");
    expect(button.disabled).toBe(false);
  });

  it("looks the post up on a later pass once its link can be resolved", async () => {
    sendMessage.mockResolvedValue({
      success: true,
      data: { type: "lookup_urls_result", saved: ["https://www.linkedin.com/feed/update/urn:li:share:42/"] },
    });
    // First pass: the MAIN-world helper is not installed yet.
    injectButtons(document);
    const button = document.querySelector<HTMLButtonElement>(SAVE_BUTTON)!;
    await flush();
    expect(sent(sendMessage, "lookup_urls")).toHaveLength(0);
    expect(button.dataset.state).toBe("idle");

    // A later pass resolves it and paints the saved look.
    answerUrnRequests(() => "urn:li:share:42");
    await checkSavedPosts(document, linkedinAdapter);
    expect(sent(sendMessage, "lookup_urls")).toEqual([
      { type: "lookup_urls", urls: ["https://www.linkedin.com/feed/update/urn:li:share:42/"] },
    ]);
    expect(button.dataset.state).toBe("saved");
    expect(button.getAttribute("aria-label")).toBe("Saved to AgentMark — click to update");

    // Checked: no further lookups for it.
    await checkSavedPosts(document, linkedinAdapter);
    expect(sent(sendMessage, "lookup_urls")).toHaveLength(1);
  });

  it("stops trying to resolve a post's link after three passes", async () => {
    for (let pass = 0; pass < 3; pass++) {
      injectButtons(document);
      await flush();
    }
    // The helper turns up too late: the button has given up.
    answerUrnRequests(() => "urn:li:share:42");
    await checkSavedPosts(document, linkedinAdapter);

    expect(sendMessage).not.toHaveBeenCalled();
    const button = document.querySelector<HTMLButtonElement>(SAVE_BUTTON)!;
    expect(button.getAttribute("data-agentmark-lookups")).toBe("3");
    expect(button.dataset.state).toBe("idle");
  });

  it("does not save when the post's link cannot be found", async () => {
    injectButtons(document);
    const button = document.querySelector<HTMLButtonElement>(SAVE_BUTTON)!;

    button.click();
    await flush();

    expect(sendMessage).not.toHaveBeenCalled();
    expect(button.dataset.state).toBe("error");
    expect(button.title).toContain("could not find this post's link");
  });
});
