// @vitest-environment jsdom
/**
 * The side panel's app states (gate), the Bookmarks / Settings switch, and
 * the Settings view (screen D) including the optional `tabs` permission.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act, fireEvent, cleanup, within } from "@testing-library/react";
import { resetChromeMock } from "../test/chrome-mock";
import {
  hostNotFound,
  notSetUpState,
  saveSettingsResult,
  settingsInvalid,
  setupState,
  setupStateResult,
  statusNotSetUp,
  statusReady,
  OLD_CLI_NOT_SET_UP_LIST,
} from "../test/fixtures";
import { SIDEPANEL_VIEW_KEY } from "../shared/constants";
import { buildChanges, formFromSettings } from "./settings-form";

vi.mock("../shared/runtime", () => ({
  sendListBookmarksMessage: vi.fn(),
  sendListCollectionsMessage: vi.fn(),
  sendShowBookmarkMessage: vi.fn(),
  sendUpdateBookmarkMessage: vi.fn(),
  sendStatus: vi.fn(),
  sendGetSetupState: vi.fn(),
  sendSaveSettings: vi.fn(),
  sendChooseFolder: vi.fn(),
}));

import SidePanel from "./SidePanel";
import * as runtime from "../shared/runtime";

const m = {
  list: vi.mocked(runtime.sendListBookmarksMessage),
  status: vi.mocked(runtime.sendStatus),
  getState: vi.mocked(runtime.sendGetSetupState),
  saveSettings: vi.mocked(runtime.sendSaveSettings),
};

let chromeMock: ReturnType<typeof resetChromeMock>;

async function openPanel() {
  await act(async () => {
    render(<SidePanel />);
  });
}

async function openSettings() {
  await openPanel();
  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
  });
}

beforeEach(() => {
  chromeMock = resetChromeMock();
  vi.clearAllMocks();
  m.status.mockResolvedValue(statusReady);
  m.list.mockResolvedValue({ bookmarks: [] });
  m.getState.mockResolvedValue(setupStateResult(setupState()));
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("side panel app states", () => {
  it("shows the same install screen as the popup when the app is missing, without the view switch", async () => {
    m.status.mockResolvedValue(hostNotFound);
    await openPanel();
    expect(screen.getByTestId("install-prompt").getAttribute("data-problem")).toBe("not_installed");
    expect(screen.getByText("Finish setting up AgentMark")).toBeDefined();
    expect(screen.queryByTestId("panel-switch")).toBeNull();
    expect(m.list).not.toHaveBeenCalled();
  });

  it("polls while visible, pauses while hidden, and loads bookmarks when the app appears", async () => {
    vi.useFakeTimers();
    m.status.mockResolvedValue(hostNotFound);
    await openPanel();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(m.status).toHaveBeenCalledTimes(2);

    Object.defineProperty(document, "visibilityState", { value: "hidden", configurable: true });
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000);
    });
    expect(m.status).toHaveBeenCalledTimes(2);

    m.status.mockResolvedValue(statusReady);
    Object.defineProperty(document, "visibilityState", { value: "visible", configurable: true });
    await act(async () => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(m.status).toHaveBeenCalledTimes(3);
    expect(m.list).toHaveBeenCalled();
    expect(screen.getByTestId("panel-switch")).toBeDefined();
  });

  it("sets up in the panel, then shows the bookmarks", async () => {
    m.status.mockResolvedValue(statusNotSetUp);
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    m.saveSettings.mockResolvedValue(saveSettingsResult(setupState()));
    await openPanel();

    expect(screen.getByTestId("setup-form")).toBeDefined();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Finish setup" }));
    });
    expect(m.saveSettings).toHaveBeenCalledWith({
      storage_path: "~/Documents/AgentMark",
      default_agent: "claude",
      enrichment_enabled: true,
    });
    expect(m.list).toHaveBeenCalled();
    expect(screen.getByTestId("panel-switch")).toBeDefined();
  });

  it("moves to setup when the list says the app is not set up", async () => {
    m.list.mockResolvedValue({ bookmarks: [], error: OLD_CLI_NOT_SET_UP_LIST });
    m.getState.mockResolvedValue(setupStateResult(notSetUpState()));
    await openPanel();
    expect(screen.getByTestId("setup-form")).toBeDefined();
  });

  it("asks to update an app that does not know get_setup_state", async () => {
    m.getState.mockResolvedValue({ success: false, error: "unknown message type: get_setup_state" });
    await openSettings();
    expect(screen.getByTestId("update-required")).toBeDefined();
  });
});

describe("Bookmarks / Settings switch", () => {
  it("starts on Bookmarks and switches without a router", async () => {
    await openPanel();
    const bookmarks = screen.getByRole("button", { name: "Bookmarks" });
    const settings = screen.getByRole("button", { name: "Settings" });
    expect(bookmarks.getAttribute("aria-pressed")).toBe("true");
    expect(settings.getAttribute("aria-pressed")).toBe("false");

    await act(async () => {
      fireEvent.click(settings);
    });
    expect(settings.getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByTestId("settings-view")).toBeDefined();

    await act(async () => {
      fireEvent.click(bookmarks);
    });
    expect(screen.queryByTestId("settings-view")).toBeNull();
  });

  it("opens on Settings when the toolbar menu asked for it, then clears the request", async () => {
    chromeMock.storage.session.data[SIDEPANEL_VIEW_KEY] = "settings";
    await openPanel();
    expect(screen.getByTestId("settings-view")).toBeDefined();
    expect(chromeMock.storage.session.data[SIDEPANEL_VIEW_KEY]).toBeUndefined();
  });

  it("switches to Settings when asked while already open", async () => {
    await openPanel();
    await act(async () => {
      await chromeMock.storage.session.set({ [SIDEPANEL_VIEW_KEY]: "settings" });
    });
    expect(screen.getByTestId("settings-view")).toBeDefined();
  });
});

describe("Settings view", () => {
  it("shows the app, storage, summaries, capture sites and toolbar sections", async () => {
    await openSettings();
    const view = screen.getByTestId("settings-view");
    expect(screen.getByTestId("app-version").textContent).toBe("agentmark 0.3.0");
    expect(screen.getByTestId("bookmark-count").textContent).toBe("42");
    expect((screen.getByTestId("folder-input") as HTMLInputElement).value).toBe("~/Documents/AgentMark");
    // Review finding #3: the note tells the truth about bookmarks already saved.
    expect(screen.getByTestId("folder-move-note").textContent).toBe(
      "Bookmarks you already saved stay in the old folder. AgentMark cannot open or edit them until you move them into the new folder.",
    );
    expect(screen.getByTestId("folder-move-note").getAttribute("data-tone")).toBe("hint");
    expect(view.textContent).toContain("With this off, those pages keep the text taken straight from the page.");
    expect(screen.getByTestId("summaries-switch").getAttribute("aria-checked")).toBe("true");
    // The switch's copy does not pretend it only affects summaries.
    expect(view.textContent).toContain("Your agent also turns LinkedIn, X, Hacker News and Reddit pages into readable text.");
    expect(screen.getAllByTestId("builtin-site").map((el) => el.textContent)).toEqual(
      setupState().builtin_snapshot_sites,
    );
    expect(screen.getAllByTestId("user-site").map((el) => el.firstChild!.textContent)).toEqual(["example.com/app/"]);
    expect(screen.getByText("Show when the current page is already saved")).toBeDefined();
    expect(screen.getByText("On every site. Chrome will ask for permission.")).toBeDefined();
    expect((screen.getByTestId("save-settings") as HTMLButtonElement).disabled).toBe(true);
  });

  it("warns about bookmarks already saved once the folder differs from the saved one", async () => {
    await openSettings();
    const input = screen.getByTestId("folder-input");
    fireEvent.change(input, { target: { value: "~/Elsewhere" } });
    const note = screen.getByTestId("folder-move-note");
    expect(note.getAttribute("data-tone")).toBe("warning");
    expect(note.getAttribute("role")).toBe("status");
    expect(note.textContent).toContain("stay in the old folder");
    // Back to the saved folder: a hint again.
    fireEvent.change(input, { target: { value: "~/Documents/AgentMark" } });
    expect(screen.getByTestId("folder-move-note").getAttribute("data-tone")).toBe("hint");
  });

  it("sends only the fields that changed, then shows Saved", async () => {
    m.saveSettings.mockResolvedValue(
      saveSettingsResult(setupState({ settings: { system_prompt: "Short.", default_agent: "codex" } })),
    );
    await openSettings();

    fireEvent.click(within(screen.getByTestId("agent-codex")).getByRole("radio"));
    fireEvent.change(screen.getByTestId("prompt-input"), { target: { value: "Short." } });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    });

    expect(m.saveSettings).toHaveBeenCalledWith({ default_agent: "codex", system_prompt: "Short." });
    expect(screen.getByTestId("save-status").textContent).toBe("Saved");
    expect(screen.getByTestId("save-status").getAttribute("aria-live")).toBe("polite");
    expect((screen.getByTestId("save-settings") as HTMLButtonElement).disabled).toBe(true);
  });

  it("replaces the whole site list when sites are added or removed", async () => {
    m.saveSettings.mockResolvedValue(saveSettingsResult(setupState({ settings: { snapshot_sites: ["docs.example.org"] } })));
    await openSettings();

    fireEvent.click(screen.getByRole("button", { name: "Remove example.com/app/" }));
    fireEvent.change(screen.getByTestId("new-site-input"), { target: { value: " docs.example.org " } });
    fireEvent.keyDown(screen.getByTestId("new-site-input"), { key: "Enter" });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    });

    expect(m.saveSettings).toHaveBeenCalledWith({ snapshot_sites: ["docs.example.org"] });
  });

  it("shows field errors next to their fields, including a site by index", async () => {
    m.saveSettings.mockResolvedValue(settingsInvalid);
    await openSettings();
    for (const site of ["b.example", "exa mple.com"]) {
      fireEvent.change(screen.getByTestId("new-site-input"), { target: { value: site } });
      fireEvent.click(screen.getByRole("button", { name: "Add site" }));
    }
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    });

    expect(screen.getByTestId("folder-input").getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByText("AgentMark cannot write to /Volumes/X.")).toBeDefined();
    expect(screen.getAllByTestId("user-site")[2].className).toContain("border-red-300");
    expect(screen.getByTestId("site-errors").textContent).toContain('"exa mple.com" is not a host/path rule.');
    expect(screen.getByTestId("save-status").textContent).toContain("Some settings need fixing");
  });

  it("shows a failed save", async () => {
    m.saveSettings.mockResolvedValue({ success: false, error: "failed to write /Users/me/.agentmark/config.toml: denied" });
    await openSettings();
    fireEvent.click(screen.getByTestId("summaries-switch"));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    });
    expect(m.saveSettings).toHaveBeenCalledWith({ enrichment_enabled: false });
    expect(screen.getByTestId("save-status").textContent).toContain("failed to write");
  });

  it("shows a load failure with Try again", async () => {
    m.getState.mockResolvedValueOnce({ success: false, error: "HOME is not set" });
    await openSettings();
    expect(screen.getByTestId("settings-error").textContent).toContain("HOME is not set");
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    });
    expect(screen.getByTestId("settings-view")).toBeDefined();
  });
});

describe("partial update (contract §5.2)", () => {
  const saved = setupState().settings;

  it("is empty when nothing changed", () => {
    expect(buildChanges(saved, formFromSettings(saved))).toEqual({});
  });

  it("treats the absolute and the ~ form of the same folder as unchanged", () => {
    expect(buildChanges(saved, { ...formFromSettings(saved), storage_path: saved.storage_path })).toEqual({});
  });

  it("clears the instructions with null", () => {
    const withPrompt = { ...saved, system_prompt: "Be brief." };
    expect(buildChanges(withPrompt, { ...formFromSettings(withPrompt), system_prompt: "  " })).toEqual({
      system_prompt: null,
    });
  });

  it("sends the folder trimmed", () => {
    expect(buildChanges(saved, { ...formFromSettings(saved), storage_path: " ~/Elsewhere " })).toEqual({
      storage_path: "~/Elsewhere",
    });
  });
});

describe("toolbar icon permission switch", () => {
  async function permissionSwitch() {
    await openSettings();
    return screen.getByTestId("tabs-permission-switch");
  }

  it("is off without the permission", async () => {
    const toggle = await permissionSwitch();
    expect(toggle.getAttribute("aria-checked")).toBe("false");
    expect(chromeMock.permissions.contains).toHaveBeenCalledWith({ permissions: ["tabs"] });
  });

  it("requests tabs from the click and turns on when granted", async () => {
    chromeMock.permissions.request.mockResolvedValue(true);
    const toggle = await permissionSwitch();
    await act(async () => {
      fireEvent.click(toggle);
    });
    expect(chromeMock.permissions.request).toHaveBeenCalledWith({ permissions: ["tabs"] });
    expect(toggle.getAttribute("aria-checked")).toBe("true");
    // Not part of "Save changes".
    expect(m.saveSettings).not.toHaveBeenCalled();
  });

  it("stays off and explains when Chrome does not grant it", async () => {
    chromeMock.permissions.request.mockResolvedValue(false);
    const toggle = await permissionSwitch();
    await act(async () => {
      fireEvent.click(toggle);
    });
    expect(toggle.getAttribute("aria-checked")).toBe("false");
    expect(screen.getByTestId("tabs-permission-note").textContent).toContain("did not grant");
  });

  it("removes the permission when turned off", async () => {
    chromeMock.permissions.contains.mockResolvedValue(true);
    chromeMock.permissions.remove.mockResolvedValue(true);
    const toggle = await permissionSwitch();
    expect(toggle.getAttribute("aria-checked")).toBe("true");
    await act(async () => {
      fireEvent.click(toggle);
    });
    expect(chromeMock.permissions.remove).toHaveBeenCalledWith({ permissions: ["tabs"] });
    expect(toggle.getAttribute("aria-checked")).toBe("false");
  });

  it("follows a revoke made elsewhere (chrome://extensions)", async () => {
    chromeMock.permissions.contains.mockResolvedValue(true);
    const toggle = await permissionSwitch();
    expect(toggle.getAttribute("aria-checked")).toBe("true");

    chromeMock.permissions.contains.mockResolvedValue(false);
    const calls = chromeMock.permissions.onRemoved.addListener.mock.calls;
    const listener = calls[calls.length - 1][0] as (
      p: chrome.permissions.Permissions,
    ) => void;
    await act(async () => {
      listener({ permissions: ["tabs"] });
    });
    expect(toggle.getAttribute("aria-checked")).toBe("false");
  });
});
