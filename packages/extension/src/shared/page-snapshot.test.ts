/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { capturePageHtml, shouldScrollBeforeSnapshot, SNAPSHOT_PRIVACY } from "./page-snapshot";

const capture = (scroll: boolean) => capturePageHtml(scroll, SNAPSHOT_PRIVACY);

describe("shouldScrollBeforeSnapshot", () => {
  it("scrolls LinkedIn profiles", () => {
    expect(shouldScrollBeforeSnapshot("https://www.linkedin.com/in/someone/")).toBe(true);
    expect(shouldScrollBeforeSnapshot("https://linkedin.com/in/someone/")).toBe(true);
  });

  it("scrolls LinkedIn company pages but not their endless posts tab", () => {
    expect(shouldScrollBeforeSnapshot("https://www.linkedin.com/company/anthropic/")).toBe(true);
    expect(shouldScrollBeforeSnapshot("https://www.linkedin.com/company/anthropic/about/")).toBe(true);
    expect(shouldScrollBeforeSnapshot("https://www.linkedin.com/company/anthropic/posts/")).toBe(false);
  });

  it("does not scroll LinkedIn feeds and posts", () => {
    expect(shouldScrollBeforeSnapshot("https://www.linkedin.com/feed/")).toBe(false);
    expect(
      shouldScrollBeforeSnapshot("https://www.linkedin.com/feed/update/urn:li:activity:1/"),
    ).toBe(false);
  });

  it("leaves other sites untouched", () => {
    expect(shouldScrollBeforeSnapshot("https://example.com/in/someone")).toBe(false);
    expect(shouldScrollBeforeSnapshot("https://notlinkedin.com/in/x")).toBe(false);
  });

  it("handles missing or malformed URLs", () => {
    expect(shouldScrollBeforeSnapshot(undefined)).toBe(false);
    expect(shouldScrollBeforeSnapshot("not a url")).toBe(false);
  });
});

