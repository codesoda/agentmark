import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { agents, readyState, setupState } from "../mock/fixtures";
import { SUMMARIES_NOTE } from "../copy";
import { installHandlers, type Handlers } from "../mock/handlers";
import Setup from "./Setup";

function setup(overrides: Handlers = {}) {
  const calls = installHandlers({
    get_setup_state: () => setupState(),
    save_settings: () => ({ type: "saved", state: readyState() }),
    ...overrides,
  });
  const onDone = vi.fn();
  render(<Setup onDone={onDone} onBack={() => {}} />);
  return { calls, onDone };
}

describe("Setup", () => {
  it("asks the same two questions and switch as the extension, with the defaults filled in", async () => {
    setup();
    expect(await screen.findByText("Where should bookmarks live?")).toBeInTheDocument();
    expect(screen.getByText("Which agent writes summaries?")).toBeInTheDocument();
    expect(screen.getByTestId("folder-input")).toHaveValue("~/Documents/AgentMark");
    expect(screen.getByTestId("agent-claude").querySelector("input")).toBeChecked();
    expect(screen.getByTestId("agent-pi")).toHaveTextContent("Not installed");
    expect(screen.getByTestId("agent-codex")).toHaveTextContent("Found");
    expect(screen.getByTestId("summaries-switch")).toHaveAttribute("aria-checked", "true");
    expect(screen.getByText("Write a summary and tags after each save")).toBeInTheDocument();
    expect(screen.getByText(new RegExp(SUMMARIES_NOTE.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")))).toBeInTheDocument();
    // Review finding #15: what turning it off does for those sites.
    expect(screen.getByText(/With this off, those pages keep the text taken straight from the page\./)).toBeInTheDocument();
  });

  it("saves the folder, agent and switch, and finishes", async () => {
    const { calls, onDone } = setup();
    fireEvent.change(await screen.findByTestId("folder-input"), { target: { value: "~/Bookmarks" } });
    fireEvent.click(screen.getByTestId("agent-codex").querySelector("input")!);
    fireEvent.click(screen.getByTestId("summaries-switch"));
    fireEvent.click(screen.getByTestId("setup-submit"));
    await waitFor(() => expect(onDone).toHaveBeenCalledTimes(1));
    expect(calls.save_settings[0].changes).toEqual({
      storage_path: "~/Bookmarks",
      default_agent: "codex",
      enrichment_enabled: false,
    });
  });

  it("shows field errors next to the field and does not finish", async () => {
    const { onDone } = setup({
      save_settings: () => ({
        type: "invalid",
        errors: [{ field: "storage_path", index: null, code: "not_writable", message: "AgentMark cannot write to /Volumes/X." }],
      }),
    });
    fireEvent.click(await screen.findByTestId("setup-submit"));
    expect(await screen.findByTestId("field-error")).toHaveTextContent("AgentMark cannot write to /Volumes/X.");
    expect(screen.getByTestId("folder-input")).toHaveAttribute("aria-invalid", "true");
    expect(onDone).not.toHaveBeenCalled();
  });

  it("shows a write failure", async () => {
    setup({ save_settings: () => Promise.reject("failed to write /x/config.toml: denied") });
    fireEvent.click(await screen.findByTestId("setup-submit"));
    expect(await screen.findByTestId("setup-error")).toHaveTextContent("failed to write");
  });

  it("says why summaries start off when no agent is found", async () => {
    setup({ get_setup_state: () => setupState({ agents: agents({}) }) });
    expect(await screen.findByTestId("no-agent")).toHaveTextContent("Install Claude Code, Codex or pi");
    expect(screen.getByTestId("summaries-switch")).toHaveAttribute("aria-checked", "false");
  });

  it("fills the folder from the native dialog", async () => {
    const { calls } = setup({
      choose_folder: () => ({ outcome: "chosen", path: "/Users/me/Notes", path_display: "~/Notes", reason: null }),
    });
    fireEvent.click(await screen.findByTestId("choose-folder"));
    await waitFor(() => expect(screen.getByTestId("folder-input")).toHaveValue("~/Notes"));
    expect(calls.choose_folder[0]).toEqual({ startPath: "~/Documents/AgentMark" });
  });

  it("falls back to a typed path when no dialog is available", async () => {
    setup({
      choose_folder: () => ({ outcome: "unavailable", path: null, path_display: null, reason: "No folder dialog is available." }),
    });
    fireEvent.click(await screen.findByTestId("choose-folder"));
    expect(await screen.findByTestId("folder-note")).toHaveTextContent("Type the path instead.");
    expect(screen.getByTestId("folder-input")).toHaveValue("~/Documents/AgentMark");
  });

  it("keeps the path when the dialog is cancelled", async () => {
    setup({ choose_folder: () => ({ outcome: "cancelled", path: null, path_display: null, reason: null }) });
    fireEvent.click(await screen.findByTestId("choose-folder"));
    await waitFor(() => expect(screen.getByTestId("choose-folder")).toHaveTextContent("Choose…"));
    expect(screen.getByTestId("folder-input")).toHaveValue("~/Documents/AgentMark");
    expect(screen.queryByTestId("folder-note")).not.toBeInTheDocument();
  });

  it("warns that an unreadable settings file will be replaced", async () => {
    setup({ get_setup_state: () => setupState({ config_problem: "invalid TOML" }) });
    expect(await screen.findByTestId("config-problem")).toHaveTextContent("keep a copy of it");
  });

  it("shows a load failure with a retry", async () => {
    let fail = true;
    setup({ get_setup_state: () => (fail ? Promise.reject("boom") : setupState()) });
    expect(await screen.findByTestId("setup-load-error")).toHaveTextContent("boom");
    fail = false;
    fireEvent.click(screen.getByText("Try again"));
    expect(await screen.findByTestId("setup")).toBeInTheDocument();
  });
});
