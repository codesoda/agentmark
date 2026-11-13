/**
 * @vitest-environment jsdom
 */
import { readFileSync } from "fs";
import { resolve } from "path";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";
import { buildSnapshot, injectSaveButtons, resetSavedCache, savePost } from "./save-button";
import {
  isPrivateRedditPage,
  oldRedditAdapter,
  redditAdapter,
  redditPermalink,
} from "./reddit-posts";
import { PRIVACY } from "./private-content";

const SAVE_BUTTON = "[data-agentmark-save]";

/** Trimmed real markup; see the comment at the top of each file. */
function fixture(name: string): string {
  return readFileSync(resolve(__dirname, "fixtures/reddit", name), "utf-8");
}

function at(href: string): void {
  window.history.replaceState(null, "", href);
}

function ownButton(item: Element): HTMLButtonElement | undefined {
  const postSelector = item.matches("shreddit-post, shreddit-comment")
    ? redditAdapter.postSelector
    : oldRedditAdapter.postSelector;
  return Array.from(item.querySelectorAll<HTMLButtonElement>(SAVE_BUTTON)).find(
    (button) => button.closest(postSelector) === item,
  );
}

const FEED_POST = "https://www.reddit.com/r/programming/comments/1xtrm8s/emudhras_official_incident_report_on_how_they/";
const COMMENT = "https://www.reddit.com/r/programming/comments/1xtrm8s/comment/19shnsfed7fgg/";
const REPLY = "https://www.reddit.com/r/programming/comments/1xtrm8s/comment/19sid877abgn4/";

beforeEach(() => {
  resetChromeMock();
  resetSavedCache();
  document.body.innerHTML = "";
  at("/");
});

describe("redditPermalink", () => {
  it("makes post permalinks absolute on www.reddit.com, without query or hash", () => {
    expect(redditPermalink("/r/programming/comments/1xtrm8s/emudhras_slug/")).toBe(
      "https://www.reddit.com/r/programming/comments/1xtrm8s/emudhras_slug/",
    );
    expect(redditPermalink("https://old.reddit.com/r/rust/comments/abc123/a_post/?sort=top#x")).toBe(
      "https://www.reddit.com/r/rust/comments/abc123/a_post/",
    );
    expect(redditPermalink("/r/rust/comments/abc123")).toBe("https://www.reddit.com/r/rust/comments/abc123/");
    expect(redditPermalink("/user/someone/comments/abc123/hi/")).toBe(
      "https://www.reddit.com/user/someone/comments/abc123/hi/",
    );
  });

  it("gives a comment the same URL from new and old Reddit", () => {
    const fromNew = redditPermalink("/r/programming/comments/1006dz8/comment/j2ggh92/");
    const fromOld = redditPermalink("/r/programming/comments/1006dz8/i_did_advent_of_code_on_a_playstation/j2ggh92/");
    expect(fromNew).toBe("https://www.reddit.com/r/programming/comments/1006dz8/comment/j2ggh92/");
    expect(fromOld).toBe(fromNew);
  });

  it("rejects anything that is not a post or comment on Reddit", () => {
    expect(redditPermalink(undefined)).toBeUndefined();
    expect(redditPermalink("")).toBeUndefined();
    expect(redditPermalink("/r/programming/")).toBeUndefined();
    expect(redditPermalink("/user/someone/")).toBeUndefined();
    expect(redditPermalink("https://example.com/r/x/comments/abc/")).toBeUndefined();
    expect(redditPermalink("https://reddit.com.evil.example/r/x/comments/abc/")).toBeUndefined();
  });
});

describe("isPrivateRedditPage", () => {
  it("treats inbox, notification and chat pages as private", () => {
    const page = (hostname: string, pathname: string) => isPrivateRedditPage({ hostname, pathname });
    expect(page("www.reddit.com", "/message/inbox/")).toBe(true);
    expect(page("old.reddit.com", "/message/messages/")).toBe(true);
    expect(page("www.reddit.com", "/notifications")).toBe(true);
    expect(page("www.reddit.com", "/chat/room/abc")).toBe(true);
    expect(page("chat.reddit.com", "/")).toBe(true);
    // Mod mail (review finding #6).
    expect(page("www.reddit.com", "/mail/inbox")).toBe(true);
    expect(page("mod.reddit.com", "/")).toBe(true);
    expect(page("mod.reddit.com", "/mail/all")).toBe(true);
    expect(page("www.reddit.com", "/r/programming/comments/abc/x/")).toBe(false);
    expect(page("www.reddit.com", "/r/messages/")).toBe(false);
  });
});

