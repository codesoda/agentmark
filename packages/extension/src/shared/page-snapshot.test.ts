/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, beforeEach, vi } from "vitest";
import { capturePageHtml, shouldScrollBeforeSnapshot } from "./page-snapshot";

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
    expect(await capturePageHtml(false)).toContain("<p>hello</p>");
  });

  it("never clicks anything on the page", async () => {
    document.body.innerHTML = `
      <a href="https://www.linkedin.com/feed/update/urn:li:activity:1/"><button data-testid="expandable-text-button">… more</button></a>`;
    const onClick = vi.fn();
    document.addEventListener("click", onClick, true);

    await capturePageHtml(true);

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

      const html = await capturePageHtml(false);

      expect(html).toContain("<title>Acme: Overview | LinkedIn</title>");
      expect(html).toContain('data-agentmark-source="iframe"');
      expect(html).toContain("<h1>Acme</h1>");
      expect(html).not.toContain("stale profile");
      expect(html).not.toContain("<style>");
    });

    it("keeps the normal document while the iframe is transparent", async () => {
      mount("0", `<section><h1>Acme</h1><p>${pageText}</p></section>`);

      const html = await capturePageHtml(false);

      expect(html).toContain("stale profile");
      expect(html).not.toContain("data-agentmark-source");
    });

    it("never captures the messaging overlay", async () => {
      mount(
        "1",
        `<section><h1>Acme</h1><p>${pageText}</p></section>
         <aside class="msg-overlay-container"><p>Dock preview of a private message</p></aside>`,
      );

      const visible = await capturePageHtml(false);
      expect(visible).toContain("<h1>Acme</h1>");
      expect(visible).not.toContain("private");

      mount("0", "");
      const hidden = await capturePageHtml(false);
      expect(hidden).not.toContain("private conversation");
    });

    it("keeps the normal document when the iframe holds only the messaging dock", async () => {
      mount("1", `<aside class="msg-overlay-container"><p>${pageText}</p></aside>`);

      const html = await capturePageHtml(false);

      expect(html).toContain("stale profile");
      expect(html).not.toContain("data-agentmark-source");
    });
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

    const html = await capturePageHtml(true);

    expect(Math.max(...positions)).toBe(700);
    expect(top).toBe(40);
    expect(html).toContain("profile");
  });
});
