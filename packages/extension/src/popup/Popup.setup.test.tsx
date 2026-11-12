/**
 * @vitest-environment jsdom
 *
 * The popup's app states end to end: polling while the app is missing, setup
 * (screen C) with the draft that survives the popup closing, and the
 * compatibility rules for older CLIs.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, cleanup, fireEvent, within } from "@testing-library/react";
import { resetChromeMock } from "../test/chrome-mock";
import {
  folderCancelled,
  folderChosen,
  folderUnavailable,
  hostNotFound,
  notSetUpState,
  saveSettingsResult,
  settingsInvalid,
  setupRequired,
  setupState,
  setupStateResult,
  statusNotSetUp,
  statusOldCli,
  statusReady,
  OLD_CLI_NOT_SET_UP_SAVE,
} from "../test/fixtures";
import { SETUP_DRAFT_KEY } from "../shared/constants";
import type { RuntimeResponse } from "../shared/types";

vi.mock("../shared/runtime", () => ({
  queryActiveTab: vi.fn(),
  sendSaveMessage: vi.fn(),
  sendListCollectionsMessage: vi.fn(),
  loadLastUsedIntent: vi.fn(),
  saveLastUsedIntent: vi.fn(),
  querySelectedText: vi.fn(),
  queryPageHtml: vi.fn(),
  takePendingSave: vi.fn(),
  sendStatus: vi.fn(),
  sendGetSetupState: vi.fn(),
  sendSaveSettings: vi.fn(),
  sendChooseFolder: vi.fn(),
}));

import Popup from "./Popup";
import * as runtime from "../shared/runtime";

const m = {
  queryActiveTab: vi.mocked(runtime.queryActiveTab),
  save: vi.mocked(runtime.sendSaveMessage),
  pageHtml: vi.mocked(runtime.queryPageHtml),
  takePending: vi.mocked(runtime.takePendingSave),
  status: vi.mocked(runtime.sendStatus),
  getState: vi.mocked(runtime.sendGetSetupState),
  saveSettings: vi.mocked(runtime.sendSaveSettings),
  chooseFolder: vi.mocked(runtime.sendChooseFolder),
};

const SAVED: RuntimeResponse = {
  success: true,
  data: { type: "save_result", id: "bm_1", path: "/tmp/bm_1", status: "created" },
};

let chromeMock: ReturnType<typeof resetChromeMock>;

async function openPopup() {
  let result!: ReturnType<typeof render>;
  await act(async () => {
    result = render(<Popup />);
  });
  return result;
}

async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

beforeEach(() => {
  chromeMock = resetChromeMock();
  vi.clearAllMocks();
  vi.useFakeTimers({ shouldAdvanceTime: false });
  vi.spyOn(window, "close").mockImplementation(() => {});
  (chrome.runtime as { id?: string }).id = "abcdefghijklmnopabcdefghijklmnop";
  m.takePending.mockResolvedValue(undefined);
  m.queryActiveTab.mockResolvedValue({ url: "https://example.com/a", title: "Example A", id: 9 });
  m.pageHtml.mockResolvedValue(undefined);
  m.save.mockResolvedValue(SAVED);
  m.status.mockResolvedValue(statusReady);
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("polling while the app is missing", () => {
  it("re-checks every 2 seconds after each probe settles, then saves once the app is set up", async () => {
    m.status.mockResolvedValue(hostNotFound);
    await openPopup();
    expect(m.status).toHaveBeenCalledTimes(1);

    await advance(1999);
    expect(m.status).toHaveBeenCalledTimes(1);
    await advance(1);
    expect(m.status).toHaveBeenCalledTimes(2);
    await advance(2000);
    expect(m.status).toHaveBeenCalledTimes(3);
    expect(m.save).not.toHaveBeenCalled();

    m.status.mockResolvedValue(statusReady);
    await advance(2000);
    expect(m.save).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("success-state")).toBeDefined();

    // Ready: no more polling.
    const calls = m.status.mock.calls.length;
    await advance(10_000);
    expect(m.status).toHaveBeenCalledTimes(calls);
  });

  it("never overlaps probes: the next waits for the previous to settle", async () => {
    let answer!: (r: RuntimeResponse) => void;
    m.status.mockResolvedValueOnce(hostNotFound).mockImplementationOnce(
      () => new Promise<RuntimeResponse>((r) => { answer = r; }),
    );
    await openPopup();
    await advance(2000);
    expect(m.status).toHaveBeenCalledTimes(2);
    await advance(10_000);
    expect(m.status).toHaveBeenCalledTimes(2);
    m.status.mockResolvedValue(hostNotFound);
    await act(async () => answer(hostNotFound));
    await advance(2000);
    expect(m.status).toHaveBeenCalledTimes(3);
  });

  it("stops polling when the popup closes", async () => {
    m.status.mockResolvedValue(hostNotFound);
    const { unmount } = await openPopup();
    unmount();
    await advance(10_000);
    expect(m.status).toHaveBeenCalledTimes(1);
  });

  it("Check again probes at once and restarts the timer", async () => {
    m.status.mockResolvedValue(hostNotFound);
    await openPopup();
    await advance(1500);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Check again" }));
    });
    expect(m.status).toHaveBeenCalledTimes(2);
    await advance(1999);
    expect(m.status).toHaveBeenCalledTimes(2);
    await advance(1);
    expect(m.status).toHaveBeenCalledTimes(3);
  });

  it("also polls while registered for another extension", async () => {
    m.status.mockResolvedValue({ success: false, error: "Access to the specified native messaging host is forbidden." });
    await openPopup();
    await advance(2000);
    expect(m.status).toHaveBeenCalledTimes(2);
  });

  it("does not poll in other problem states", async () => {
    m.status.mockResolvedValue({ success: false, error: "Could not establish connection. Receiving end does not exist." });
    await openPopup();
    expect(screen.getByTestId("install-prompt").getAttribute("data-problem")).toBe("worker_unreachable");
    await advance(10_000);
    expect(m.status).toHaveBeenCalledTimes(1);
  });

  it("moves to setup when the app appears but is not set up", async () => {
    m.status.mockResolvedValue(hostNotFound);
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    await openPopup();
    m.status.mockResolvedValue(statusNotSetUp);
    await advance(2000);
    expect(screen.getByTestId("setup-form")).toBeDefined();
    expect(m.save).not.toHaveBeenCalled();
  });
});

describe("setup (screen C)", () => {
  beforeEach(() => {
    m.status.mockResolvedValue(statusNotSetUp);
  });

  it("shows the CLI's defaults: folder, first found agent preselected with badges, summaries on", async () => {
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    await openPopup();

    expect(screen.getByRole("heading", { name: "Set up AgentMark" })).toBeDefined();
    expect(screen.getByText("The app is installed (0.3.0). Two choices and you're done.")).toBeDefined();
    expect((screen.getByTestId("folder-input") as HTMLInputElement).value).toBe("~/Documents/AgentMark");
    const claude = screen.getByTestId("agent-claude");
    expect((within(claude).getByRole("radio") as HTMLInputElement).checked).toBe(true);
    expect(claude.textContent).toContain("Claude Code");
    expect(claude.textContent).toContain("Found");
    expect(screen.getByTestId("agent-codex").textContent).toContain("Not installed");
    expect(screen.getByTestId("summaries-switch").getAttribute("aria-checked")).toBe("true");
    expect(screen.getByText(/Sends the page text to your agent's provider/)).toBeDefined();
    // Review finding #15: what turning it off does for those sites.
    expect(screen.getByText(/With this off, those pages keep the text taken straight from the page\./)).toBeDefined();
    expect(screen.getByRole("button", { name: "Finish and save this page" })).toBeDefined();
    expect(screen.getByText("You can change all of this later in Settings.")).toBeDefined();
  });

  it("saves the three answers, then saves the page", async () => {
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    m.saveSettings.mockResolvedValue(saveSettingsResult(setupState()));
    await openPopup();

    fireEvent.change(screen.getByTestId("folder-input"), { target: { value: "~/Bookmarks" } });
    fireEvent.click(within(screen.getByTestId("agent-codex")).getByRole("radio"));
    fireEvent.click(screen.getByTestId("summaries-switch"));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Finish and save this page" }));
    });

    // Absent keys stay unchanged: instructions and sites are not sent.
    expect(m.saveSettings).toHaveBeenCalledWith({
      storage_path: "~/Bookmarks",
      default_agent: "codex",
      enrichment_enabled: false,
    });
    expect(m.save).toHaveBeenCalledWith("https://example.com/a", "Example A", { page_html: undefined });
    expect(screen.getByTestId("success-state")).toBeDefined();
    expect(chromeMock.storage.session.data[SETUP_DRAFT_KEY]).toBeUndefined();
  });

  it("saves the in-page post that opened the popup once setup finishes", async () => {
    const post = { url: "https://x.com/ada/status/1", title: "Post by Ada", page_html: "<p>post</p>" };
    m.takePending.mockResolvedValueOnce(post);
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    m.saveSettings.mockResolvedValue(saveSettingsResult(setupState()));
    await openPopup();
    // The pending save was taken at once, before it could expire.
    expect(m.takePending).toHaveBeenCalledTimes(1);

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Finish and save this post" }));
    });
    expect(m.save).toHaveBeenCalledWith(post.url, post.title, { page_html: post.page_html });
    expect(m.queryActiveTab).not.toHaveBeenCalled();
  });

  // Review finding #14: the folder dialog closes the popup mid-setup.
  it("still saves the in-page post after the popup closes and reopens mid-setup", async () => {
    const post = { url: "https://www.reddit.com/r/x/comments/abc/comment/def/", title: "Reddit comment", page_html: "<p>c</p>" };
    m.takePending.mockResolvedValueOnce(post);
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    m.saveSettings.mockResolvedValue(saveSettingsResult(setupState()));
    m.queryActiveTab.mockResolvedValue({ url: "https://www.reddit.com/r/x/", title: "r/x feed", id: 9 });
    const first = await openPopup();
    expect(screen.getByRole("button", { name: "Finish and save this post" })).toBeDefined();
    first.unmount();
    // The item waits in the setup draft (session storage).
    expect((chromeMock.storage.session.data[SETUP_DRAFT_KEY] as { pending_save?: unknown }).pending_save).toEqual(post);

    // Reopened (the pending save in the worker is gone by now): the post, not the feed.
    await openPopup();
    expect(screen.getByRole("button", { name: "Finish and save this post" })).toBeDefined();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Finish and save this post" }));
    });
    expect(m.save).toHaveBeenCalledTimes(1);
    expect(m.save).toHaveBeenCalledWith(post.url, post.title, { page_html: post.page_html });
    expect(m.pageHtml).not.toHaveBeenCalled();
    expect(chromeMock.storage.session.data[SETUP_DRAFT_KEY]).toBeUndefined();
  });

  it("saves the tab, not a drafted post, when the app was set up elsewhere meanwhile", async () => {
    chromeMock.storage.session.data[SETUP_DRAFT_KEY] = {
      storage_path: null,
      default_agent: null,
      enrichment_enabled: null,
      pending_save: { url: "https://x.com/ada/status/1", title: "Post by Ada" },
      updated_at: Date.now(),
    };
    m.status.mockResolvedValue(statusReady);
    await openPopup();
    expect(m.save).toHaveBeenCalledTimes(1);
    expect(m.save).toHaveBeenCalledWith("https://example.com/a", "Example A", { page_html: undefined });
    await act(async () => {});
    expect((chromeMock.storage.session.data[SETUP_DRAFT_KEY] as { pending_save?: unknown }).pending_save).toBeNull();
  });

  it("forgets a drafted post older than ten minutes", async () => {
    chromeMock.storage.session.data[SETUP_DRAFT_KEY] = {
      storage_path: null,
      default_agent: null,
      enrichment_enabled: null,
      pending_save: { url: "https://x.com/ada/status/1", title: "Post by Ada" },
      updated_at: Date.now() - 600_001,
    };
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    await openPopup();
    expect(screen.getByRole("button", { name: "Finish and save this page" })).toBeDefined();
  });

  it("says Finish setup when there is no page to save", async () => {
    m.queryActiveTab.mockRejectedValue(new Error("Unsupported page: chrome://newtab/"));
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    await openPopup();
    expect(screen.getByRole("button", { name: "Finish setup" })).toBeDefined();
  });

  it("shows field errors from the CLI next to their fields and saves nothing", async () => {
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    m.saveSettings.mockResolvedValue(settingsInvalid);
    await openPopup();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Finish and save this page" }));
    });

    const input = screen.getByTestId("folder-input");
    expect(input.getAttribute("aria-invalid")).toBe("true");
    const describedBy = input.getAttribute("aria-describedby")!;
    expect(document.getElementById(describedBy.split(" ")[0])!.textContent).toBe("AgentMark cannot write to /Volumes/X.");
    // An error for a field this screen does not show still reaches the user.
    expect(screen.getByTestId("setup-error").textContent).toContain('"exa mple.com" is not a host/path rule.');
    expect(m.save).not.toHaveBeenCalled();
    expect(screen.getByTestId("setup-form")).toBeDefined();
  });

  it("starts with summaries off and says why when no agent is found", async () => {
    const state = notSetUpState({
      agents: [
        { id: "claude", name: "Claude Code", found: false, path: null },
        { id: "codex", name: "Codex", found: false, path: null },
        { id: "pi", name: "pi", found: false, path: null },
      ],
      settings: { enrichment_enabled: false, default_agent: "claude" },
    });
    m.getState.mockResolvedValue(setupStateResult(state));
    await openPopup();

    expect(screen.getByTestId("summaries-switch").getAttribute("aria-checked")).toBe("false");
    expect(screen.getByTestId("no-agent").textContent).toBe(
      "No agent was found on this computer, so summaries start off. Install Claude Code, Codex or pi to turn them on later.",
    );
    expect(screen.getAllByText("Not installed")).toHaveLength(3);
    // No agent that is not installed shows as the active pick.
    expect(screen.getAllByRole("radio").filter((radio) => (radio as HTMLInputElement).checked)).toHaveLength(0);

    // The stored choice is still sent, as the contract requires.
    m.saveSettings.mockResolvedValue(saveSettingsResult(setupState()));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Finish and save this page" }));
    });
    expect(m.saveSettings).toHaveBeenCalledWith({
      storage_path: "~/Documents/AgentMark",
      default_agent: "claude",
      enrichment_enabled: false,
    });
  });

  it("shows an agent picked by the user as picked even when none is found", async () => {
    const state = notSetUpState({
      agents: [
        { id: "claude", name: "Claude Code", found: false, path: null },
        { id: "codex", name: "Codex", found: false, path: null },
      ],
      settings: { enrichment_enabled: false, default_agent: "claude" },
    });
    m.getState.mockResolvedValue(setupStateResult(state));
    await openPopup();

    fireEvent.click(within(screen.getByTestId("agent-codex")).getByRole("radio"));

    expect((within(screen.getByTestId("agent-codex")).getByRole("radio") as HTMLInputElement).checked).toBe(true);
    expect(screen.getByTestId("no-agent")).toBeDefined();
  });

  it("fills the folder from the dialog", async () => {
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    m.chooseFolder.mockResolvedValue(folderChosen);
    await openPopup();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    });
    expect(m.chooseFolder).toHaveBeenCalledWith("~/Documents/AgentMark");
    expect((screen.getByTestId("folder-input") as HTMLInputElement).value).toBe("~/Bookmarks");
  });

  it("keeps the folder when the dialog is cancelled", async () => {
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    m.chooseFolder.mockResolvedValue(folderCancelled);
    await openPopup();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    });
    expect((screen.getByTestId("folder-input") as HTMLInputElement).value).toBe("~/Documents/AgentMark");
    expect(screen.queryByTestId("folder-note")).toBeNull();
  });

  it("falls back to typing when no dialog is available", async () => {
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    m.chooseFolder.mockResolvedValue(folderUnavailable);
    await openPopup();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
    });
    expect(screen.getByTestId("folder-note").textContent).toBe(
      "No folder dialog is available on this system. Type the path instead.",
    );
    expect(document.activeElement).toBe(screen.getByTestId("folder-input"));
  });

  it("restores the answers after the popup closes and reopens", async () => {
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    const first = await openPopup();
    fireEvent.click(within(screen.getByTestId("agent-pi")).getByRole("radio"));
    fireEvent.click(screen.getByTestId("summaries-switch"));
    await act(async () => {});
    first.unmount();

    // Meanwhile the service worker wrote the folder the user chose in the dialog.
    const draft = chromeMock.storage.session.data[SETUP_DRAFT_KEY] as Record<string, unknown>;
    chromeMock.storage.session.data[SETUP_DRAFT_KEY] = { ...draft, storage_path: "~/Bookmarks", updated_at: Date.now() };

    await openPopup();
    expect((screen.getByTestId("folder-input") as HTMLInputElement).value).toBe("~/Bookmarks");
    expect((within(screen.getByTestId("agent-pi")).getByRole("radio") as HTMLInputElement).checked).toBe(true);
    expect(screen.getByTestId("summaries-switch").getAttribute("aria-checked")).toBe("false");
  });

  it("ignores a draft older than ten minutes", async () => {
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    chromeMock.storage.session.data[SETUP_DRAFT_KEY] = {
      storage_path: "~/Old",
      default_agent: null,
      enrichment_enabled: null,
      updated_at: Date.now() - 600_001,
    };
    await openPopup();
    expect((screen.getByTestId("folder-input") as HTMLInputElement).value).toBe("~/Documents/AgentMark");
  });

  it("shows a settings file problem before saving over it", async () => {
    m.getState.mockResolvedValue(
      setupStateResult(notSetUpState({ config_problem: "failed to parse config: expected `=`" })),
    );
    await openPopup();
    expect(screen.getByTestId("config-problem").textContent).toContain("could not be read");
  });

  it("goes straight to saving when setup turns out to be done already", async () => {
    m.getState.mockResolvedValue(setupStateResult(setupState()));
    await openPopup();
    expect(m.save).toHaveBeenCalledTimes(1);
  });

  it("shows a setup-state failure with Try again", async () => {
    m.getState.mockResolvedValueOnce({ success: false, error: "HOME is not set" });
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    await openPopup();
    expect(screen.getByTestId("setup-load-error").textContent).toContain("HOME is not set");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    });
    expect(screen.getByTestId("setup-form")).toBeDefined();
  });
});

describe("older CLIs (contract §2.6)", () => {
  it("treats a status without `initialized` as ready, then sets up on the old CLI's save error", async () => {
    m.status.mockResolvedValue(statusOldCli);
    m.save.mockResolvedValueOnce({ success: false, error: OLD_CLI_NOT_SET_UP_SAVE });
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    await openPopup();
    expect(m.save).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("setup-form")).toBeDefined();
  });

  it("asks to update when the CLI does not know get_setup_state", async () => {
    m.status.mockResolvedValue(statusOldCli);
    m.save.mockResolvedValueOnce({ success: false, error: OLD_CLI_NOT_SET_UP_SAVE });
    m.getState.mockResolvedValue({ success: false, error: "unknown message type: get_setup_state" });
    await openPopup();

    const update = screen.getByTestId("update-required");
    expect(update.textContent).toContain("This AgentMark app (0.2.6) is too old for this extension. Update it:");
    expect(screen.getByTestId("command-text").textContent).toBe(
      "curl -sSL https://raw.githubusercontent.com/codesoda/agentmark/main/install.sh | bash -s -- --extension-id abcdefghijklmnopabcdefghijklmnop",
    );
  });

  it("moves to setup on the setup_required code from a save", async () => {
    m.save.mockResolvedValueOnce(setupRequired);
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    await openPopup();
    expect(screen.getByTestId("setup-form")).toBeDefined();
  });
});
