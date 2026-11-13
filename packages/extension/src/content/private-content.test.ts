/**
 * @vitest-environment jsdom
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";
import { SNAPSHOT_PRIVACY } from "../shared/page-snapshot";
import { isPrivatePageOf, PRIVACY, removePrivateElements } from "./private-content";
import { redditAdapter } from "./reddit-posts";
import { buildSnapshot, injectSaveButtons, resetSavedCache, savePost } from "./save-button";
import { xAdapter } from "./x-posts";

const page = (hostname: string, pathname: string) => ({ hostname, pathname });

beforeEach(() => {
  resetChromeMock();
  resetSavedCache();
  document.body.innerHTML = "";
  window.history.replaceState(null, "", "/");
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("private content rules", () => {
  it("are the same in the content script and the whole-page snapshot", () => {
    // Content scripts cannot import shared code, so each bundle has its own copy.
    expect(PRIVACY).toEqual(SNAPSHOT_PRIVACY);
  });

  it("treat X's messages pages as private", () => {
    expect(isPrivatePageOf("x.com", page("x.com", "/messages"))).toBe(true);
    expect(isPrivatePageOf("x.com", page("x.com", "/messages/123-456"))).toBe(true);
    expect(isPrivatePageOf("x.com", page("x.com", "/i/chat/123"))).toBe(true);
    expect(isPrivatePageOf("x.com", page("x.com", "/home"))).toBe(false);
    expect(isPrivatePageOf("x.com", page("x.com", "/messagesfan/status/1"))).toBe(false);
  });

  it("treat Reddit's mod mail as private", () => {
    expect(isPrivatePageOf("reddit.com", page("www.reddit.com", "/mail/inbox"))).toBe(true);
    expect(isPrivatePageOf("reddit.com", page("mod.reddit.com", "/r/x/about"))).toBe(true);
    expect(isPrivatePageOf("reddit.com", page("www.reddit.com", "/r/mail/"))).toBe(false);
  });

  it("cut X's docked messages drawer out", () => {
    const root = document.createElement("div");
    root.innerHTML = `<article><p>A public post</p></article>
      <div data-testid="DMDrawer"><div data-testid="DMDrawerHeader">Messages</div><p>A private conversation</p></div>`;
    removePrivateElements(root, false);
    expect(root.innerHTML).toContain("A public post");
    expect(root.innerHTML).not.toContain("Messages");
    expect(root.innerHTML).not.toContain("private");
  });

  it("refuse a root that is itself private", () => {
    const root = document.createElement("div");
    root.setAttribute("data-testid", "DMDrawer");
    expect(() => removePrivateElements(root, false)).toThrow();
  });
});

describe("snapshots fail closed", () => {
  function tweet(): Element {
    document.body.innerHTML = `<article data-testid="tweet" id="t">
      <a href="/ada/status/1"><time>1h</time></a><p>Post text</p><div role="group"></div></article>`;
    return document.getElementById("t")!;
  }

  it("give no snapshot when private elements cannot be removed", () => {
    const post = tweet();
    vi.spyOn(Element.prototype, "matches").mockImplementation(() => {
      throw new SyntaxError("not a valid selector");
    });
    expect(buildSnapshot(post, "Post", xAdapter)).toBeUndefined();
  });

  it("send the save without a snapshot then", async () => {
    const post = tweet();
    injectSaveButtons(document, xAdapter);
    const button = post.querySelector<HTMLButtonElement>("[data-agentmark-save]")!;
    const sendMessage = vi.mocked(chrome.runtime.sendMessage);
    sendMessage.mockResolvedValue({ success: true, popup_opened: true } as never);
    vi.spyOn(Element.prototype, "matches").mockImplementation(() => {
      throw new SyntaxError("not a valid selector");
    });

    await savePost(post, button, xAdapter);

    const message = sendMessage.mock.calls.find((call) => (call[0] as { type?: string }).type === "open_save_popup")?.[0] as {
      page_html?: string;
    };
    expect(message).toBeDefined();
    expect(message.page_html).toBeUndefined();
  });
});

describe("private pages get no buttons", () => {
  it("on X's messages pages", () => {
    window.history.replaceState(null, "", "/messages/123-456");
    document.body.innerHTML = `<article data-testid="tweet"><a href="/ada/status/1"><time>1h</time></a><div role="group"></div></article>`;
    expect(injectSaveButtons(document, xAdapter)).toBe(0);
  });

  it("on Reddit's mod mail", () => {
    window.history.replaceState(null, "", "/mail/inbox");
    document.body.innerHTML = `<shreddit-post permalink="/r/x/comments/abc/t/"></shreddit-post>`;
    expect(injectSaveButtons(document, redditAdapter)).toBe(0);
  });
});
