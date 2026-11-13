// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from "vitest";
import { resetChromeMock } from "../test/chrome-mock";
import { REPORT_PAGE_MESSAGE, reportPageIfChanged, resetPageReport } from "./page-report";
import { REPORT_PAGE_MESSAGE as SHARED_REPORT_PAGE_MESSAGE } from "../shared/constants";
import { watchPosts, type SiteAdapter } from "./save-button";

const at = (href: string) => ({ href }) as Location;

describe("page report (contract §3.3)", () => {
  let chromeMock: ReturnType<typeof resetChromeMock>;

  beforeEach(() => {
    chromeMock = resetChromeMock();
    chromeMock.runtime.sendMessage.mockResolvedValue({ ok: true });
    resetPageReport();
  });

  it("uses the same message type as the service worker", () => {
    expect(REPORT_PAGE_MESSAGE).toBe(SHARED_REPORT_PAGE_MESSAGE);
    expect(REPORT_PAGE_MESSAGE).toBe("report_page");
  });

  it("reports each distinct page once", () => {
    reportPageIfChanged(at("https://x.com/ada/status/1"));
    reportPageIfChanged(at("https://x.com/ada/status/1"));
    reportPageIfChanged(at("https://x.com/home"));

    expect(chromeMock.runtime.sendMessage.mock.calls).toEqual([
      [{ type: "report_page", url: "https://x.com/ada/status/1" }],
      [{ type: "report_page", url: "https://x.com/home" }],
    ]);
  });

  it("never throws when the extension is gone", () => {
    chromeMock.runtime.sendMessage.mockImplementation(() => {
      throw new Error("Extension context invalidated.");
    });
    expect(() => reportPageIfChanged(at("https://news.ycombinator.com/item?id=1"))).not.toThrow();
  });

  it("runs on the first injection pass of watchPosts", () => {
    const adapter: SiteAdapter = {
      postSelector: ".none",
      findSlot: () => null,
      resolveUrl: () => undefined,
      title: () => "",
      snapshotRoot: (post) => post,
    };
    let passes = 0;
    const observer = watchPosts(document, adapter, () => passes++);
    observer.disconnect();
    expect(passes).toBe(1);
  });
});
