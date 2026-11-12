/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, beforeEach, vi } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";
import { handleSaveResult, injectSaveButtons, resetSavedCache } from "./save-button";
import { hnAdapter, itemUrl } from "./hn-posts";

const SAVE_BUTTON = "[data-agentmark-save]";
const BRAND_BLUE = "rgb(79, 70, 229)";

/** A Hacker News thread with one comment per ID. */
function threadHtml(ids: number[]): string {
  const rows = ids
    .map(
      (id) => `<tr class="athing comtr" id="${id}"><td><span class="comhead">
        <a href="user?id=u${id}">u${id}</a><span class="navs"></span></span>
        <div class="comment">Comment ${id}</div></td></tr>`,
    )
    .join("");
  return `<table class="comment-tree"><tbody>${rows}</tbody></table>`;
}

function buttonFor(id: number): HTMLButtonElement {
  return document.querySelector<HTMLButtonElement>(`[id="${id}"] ${SAVE_BUTTON}`)!;
}

function lookupAnswer(saved: string[]) {
  return { success: true, data: { type: "lookup_urls_result", saved } };
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

function expectPlainSaved(button: HTMLButtonElement): void {
  expect(button.dataset.state).toBe("saved");
  expect(button.style.color).toBe(BRAND_BLUE);
  expect(button.querySelector('[data-part="badge"]')).not.toBeNull();
  expect(button.querySelector('[data-part="tick"]')).toBeNull();
  expect(button.querySelector('[data-part="plus"]')).toBeNull();
  expect(button.getAttribute("aria-label")).toBe("Saved to AgentMark — click to update");
  expect(button.title).toBe("Saved to AgentMark — click to update");
}

describe("already-saved lookup", () => {
  let sendMessage: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    document.body.innerHTML = threadHtml([1, 2, 3]);
    sendMessage = resetChromeMock().runtime.sendMessage;
    resetSavedCache();
  });

  it("asks once per pass and shows bookmarked posts plainly saved", async () => {
    sendMessage.mockResolvedValue(lookupAnswer([itemUrl("2")!]));

    expect(injectSaveButtons(document, hnAdapter)).toBe(3);
    await flush();

    expect(sent(sendMessage, "lookup_urls")).toEqual([
      { type: "lookup_urls", urls: [itemUrl("1"), itemUrl("2"), itemUrl("3")] },
    ]);
    expectPlainSaved(buttonFor(2));
    for (const id of [1, 3]) {
      expect(buttonFor(id).dataset.state).toBe("idle");
      expect(buttonFor(id).querySelector('[data-part="outline"]')).not.toBeNull();
    }
  });

  it("does not ask again for checked posts, and paints re-rendered posts from the cache", async () => {
    sendMessage.mockResolvedValue(lookupAnswer([itemUrl("2")!]));
    injectSaveButtons(document, hnAdapter);
    await flush();

    // A later pass over the same buttons.
    injectSaveButtons(document, hnAdapter);
    await flush();
    expect(sent(sendMessage, "lookup_urls")).toHaveLength(1);

    // The site re-renders the thread: fresh nodes, same URLs.
    document.body.innerHTML = threadHtml([1, 2, 3]);
    expect(injectSaveButtons(document, hnAdapter)).toBe(3);
    // Painted straight away, without waiting for a reply.
    expectPlainSaved(buttonFor(2));
    await flush();
    expect(sent(sendMessage, "lookup_urls")).toHaveLength(1);
    expect(buttonFor(1).dataset.state).toBe("idle");

    // Only a new post is asked about.
    document.body.innerHTML = threadHtml([1, 2, 3, 4]);
    injectSaveButtons(document, hnAdapter);
    await flush();
    expect(sent(sendMessage, "lookup_urls").slice(1)).toEqual([{ type: "lookup_urls", urls: [itemUrl("4")] }]);
  });

  it.each([
    ["a failure response", () => Promise.resolve({ success: false, error: "Native host has exited" })],
    ["no response", () => Promise.resolve(undefined)],
    ["a different result", () => Promise.resolve({ success: true, data: { type: "save_result" } })],
    ["a non-array saved list", () => Promise.resolve(lookupAnswer("all" as unknown as string[]))],
    ["non-string saved entries", () => Promise.resolve(lookupAnswer([1] as unknown as string[]))],
    ["a rejected message", () => Promise.reject(new Error("Extension context invalidated."))],
    [
      "a synchronous throw",
      () => {
        throw new Error("Extension context invalidated.");
      },
    ],
  ])("leaves buttons idle after %s", async (_name, reply) => {
    sendMessage.mockImplementation(reply);

    expect(() => injectSaveButtons(document, hnAdapter)).not.toThrow();
    await flush();

    for (const id of [1, 2, 3]) expect(buttonFor(id).dataset.state).toBe("idle");
  });

  it("does not repaint a button clicked while the lookup was in flight", async () => {
    let answer: (value: unknown) => void = () => {};
    sendMessage.mockImplementation((message: { type: string }) =>
      message.type === "lookup_urls"
        ? new Promise((resolve) => (answer = resolve))
        : new Promise(() => {}),
    );
    injectSaveButtons(document, hnAdapter);

    buttonFor(2).click();
    expect(buttonFor(2).dataset.state).toBe("saving");
    answer(lookupAnswer([itemUrl("1")!, itemUrl("2")!]));
    await flush();

    expect(buttonFor(2).dataset.state).toBe("saving");
    expectPlainSaved(buttonFor(1));
  });

  it("sends more than 200 URLs in batches of at most 200", async () => {
    const ids = Array.from({ length: 450 }, (_, i) => i + 1);
    document.body.innerHTML = threadHtml(ids);
    sendMessage.mockResolvedValue(lookupAnswer([itemUrl("450")!]));

    injectSaveButtons(document, hnAdapter);
    await flush();

    const lookups = sent(sendMessage, "lookup_urls").map((message) => message.urls as string[]);
    expect(lookups.map((urls) => urls.length)).toEqual([200, 200, 50]);
    expect(lookups.flat()).toEqual(ids.map((id) => itemUrl(String(id))));
    expectPlainSaved(buttonFor(450));
  });

  it("remembers a save made through the popup", async () => {
    sendMessage.mockImplementation((message: { type: string }) =>
      Promise.resolve(
        message.type === "lookup_urls" ? lookupAnswer([]) : { success: true, popup_opened: true },
      ),
    );
    injectSaveButtons(document, hnAdapter);
    await flush();

    buttonFor(1).click();
    await flush();
    handleSaveResult({ type: "agentmark:save-result", url: itemUrl("1"), success: true });
    expect(buttonFor(1).dataset.state).toBe("saved");

    document.body.innerHTML = threadHtml([1, 2, 3]);
    injectSaveButtons(document, hnAdapter);
    expectPlainSaved(buttonFor(1));
    await flush();
    expect(sent(sendMessage, "lookup_urls")).toHaveLength(1);
  });

  it("remembers a save made directly by the service worker", async () => {
    sendMessage.mockImplementation((message: { type: string }) =>
      Promise.resolve(
        message.type === "lookup_urls" ? lookupAnswer([]) : { success: true, data: { type: "save_result" } },
      ),
    );
    injectSaveButtons(document, hnAdapter);
    await flush();

    buttonFor(3).click();
    await flush();
    // A fresh save shows its tick.
    expect(buttonFor(3).querySelector('[data-part="tick"]')).not.toBeNull();

    document.body.innerHTML = threadHtml([1, 2, 3]);
    injectSaveButtons(document, hnAdapter);
    expectPlainSaved(buttonFor(3));
  });

  it("saves an already-saved post again when clicked", async () => {
    sendMessage.mockImplementation((message: { type: string }) =>
      Promise.resolve(
        message.type === "lookup_urls" ? lookupAnswer([itemUrl("2")!]) : { success: true, popup_opened: true },
      ),
    );
    injectSaveButtons(document, hnAdapter);
    await flush();
    expectPlainSaved(buttonFor(2));

    buttonFor(2).click();
    expect(buttonFor(2).dataset.state).toBe("saving");
    await flush();

    const [message] = sent(sendMessage, "open_save_popup");
    expect(message.url).toBe(itemUrl("2"));
    expect(message.title).toBe("Hacker News comment by u2");
  });
});
