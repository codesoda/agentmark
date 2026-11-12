/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, beforeEach, vi } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";
import { buildSnapshot, injectSaveButtons, resetSavedCache } from "./save-button";
import { resolveTweetUrl, statusUrl, tweetAuthor, xAdapter } from "./x-posts";

const SAVE_BUTTON = "[data-agentmark-save]";

function timelineHtml(): string {
  return `
    <article data-testid="tweet" id="tweet">
      <div data-testid="User-Name"><a href="/ada"><span>Ada Lovelace</span></a><span>@ada</span>
        <a href="/ada/status/1234567890"><time datetime="2026-11-10T00:00:00.000Z">2h</time></a></div>
      <div data-testid="tweetText">The engine weaves algebraic patterns.</div>
      <div role="link"><div data-testid="User-Name"><span>Charles Babbage</span></div>
        <time datetime="2026-11-09T00:00:00.000Z">Nov 9</time><div>Quoted post text</div></div>
      <div role="group" aria-label="3 replies, 5 likes">
        <button data-testid="reply"></button><button data-testid="like"></button>
      </div>
    </article>
    <article data-testid="tweet" id="promo"><div data-testid="tweetText">Promoted, no action bar</div></article>
    <article data-testid="notification" id="other"><div role="group"></div></article>`;
}

function fakeLocation(href: string): Location {
  return new URL(href) as unknown as Location;
}

const timeline = fakeLocation("https://x.com/home");

describe("X post save button", () => {
  beforeEach(() => {
    document.body.innerHTML = timelineHtml();
    resetChromeMock();
    resetSavedCache();
  });

  it("normalises status links to the canonical permalink", () => {
    expect(statusUrl("/ada/status/123", "https://x.com")).toBe("https://x.com/ada/status/123");
    expect(statusUrl("/ada/status/123/photo/1?s=20", "https://x.com")).toBe(
      "https://x.com/ada/status/123",
    );
    expect(statusUrl("https://twitter.com/ada/status/123/analytics", "https://x.com")).toBe(
      "https://x.com/ada/status/123",
    );
    expect(statusUrl("/ada", "https://x.com")).toBeUndefined();
    expect(statusUrl("/i/bookmarks", "https://x.com")).toBeUndefined();
    expect(statusUrl("/ada/status/not-a-number", "https://x.com")).toBeUndefined();
  });

  it("takes the post's own permalink, not the quoted post's", () => {
    expect(resolveTweetUrl(document.getElementById("tweet")!, timeline)).toBe(
      "https://x.com/ada/status/1234567890",
    );
  });

  it("falls back to the address bar on a post's own page only", () => {
    const promo = document.getElementById("promo")!;
    expect(resolveTweetUrl(promo, timeline)).toBeUndefined();
    expect(resolveTweetUrl(promo, fakeLocation("https://x.com/ada/status/42/photo/1"))).toBe(
      "https://x.com/ada/status/42",
    );
  });

  it("reads the author's display name", () => {
    expect(tweetAuthor(document.getElementById("tweet")!)).toBe("Ada Lovelace");
    expect(xAdapter.title(document.getElementById("tweet")!)).toBe("Post by Ada Lovelace on X");
    expect(xAdapter.title(document.getElementById("promo")!)).toBe("Post on X");
  });

  it("appends one button to each post's action bar and skips everything else", () => {
    expect(injectSaveButtons(document, xAdapter)).toBe(1);
    expect(injectSaveButtons(document, xAdapter)).toBe(0);

    const button = document.querySelector<HTMLButtonElement>(SAVE_BUTTON)!;
    const bar = document.querySelector('#tweet [role="group"]')!;
    expect(bar.lastElementChild).toBe(button);
    expect(button.querySelector("svg")?.getAttribute("data-look")).toBe("idle");
    expect(button.querySelector('[data-part="outline"]')).not.toBeNull();
    expect(button.style.color).toBe("rgb(113, 118, 123)");
    // Centred in the action bar whatever its height (it is taller on a post's own page).
    expect(button.style.alignSelf).toBe("center");
    expect(document.querySelector(`#promo ${SAVE_BUTTON}`)).toBeNull();
    expect(document.querySelector(`#other ${SAVE_BUTTON}`)).toBeNull();
  });

  it("snapshots just that post, without AgentMark's button", () => {
    injectSaveButtons(document, xAdapter);
    const html = buildSnapshot(document.getElementById("tweet")!, "Post by Ada Lovelace on X", xAdapter);

    expect(html).toContain("<title>Post by Ada Lovelace on X</title>");
    expect(html).toContain("The engine weaves algebraic patterns.");
    expect(html).toContain("Quoted post text");
    expect(html).not.toContain("data-agentmark");
    expect(html).not.toContain("Promoted");
  });

  it("hands the post to the popup with its permalink", async () => {
    const sendMessage = resetChromeMock().runtime.sendMessage;
    sendMessage.mockResolvedValue({ success: true, popup_opened: true });
    injectSaveButtons(document, xAdapter);
    const reachedX = vi.fn();
    document.getElementById("tweet")!.addEventListener("click", reachedX);

    const button = document.querySelector<HTMLButtonElement>(SAVE_BUTTON)!;
    button.click();
    // X is usually dark: the lighter indigo outline while saving.
    expect(button.dataset.state).toBe("saving");
    expect(button.style.color).toBe("rgb(165, 180, 252)");
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(reachedX).not.toHaveBeenCalled();
    const message = sendMessage.mock.calls
      .map((call) => call[0])
      .find((sent) => sent.type === "open_save_popup");
    expect(message.type).toBe("open_save_popup");
    // jsdom's own location is not a status page, so this came from the timestamp link.
    expect(message.url).toBe("https://x.com/ada/status/1234567890");
    expect(message.title).toBe("Post by Ada Lovelace on X");
    expect(message.page_html).toContain("The engine weaves algebraic patterns.");
  });
});