describe("capturePageHtml", () => {
  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("returns the page HTML", async () => {
    document.body.innerHTML = `<p>hello</p>`;
    expect(await capture(false)).toContain("<p>hello</p>");
  });

  it("never clicks anything on the page", async () => {
    document.body.innerHTML = `
      <a href="https://www.linkedin.com/feed/update/urn:li:activity:1/"><button data-testid="expandable-text-button">… more</button></a>`;
    const onClick = vi.fn();
    document.addEventListener("click", onClick, true);

    await capture(true);

    document.removeEventListener("click", onClick, true);
    expect(onClick).not.toHaveBeenCalled();
  });

  describe("LinkedIn interop iframe", () => {
    const pageText = "Acme builds rockets. ".repeat(20);

    function mount(opacity: string, frameHtml: string): void {
      document.title = "Acme: Overview | LinkedIn";
      document.body.innerHTML = `<main><h2>Previous Person</h2><p>stale profile</p></main>
        <div id="interop-outlet" data-testid="interop-shadowdom" style="visibility: visible;"></div>
        <iframe data-testid="interop-iframe" style="opacity: ${opacity};"></iframe>`;
      const outlet = document.getElementById("interop-outlet")!;
      outlet.attachShadow({ mode: "open" }).innerHTML =
        `<aside><p>${"A private conversation in the messaging overlay. ".repeat(10)}</p></aside>`;
      document.querySelector("iframe")!.contentDocument!.body.innerHTML = frameHtml;
    }

    it("captures the iframe page instead of the stale page behind it", async () => {
      mount("1", `<style>p{}</style><section><h1>Acme</h1><p>${pageText}</p></section>`);

      const html = await capture(false);

      expect(html).toContain("<title>Acme: Overview | LinkedIn</title>");
      expect(html).toContain('data-agentmark-source="iframe"');
      expect(html).toContain("<h1>Acme</h1>");
      expect(html).not.toContain("stale profile");
      expect(html).not.toContain("<style>");
    });

    it("keeps the normal document while the iframe is transparent", async () => {
      mount("0", `<section><h1>Acme</h1><p>${pageText}</p></section>`);

      const html = await capture(false);

      expect(html).toContain("stale profile");
      expect(html).not.toContain("data-agentmark-source");
    });

    it("never captures the messaging overlay", async () => {
      mount(
        "1",
        `<section><h1>Acme</h1><p>${pageText}</p></section>
         <aside class="msg-overlay-container"><p>Dock preview of a private message</p></aside>`,
      );

      const visible = await capture(false);
      expect(visible).toContain("<h1>Acme</h1>");
      expect(visible).not.toContain("private");

      mount("0", "");
      const hidden = await capture(false);
      expect(hidden).not.toContain("private conversation");
    });

    it("keeps the normal document when the iframe holds only the messaging dock", async () => {
      mount("1", `<aside class="msg-overlay-container"><p>${pageText}</p></aside>`);

      const html = await capture(false);

      expect(html).toContain("stale profile");
      expect(html).not.toContain("data-agentmark-source");
    });
  });

  describe("Reddit", () => {
    afterEach(() => {
      vi.restoreAllMocks();
    });

    /** jsdom serves tests from localhost; pretend to be on this Reddit page. */
    function onReddit(pathname: string): void {
      vi.spyOn(window, "location", "get").mockReturnValue({
        ...window.location,
        hostname: "www.reddit.com",
        pathname,
      } as Location);
    }

    it("cuts chat, inbox and notification elements out of the page", async () => {
      onReddit("/r/programming/comments/1xtrm8s/x/");
      document.body.innerHTML = `
        <reddit-header-large><div id="reddit-chat"><p>Chat: a private message</p></div>
          <reddit-notification-dropdown><p>Inbox: a private reply</p></reddit-notification-dropdown></reddit-header-large>
        <main><shreddit-post post-title="A post"><div slot="text-body"><p>The post body</p></div></shreddit-post>
          <shreddit-comment><div slot="comment"><p>A public comment</p></div></shreddit-comment></main>
        <shreddit-chat-window><p>Another private chat</p></shreddit-chat-window>`;

      const html = await capture(false);

      expect(html).toContain("The post body");
      expect(html).toContain("A public comment");
      expect(html).not.toContain("private");
      // The live page is left alone.
      expect(document.getElementById("reddit-chat")).not.toBeNull();
    });

    it("gives no snapshot of inbox, notification or chat pages", async () => {
      document.body.innerHTML = `<main><p>A private message in the inbox</p></main>`;
      for (const path of ["/message/inbox/", "/message/messages/x", "/notifications", "/chat/room/x"]) {
        onReddit(path);
        expect(await capture(false)).toBe("");
      }
    });
  });

  describe("X and mod mail (review finding #6)", () => {
    afterEach(() => {
      vi.restoreAllMocks();
    });

    function at(hostname: string, pathname: string): void {
      vi.spyOn(window, "location", "get").mockReturnValue({ ...window.location, hostname, pathname } as Location);
    }

    it("cuts X's docked messages drawer out of the page", async () => {
      at("x.com", "/ada/status/1");
      document.body.innerHTML = `<main><article><p>The post</p></article></main>
        <div data-testid="DMDrawer"><div data-testid="DMDrawerHeader">Messages</div><p>A private conversation</p></div>`;
      const html = await capture(false);
      expect(html).toContain("The post");
      expect(html).not.toContain("private conversation");
      expect(html).not.toContain("DMDrawer");
    });

    it("gives no snapshot of X messages, Reddit mod mail or mod.reddit.com", async () => {
      document.body.innerHTML = `<main><p>A private message</p></main>`;
      for (const [host, path] of [
        ["x.com", "/messages"],
        ["twitter.com", "/messages/123-456"],
        ["www.reddit.com", "/mail/inbox"],
        ["mod.reddit.com", "/r/x/"],
      ]) {
        at(host, path);
        expect(await capture(false)).toBe("");
      }
    });
  });

  describe("fails closed", () => {
    afterEach(() => {
      vi.restoreAllMocks();
    });

    it("gives no snapshot when a private rule is not a valid selector", async () => {
      document.body.innerHTML = `<main><p>Page</p><div class="msg-overlay">Private</div></main>`;
      expect(await capturePageHtml(false, { ...SNAPSHOT_PRIVACY, selector: "[[[" })).toBe("");
    });

    it("gives no snapshot when removing private elements throws", async () => {
      document.body.innerHTML = `<main><p>Page</p><div class="msg-overlay">Private</div></main>`;
      const matches = Element.prototype.matches;
      let calls = 0;
      // The first check passes; filtering the page then throws.
      vi.spyOn(Element.prototype, "matches").mockImplementation(function (this: Element, selector: string) {
        if (++calls > 1) throw new Error("boom");
        return matches.call(this, selector);
      });
      expect(await capture(false)).toBe("");
    });
  });

  it("leaves pages without private elements as they are", async () => {
    document.body.innerHTML = `<main><p>Messages from the editor</p><my-chat-widget>Public chat demo</my-chat-widget></main>`;
    const html = await capture(false);
    expect(html).toContain("Public chat demo");
  });

  it("scrolls through the page and restores the scroll position", async () => {
    document.body.innerHTML = `<main><p>profile</p></main>`;
    const root = document.documentElement;
    Object.defineProperty(document, "scrollingElement", { value: root, configurable: true });
    Object.defineProperty(root, "clientHeight", { value: 500, configurable: true });
    Object.defineProperty(root, "scrollHeight", { value: 1200, configurable: true });
    const positions: number[] = [];
    let top = 40;
    Object.defineProperty(root, "scrollTop", {
      configurable: true,
      get: () => top,
      set: (value: number) => {
        top = Math.min(value, 700);
        positions.push(top);
      },
    });

    const html = await capture(true);

    expect(Math.max(...positions)).toBe(700);
    expect(top).toBe(40);
    expect(html).toContain("profile");
  });
});
