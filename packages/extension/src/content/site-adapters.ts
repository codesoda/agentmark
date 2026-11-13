/**
 * Picks the save-button adapter for a page by its hostname. Bundled into the
 * posts content script only.
 */

import { hnAdapter } from "./hn-posts";
import { linkedinAdapter } from "./linkedin-posts";
import { oldRedditAdapter, redditAdapter } from "./reddit-posts";
import type { SiteAdapter } from "./save-button";
import { xAdapter } from "./x-posts";

export function adapterFor(hostname: string): SiteAdapter | undefined {
  const host = hostname.toLowerCase();
  const isSite = (domain: string) => host === domain || host.endsWith(`.${domain}`);
  if (isSite("linkedin.com")) return linkedinAdapter;
  if (isSite("x.com") || isSite("twitter.com")) return xAdapter;
  if (host === "news.ycombinator.com") return hnAdapter;
  if (host === "old.reddit.com") return oldRedditAdapter;
  if (isSite("reddit.com")) return redditAdapter;
  return undefined;
}
