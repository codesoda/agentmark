import { describe, it, expect } from "vitest";
import { hnAdapter } from "./hn-posts";
import { linkedinAdapter } from "./linkedin-posts";
import { oldRedditAdapter, redditAdapter } from "./reddit-posts";
import { adapterFor } from "./site-adapters";
import { xAdapter } from "./x-posts";

describe("adapterFor", () => {
  it("picks the adapter by hostname", () => {
    expect(adapterFor("www.linkedin.com")).toBe(linkedinAdapter);
    expect(adapterFor("x.com")).toBe(xAdapter);
    expect(adapterFor("twitter.com")).toBe(xAdapter);
    expect(adapterFor("news.ycombinator.com")).toBe(hnAdapter);
    expect(adapterFor("www.reddit.com")).toBe(redditAdapter);
    expect(adapterFor("WWW.Reddit.com")).toBe(redditAdapter);
    expect(adapterFor("reddit.com")).toBe(redditAdapter);
    expect(adapterFor("old.reddit.com")).toBe(oldRedditAdapter);
  });

  it("ignores other sites and lookalikes", () => {
    expect(adapterFor("example.com")).toBeUndefined();
    expect(adapterFor("notreddit.com")).toBeUndefined();
    expect(adapterFor("reddit.com.evil.example")).toBeUndefined();
    expect(adapterFor("ycombinator.com")).toBeUndefined();
  });
});
