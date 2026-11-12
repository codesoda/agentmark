/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, beforeEach } from "vitest";
import { findPostUrn, installUrnResolver, RESOLVE_EVENT, URN_ATTR } from "./linkedin-urn";
import * as posts from "./linkedin-posts";

/** Attach React-like fiber data to an element the way React does. */
function attachFiber(element: Element, fiber: unknown): void {
  (element as unknown as Record<string, unknown>)["__reactFiber$abc123"] = fiber;
}

describe("findPostUrn", () => {
  beforeEach(() => {
    document.body.innerHTML = `<div id="post"></div>`;
  });

  it("returns null when the element has no React data", () => {
    expect(findPostUrn(document.getElementById("post")!)).toBeNull();
  });

  it("finds a URN in the element's own props", () => {
    const post = document.getElementById("post")!;
    attachFiber(post, { memoizedProps: { update: { urn: "urn:li:activity:7518977372487045122" } } });
    expect(findPostUrn(post)).toBe("urn:li:activity:7518977372487045122");
  });

  it("recognises share and ugcPost URNs", () => {
    const post = document.getElementById("post")!;
    attachFiber(post, { memoizedProps: { id: "urn:li:share:7525423658144530432" } });
    expect(findPostUrn(post)).toBe("urn:li:share:7525423658144530432");
    attachFiber(post, { memoizedProps: { id: "urn:li:ugcPost:7525980725481955328" } });
    expect(findPostUrn(post)).toBe("urn:li:ugcPost:7525980725481955328");
  });

  it("climbs to parent components and prefers the nearest match", () => {
    const post = document.getElementById("post")!;
    attachFiber(post, {
      memoizedProps: { className: "x" },
      return: {
        memoizedProps: { urn: "urn:li:activity:111" },
        return: { memoizedProps: { urn: "urn:li:activity:999" } },
      },
    });
    expect(findPostUrn(post)).toBe("urn:li:activity:111");
  });

  it("ignores children and survives cyclic props", () => {
    const post = document.getElementById("post")!;
    const cyclic: Record<string, unknown> = {};
    cyclic.self = cyclic;
    attachFiber(post, {
      memoizedProps: { children: { urn: "urn:li:activity:222" }, cyclic },
      return: { memoizedProps: { urn: "urn:li:activity:333" } },
    });
    expect(findPostUrn(post)).toBe("urn:li:activity:333");
  });

  it("gives up when no component holds a URN", () => {
    const post = document.getElementById("post")!;
    attachFiber(post, { memoizedProps: { other: "urn:li:member:5" }, return: null });
    expect(findPostUrn(post)).toBeNull();
  });
});

describe("installUrnResolver", () => {
  it("publishes the URN on the element when asked, and clears a stale one", () => {
    document.body.innerHTML = `<div id="a"></div><div id="b" ${URN_ATTR}="urn:li:activity:1"></div>`;
    installUrnResolver(document);
    const a = document.getElementById("a")!;
    const b = document.getElementById("b")!;
    attachFiber(a, { memoizedProps: { urn: "urn:li:share:42" } });

    a.dispatchEvent(new CustomEvent(RESOLVE_EVENT, { bubbles: true }));
    b.dispatchEvent(new CustomEvent(RESOLVE_EVENT, { bubbles: true }));

    expect(a.getAttribute(URN_ATTR)).toBe("urn:li:share:42");
    expect(b.hasAttribute(URN_ATTR)).toBe(false);
  });
});

describe("contract with the isolated script", () => {
  it("uses the same event name and attribute", () => {
    expect(posts.RESOLVE_EVENT).toBe(RESOLVE_EVENT);
    expect(posts.URN_ATTR).toBe(URN_ATTR);
  });
});
