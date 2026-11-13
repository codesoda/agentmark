import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CONNECTION, readyState, STALE_CONNECTION } from "../mock/fixtures";
import { FOLDER_MOVE_NOTE, SUMMARIES_NOTE } from "../copy";
import { installHandlers, type Handlers } from "../mock/handlers";
import type { SettingsChanges, SetupState } from "../types";
import Settings from "./Settings";

function setup(overrides: Handlers = {}, connection = CONNECTION) {
  let state: SetupState = readyState();
  const calls = installHandlers({
    get_setup_state: () => state,
    save_settings: ({ changes }) => {
      const c = changes as SettingsChanges;
      state = {
        ...state,
        settings: { ...state.settings, ...c, system_prompt: c.system_prompt === undefined ? state.settings.system_prompt : c.system_prompt } as SetupState["settings"],
      };
      return { type: "saved", state };
    },
    ...overrides,
  });
  const onSaved = vi.fn();
  render(<Settings connection={connection} onSaved={onSaved} />);
  return { calls, onSaved };
}

describe("Settings", () => {
  it("shows the app, storage, summaries and capture sections, without a browser permission switch", async () => {
    setup();
    expect(await screen.findByTestId("app-version")).toHaveTextContent("agentmark 0.2.6");
    expect(screen.getByTestId("bookmark-count")).toHaveTextContent("674");
    expect(screen.getByTestId("extension-status")).toHaveTextContent("Connected · 0.2.6");
    expect(screen.getByTestId("folder-input")).toHaveValue("~/Documents/AgentMark");
    // Review finding #3: the note tells the truth about bookmarks already saved.
    expect(screen.getByTestId("folder-move-note")).toHaveTextContent(FOLDER_MOVE_NOTE);
    expect(screen.getByTestId("folder-move-note")).toHaveAttribute("data-tone", "hint");
    expect(screen.getByText(new RegExp(SUMMARIES_NOTE.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")))).toBeInTheDocument();
    expect(screen.getByText(/those pages keep the text taken straight from the page/)).toBeInTheDocument();
    expect(screen.getAllByTestId("builtin-site")).toHaveLength(8);
    expect(screen.getAllByTestId("user-site").map((el) => el.textContent?.replace("×", ""))).toEqual(["example.com/app/"]);
    expect(screen.queryByText(/Chrome will ask for permission/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Toolbar icon/i)).not.toBeInTheDocument();
    expect(screen.getByTestId("save-settings")).toBeDisabled();
  });

  it("warns about bookmarks already saved once the folder differs from the saved one", async () => {
    setup();
    const input = await screen.findByTestId("folder-input");
    fireEvent.change(input, { target: { value: "~/Elsewhere" } });
    expect(screen.getByTestId("folder-move-note")).toHaveAttribute("data-tone", "warning");
    expect(screen.getByTestId("folder-move-note")).toHaveAttribute("role", "status");
    expect(FOLDER_MOVE_NOTE).toBe(
      "Bookmarks you already saved stay in the old folder. AgentMark cannot open or edit them until you move them into the new folder.",
    );
    fireEvent.change(input, { target: { value: "~/Documents/AgentMark" } });
    expect(screen.getByTestId("folder-move-note")).toHaveAttribute("data-tone", "hint");
  });

  it("shows an old liveness record as last seen, not Connected", async () => {
    setup({}, STALE_CONNECTION);
    const status = await screen.findByTestId("extension-status");
    expect(status).toHaveTextContent(/^Last connected .+ · version 0\.2\.6$/);
    expect(status).not.toHaveTextContent("Connected ·");
    expect(status.className).not.toContain("emerald");
  });

  it("says when the extension is not connected", async () => {
    setup({}, null as never);
    expect(await screen.findByTestId("extension-status")).toHaveTextContent("Not connected yet");
  });

  it("sends only the fields that changed", async () => {
    const { calls, onSaved } = setup();
    fireEvent.click(await screen.findByTestId("summaries-switch"));
    fireEvent.change(screen.getByTestId("prompt-input"), { target: { value: "Keep it short." } });
    fireEvent.change(screen.getByTestId("new-site-input"), { target: { value: "notes.example.com" } });
    fireEvent.click(screen.getByTestId("add-site"));
    expect(screen.getByTestId("save-status")).toHaveTextContent("Unsaved changes");
    fireEvent.click(screen.getByTestId("save-settings"));
    await waitFor(() => expect(screen.getByTestId("save-status")).toHaveTextContent("Saved"));
    expect(calls.save_settings[0].changes).toEqual({
      enrichment_enabled: false,
      system_prompt: "Keep it short.",
      snapshot_sites: ["example.com/app/", "notes.example.com"],
    });
    expect(onSaved).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("save-settings")).toBeDisabled();
  });

  it("clears the instructions with null", async () => {
    let state = readyState();
    state = { ...state, settings: { ...state.settings, system_prompt: "Old" } };
    const { calls } = setup({ get_setup_state: () => state, save_settings: () => ({ type: "saved", state }) });
    fireEvent.change(await screen.findByTestId("prompt-input"), { target: { value: "  " } });
    fireEvent.click(screen.getByTestId("save-settings"));
    await waitFor(() => expect(calls.save_settings).toHaveLength(1));
    expect(calls.save_settings[0].changes).toEqual({ system_prompt: null });
  });

  it("removes a user site", async () => {
    const { calls } = setup();
    fireEvent.click(await screen.findByLabelText("Remove example.com/app/"));
    fireEvent.click(screen.getByTestId("save-settings"));
    await waitFor(() => expect(calls.save_settings).toHaveLength(1));
    expect(calls.save_settings[0].changes).toEqual({ snapshot_sites: [] });
  });

  it("shows field errors from the core", async () => {
    setup({
      save_settings: () => ({
        type: "invalid",
        errors: [
          { field: "storage_path", index: null, code: "not_absolute", message: "Use a full path, like ~/Documents/AgentMark." },
          { field: "snapshot_sites", index: 1, code: "invalid_site", message: '"exa mple.com" is not a host/path rule.' },
        ],
      }),
    });
    fireEvent.change(await screen.findByTestId("folder-input"), { target: { value: "relative" } });
    fireEvent.change(screen.getByTestId("new-site-input"), { target: { value: "exa mple.com" } });
    fireEvent.click(screen.getByTestId("add-site"));
    fireEvent.click(screen.getByTestId("save-settings"));
    expect(await screen.findByText("Use a full path, like ~/Documents/AgentMark.")).toBeInTheDocument();
    expect(within(screen.getByTestId("site-errors")).getByText(/is not a host\/path rule/)).toBeInTheDocument();
    expect(screen.getByTestId("save-status")).toHaveTextContent("Some settings need fixing");
  });

  it("shows a write failure", async () => {
    setup({ save_settings: () => Promise.reject("failed to write config.toml") });
    fireEvent.click(await screen.findByTestId("summaries-switch"));
    fireEvent.click(screen.getByTestId("save-settings"));
    expect(await screen.findByText("failed to write config.toml")).toBeInTheDocument();
  });

  it("shows a load failure with a retry", async () => {
    let fail = true;
    setup({ get_setup_state: () => (fail ? Promise.reject("nope") : readyState()) });
    expect(await screen.findByTestId("settings-error")).toHaveTextContent("nope");
    fail = false;
    fireEvent.click(screen.getByText("Try again"));
    expect(await screen.findByTestId("settings")).toBeInTheDocument();
  });
});
