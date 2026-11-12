import { hnAdapter } from "./hn-posts";
import { linkedinAdapter } from "./linkedin-posts";
import { watchPosts, type SiteAdapter } from "./save-button";
import { xAdapter } from "./x-posts";

function adapterFor(hostname: string): SiteAdapter | undefined {
  const host = hostname.toLowerCase();
  const isSite = (domain: string) => host === domain || host.endsWith(`.${domain}`);
  if (isSite("linkedin.com")) return linkedinAdapter;
  if (isSite("x.com") || isSite("twitter.com")) return xAdapter;
  if (host === "news.ycombinator.com") return hnAdapter;
  return undefined;
}

const adapter = adapterFor(window.location.hostname);
if (adapter) watchPosts(document, adapter);