describe("www.reddit.com feed", () => {
  beforeEach(() => {
    document.body.innerHTML = fixture("new-feed.html");
    at("/r/programming/");
  });

  it("adds one button per post, shown in the action row's right-hand slot", () => {
    expect(injectSaveButtons(document, redditAdapter)).toBe(3);
    expect(injectSaveButtons(document, redditAdapter)).toBe(0);

    for (const post of Array.from(document.querySelectorAll("shreddit-post"))) {
      const button = ownButton(post)!;
      expect(button.parentElement).toBe(post);
      expect(button.slot).toBe("promote-post-button");
      expect(button.style.width).toBe("32px");
      // Above the card-wide link Reddit lays over each feed post.
      expect(button.style.position).toBe("relative");
      expect(button.dataset.idleColor).toContain("--color-neutral-content-weak");
    }
  });

  it("reads each post's permalink, title and author", () => {
    const post = document.getElementById("t3_1xtrm8s")!;
    expect(redditAdapter.resolveUrl(post, window.location)).toBe(FEED_POST);
    expect(redditAdapter.title(post)).toBe(
      "eMudhra's official incident report on how they ended up with private key 4",
    );
    post.removeAttribute("post-title");
    expect(redditAdapter.title(post)).toBe("Reddit post by u/alexeyr");
  });

  it("snapshots just the post in a feed, text body included", () => {
    const post = document.getElementById("t3_1xu43b7")!;
    injectSaveButtons(document, redditAdapter);

    const html = buildSnapshot(post, redditAdapter.title(post), redditAdapter);

    expect(html).toContain("How can programmers solve problems they have never encountered before?");
    expect(html).toContain('slot="text-body"');
    expect(html).not.toContain("data-agentmark-save");
    expect(html).not.toContain("t3_1xtrm8s");
  });

  it("adds buttons to posts loaded later by infinite scroll", async () => {
    vi.useFakeTimers();
    try {
      const { watchPosts } = await import("./save-button");
      const observer = watchPosts(document, redditAdapter);
      const more = document.createElement("article");
      more.innerHTML = `<shreddit-post id="t3_new" permalink="/r/programming/comments/new1/later/" post-title="Later"></shreddit-post>`;
      document.querySelector("shreddit-feed")!.append(more);
      await vi.advanceTimersByTimeAsync(300);
      observer.disconnect();
      expect(ownButton(document.getElementById("t3_new")!)).toBeDefined();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("www.reddit.com post page", () => {
  beforeEach(() => {
    document.body.innerHTML = fixture("new-comments.html");
    at("/r/programming/comments/1xtrm8s/emudhras_official_incident_report_on_how_they/");
  });

  it("gives the post and each comment, replies included, a button of its own", () => {
    expect(injectSaveButtons(document, redditAdapter)).toBe(3);
    expect(injectSaveButtons(document, redditAdapter)).toBe(0);

    const [comment, reply] = Array.from(document.querySelectorAll("shreddit-comment"));
    const commentButton = ownButton(comment)!;
    const replyButton = ownButton(reply)!;
    expect(commentButton.parentElement?.tagName).toBe("SHREDDIT-COMMENT-ACTION-ROW");
    expect(commentButton.parentElement?.getAttribute("comment-id")).toBe("t1_19shnsfed7fgg");
    expect(commentButton.slot).toBe("comment-share");
    expect(replyButton.parentElement?.getAttribute("comment-id")).toBe("t1_19sid877abgn4");
  });

  it("reads comment permalinks and authors", () => {
    const [comment, reply] = Array.from(document.querySelectorAll("shreddit-comment"));
    expect(redditAdapter.resolveUrl(comment, window.location)).toBe(COMMENT);
    expect(redditAdapter.resolveUrl(reply, window.location)).toBe(REPLY);
    expect(redditAdapter.title(comment)).toBe("Reddit comment by u/tsammons");
    reply.setAttribute("author", "[deleted]");
    expect(redditAdapter.title(reply)).toBe("Reddit comment");
  });

  it("snapshots the post with its comments, and a comment with its replies", () => {
    injectSaveButtons(document, redditAdapter);
    const post = document.querySelector("shreddit-post")!;
    const [comment, reply] = Array.from(document.querySelectorAll("shreddit-comment"));

    const postHtml = buildSnapshot(post, "Post", redditAdapter);
    expect(postHtml).toContain("<main");
    expect(postHtml).toContain("I hate when I accidentally declare a short in Python.");

    const commentHtml = buildSnapshot(comment, "Comment", redditAdapter);
    expect(commentHtml).toContain("I hate when I accidentally declare a short in Python.");
    expect(commentHtml).toContain('thingid="t1_19sid877abgn4"');
    expect(commentHtml).not.toContain("post-title=");
    expect(commentHtml).not.toContain("data-agentmark-save");

    const replyHtml = buildSnapshot(reply, "Reply", redditAdapter);
    expect(replyHtml).not.toContain("I hate when I accidentally declare a short in Python.");
  });

  it("never puts chat into a snapshot", () => {
    const main = document.querySelector("main")!;
    const chat = document.createElement("div");
    chat.id = "chat-window";
    chat.textContent = "A private chat message";
    main.append(chat);
    const frame = document.createElement("iframe");
    frame.src = "https://chat.reddit.com/room/x";
    main.append(frame);

    const html = buildSnapshot(document.querySelector("shreddit-post")!, "Post", redditAdapter);

    expect(html).toContain("I hate when I accidentally declare a short in Python.");
    expect(html).not.toContain("private chat");
    expect(html).not.toContain("chat.reddit.com");
    expect(document.querySelectorAll(PRIVACY.redditSelector)).toHaveLength(3);
  });

  it("hands the comment's permalink and snapshot to the popup", async () => {
    injectSaveButtons(document, redditAdapter);
    const sendMessage = vi.mocked(chrome.runtime.sendMessage);
    sendMessage.mockResolvedValue({ success: true, popup_opened: true } as never);
    const comment = document.querySelector("shreddit-comment")!;

    await savePost(comment, ownButton(comment)!, redditAdapter);

    const message = sendMessage.mock.calls
      .map((call) => call[0] as unknown as Record<string, unknown>)
      .find((m) => m.type === "open_save_popup")!;
    expect(message.url).toBe(COMMENT);
    expect(message.title).toBe("Reddit comment by u/tsammons");
    expect(String(message.page_html)).toContain("declare a short in Python");
  });

  it("adds no buttons on inbox and notification pages", () => {
    at("/message/inbox/");
    expect(injectSaveButtons(document, redditAdapter)).toBe(0);
    at("/notifications");
    expect(injectSaveButtons(document, redditAdapter)).toBe(0);
  });
});

describe("old.reddit.com", () => {
  it("adds a small button to each listed post's link row", () => {
    document.body.innerHTML = fixture("old-listing.html");
    at("/r/programming/");

    expect(injectSaveButtons(document, oldRedditAdapter)).toBe(2);
    expect(injectSaveButtons(document, oldRedditAdapter)).toBe(0);

    const post = document.getElementById("thing_t3_1vvrbyb")!;
    const button = ownButton(post)!;
    expect(button.parentElement?.matches("ul.flat-list.buttons")).toBe(true);
    expect(button.closest(".thing")).toBe(post);
    expect(button.style.width).toBe("16px");
    expect(button.dataset.idleColor).toBe("#888");
    expect(oldRedditAdapter.resolveUrl(post, window.location)).toBe(
      "https://www.reddit.com/r/programming/comments/1vvrbyb/the_unlikely_linux_macro/",
    );
    expect(oldRedditAdapter.title(post)).toBe("The unlikely Linux macro");
    expect(oldRedditAdapter.snapshotRoot(post)).toBe(post);
  });

  it("skips promoted posts", () => {
    document.body.innerHTML = fixture("old-listing.html");
    document.getElementById("thing_t3_1vvrbyb")!.classList.add("promoted");
    expect(injectSaveButtons(document, oldRedditAdapter)).toBe(1);
  });

  it("gives the post and every comment, replies included, their own button", () => {
    document.body.innerHTML = fixture("old-comments.html");
    at("/r/programming/comments/1006dz8/i_did_advent_of_code_on_a_playstation/");

    const things = Array.from(document.querySelectorAll(oldRedditAdapter.postSelector));
    expect(things).toHaveLength(4);
    expect(injectSaveButtons(document, oldRedditAdapter)).toBe(4);
    expect(injectSaveButtons(document, oldRedditAdapter)).toBe(0);

    const reply = document.getElementById("thing_t1_j2iis83")!;
    expect(ownButton(reply)!.closest(".thing")).toBe(reply);
    expect(oldRedditAdapter.resolveUrl(reply, window.location)).toBe(
      "https://www.reddit.com/r/programming/comments/1006dz8/comment/j2iis83/",
    );
    expect(oldRedditAdapter.title(reply)).toBe("Reddit comment by u/bvisness");
  });

  it("snapshots a post page with its comments, and a comment with its replies only", () => {
    document.body.innerHTML = fixture("old-comments.html");
    injectSaveButtons(document, oldRedditAdapter);
    const post = document.getElementById("thing_t3_1006dz8")!;
    const parent = document.getElementById("thing_t1_j2h9u2r")!;

    expect(oldRedditAdapter.snapshotRoot(post)).toBe(document.querySelector(".content"));
    expect(oldRedditAdapter.title(post)).toBe("I did Advent of Code on a PlayStation");

    const html = buildSnapshot(parent, "Comment", oldRedditAdapter);
    expect(html).toContain('id="thing_t1_j2iis83"');
    expect(html).not.toContain('id="thing_t1_j2h0e0w"');
    expect(html).not.toContain("data-agentmark-save");
  });
});
