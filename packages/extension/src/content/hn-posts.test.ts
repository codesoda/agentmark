/**
 * @vitest-environment jsdom
 */
import { describe, it, expect, beforeEach } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";
import { buildSnapshot, injectSaveButtons, resetSavedCache } from "./save-button";
import { commentAuthor, hnAdapter, itemUrl } from "./hn-posts";

const SAVE_BUTTON = "[data-agentmark-save]";

/** Markup as served by news.ycombinator.com/item. */
function threadHtml(): string {
  return `<table class="comment-tree"><tbody>
    <tr class="athing comtr" id="15"><td><table><tbody><tr>
      <td class="ind" indent="0"></td>
      <td class="default"><div><span class="comhead"><a href="user?id=sama">sama</a>
        <span class="age" title="2006-10-09T19:51:01"><a href="item?id=15">on Oct 9, 2006</a></span>
        <span id="unv_15"></span><span class="navs"><a class="togg clicky" id="15">[–]</a></span></span></div>
        <div class="comment"><div class="commtext c00">"the rising star of venture capital" -unknown VC</div></div></td>
    </tr></tbody></table></td></tr>
    <tr class="athing comtr" id="17"><td><table><tbody><tr>
      <td class="ind" indent="1"></td>
      <td class="default"><div><span class="comhead"><a href="user?id=pg">pg</a>
        <span class="navs"><a class="togg clicky" id="17">[–]</a></span></span></div>
        <div class="comment"><div class="commtext c00">Is there anywhere to eat on Sandhill Road?</div></div></td>
    </tr></tbody></table></td></tr>
    <tr class="athing comtr" id="99"><td><div class="comment">[deleted]</div></td></tr>
  </tbody></table>`;
}

describe("Hacker News comment save button", () => {
  beforeEach(() => {
    document.body.innerHTML = threadHtml();
    resetChromeMock();
    resetSavedCache();
  });

  it("builds item permalinks from numeric IDs only", () => {
    expect(itemUrl("15")).toBe("https://news.ycombinator.com/item?id=15");
    expect(itemUrl("")).toBeUndefined();
    expect(itemUrl("15&x=1")).toBeUndefined();
  });

  it("reads the comment author", () => {
    expect(commentAuthor(document.getElementById("15")!)).toBe("sama");
    expect(hnAdapter.title(document.getElementById("17")!)).toBe("Hacker News comment by pg");
    expect(hnAdapter.title(document.getElementById("99")!)).toBe("Hacker News comment");
  });

  it("adds a small button to each comment header, before the fold link", () => {
    expect(injectSaveButtons(document, hnAdapter)).toBe(2);
    expect(injectSaveButtons(document, hnAdapter)).toBe(0);

    const button = document.querySelector<HTMLButtonElement>(`[id="15"] ${SAVE_BUTTON}`)!;
    expect(button.parentElement?.className).toBe("comhead");
    expect(button.nextElementSibling?.className).toBe("navs");
    expect(button.style.width).toBe("18px");
    expect(button.querySelector("svg")?.getAttribute("width")).toBe("18");
    expect(document.querySelector(`[id="99"] ${SAVE_BUTTON}`)).toBeNull();
  });

  it("snapshots the comment as a parseable table, without replies or the button", () => {
    injectSaveButtons(document, hnAdapter);
    const html = buildSnapshot(document.getElementById("15")!, "Hacker News comment by sama", hnAdapter);

    expect(html).toContain("<body><table><tbody><tr");
    expect(html).toContain("the rising star of venture capital");
    expect(html).not.toContain("Sandhill Road");
    expect(html).not.toContain("data-agentmark");

    // It must survive a real HTML parse with the text still inside table cells.
    const parsed = new DOMParser().parseFromString(html!, "text/html");
    expect(parsed.querySelector("tr.athing.comtr .commtext")?.textContent).toContain("rising star");
  });

  it("saves the comment under its own permalink", async () => {
    const sendMessage = resetChromeMock().runtime.sendMessage;
    sendMessage.mockResolvedValue({ success: true, popup_opened: true });
    injectSaveButtons(document, hnAdapter);

    document.querySelector<HTMLButtonElement>(`[id="17"] ${SAVE_BUTTON}`)!.click();
    await new Promise((resolve) => setTimeout(resolve, 0));

    const message = sendMessage.mock.calls
      .map((call) => call[0])
      .find((sent) => sent.type === "open_save_popup");
    expect(message.type).toBe("open_save_popup");
    expect(message.url).toBe("https://news.ycombinator.com/item?id=17");
    expect(message.title).toBe("Hacker News comment by pg");
    expect(message.page_html).toContain("Sandhill Road");
  });
});
