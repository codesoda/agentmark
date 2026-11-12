import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "./App";
import { CLI_FAILED, CONNECTION, HOST_WAITING, STEP_DONE, readyState, setupState } from "./mock/fixtures";
import { installHandlers, type Handlers } from "./mock/handlers";
import type { StepId } from "./types";

function handlers(overrides: Handlers = {}): Handlers {
  return {
    app_info: () => ({ version: "0.2.6", updates_enabled: false }),
    run_step: ({ step }) => STEP_DONE[step as StepId],
    get_setup_state: () => readyState(),
    get_connection: () => CONNECTION,
    "plugin:window|close": () => undefined,
    ...overrides,
  };
}

const row = (id: string) => screen.getByTestId(`step-${id}`);

describe("App launch", () => {
  it("runs the three steps in order and says it is ready", async () => {
    const calls = installHandlers(handlers());
    render(<App />);
    expect(await screen.findByText("AgentMark is ready")).toBeInTheDocument();
    expect(calls.run_step.map((c) => c.step)).toEqual(["cli", "native_host", "skill"]);
    for (const id of ["cli", "native_host", "skill", "extension"]) expect(row(id)).toHaveAttribute("data-state", "done");
    expect(screen.getByTestId("ready-summary")).toHaveTextContent("674");
    expect(screen.getByTestId("tab-settings")).toBeInTheDocument();
  });

  it("shows progress while the steps run", async () => {
    installHandlers(handlers({ run_step: () => new Promise(() => {}) }));
    render(<App />);
    expect(await screen.findByText("Welcome to AgentMark")).toBeInTheDocument();
    await waitFor(() => expect(row("cli")).toHaveAttribute("data-state", "running"));
    expect(row("skill")).toHaveAttribute("data-state", "pending");
    expect(screen.getByTestId("primary-action")).toBeDisabled();
  });

  it("offers Try again on a failed step and runs only that step again", async () => {
    let attempts = 0;
    const calls = installHandlers(
      handlers({
        run_step: ({ step }) => {
          if (step === "cli") return ++attempts === 1 ? CLI_FAILED : STEP_DONE.cli;
          return STEP_DONE[step as StepId];
        },
      }),
    );
    render(<App />);
    expect(await screen.findByText("Something needs attention")).toBeInTheDocument();
    expect(row("cli")).toHaveAttribute("data-state", "failed");
    expect(row("cli")).toHaveTextContent("missing from this copy of the app");
    fireEvent.click(screen.getByTestId("retry-cli"));
    await waitFor(() => expect(row("cli")).toHaveAttribute("data-state", "done"));
    expect(calls.run_step.map((c) => c.step)).toEqual(["cli", "native_host", "skill", "cli"]);
  });

  it("reports a command that throws as a failed step", async () => {
    installHandlers(handlers({ run_step: ({ step }) => (step === "skill" ? Promise.reject("disk full") : STEP_DONE[step as StepId]) }));
    render(<App />);
    await waitFor(() => expect(row("skill")).toHaveAttribute("data-state", "failed"));
    expect(row("skill")).toHaveTextContent("disk full");
  });

  it("shows the native host waiting for an extension ID as waiting, not failed", async () => {
    installHandlers(
      handlers({
        run_step: ({ step }) => (step === "native_host" ? HOST_WAITING : STEP_DONE[step as StepId]),
        get_connection: () => null,
      }),
    );
    render(<App />);
    await waitFor(() => expect(row("native_host")).toHaveAttribute("data-state", "waiting"));
    expect(row("native_host")).toHaveTextContent("Waiting for the Chrome extension's ID.");
    expect(screen.getByText("Almost there")).toBeInTheDocument();
  });

  it("goes from first launch through setup to the extension screen", async () => {
    let state = setupState();
    installHandlers(
      handlers({
        get_setup_state: () => state,
        get_connection: () => null,
        save_settings: () => {
          state = readyState({ bookmark_count: 0 });
          return { type: "saved", state };
        },
      }),
    );
    render(<App />);
    expect(screen.queryByTestId("tab-settings")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getByTestId("primary-action")).toHaveTextContent("Set up AgentMark"));
    fireEvent.click(screen.getByTestId("primary-action"));
    fireEvent.click(await screen.findByTestId("setup-submit"));
    expect(await screen.findByText("Add the Chrome extension")).toBeInTheDocument();
    expect(screen.getByTestId("tab-settings")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("extension-skip"));
    expect(await screen.findByText("Almost there")).toBeInTheDocument();
  });

  it("Done closes the window", async () => {
    const calls = installHandlers(handlers());
    render(<App />);
    await screen.findByText("AgentMark is ready");
    fireEvent.click(screen.getByTestId("primary-action"));
    await waitFor(() => expect(calls["plugin:window|close"]).toHaveLength(1));
  });

  it("opens Settings and About from the header", async () => {
    installHandlers(handlers());
    render(<App />);
    await screen.findByText("AgentMark is ready");
    fireEvent.click(screen.getByTestId("tab-settings"));
    expect(await screen.findByTestId("settings")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("tab-about"));
    expect(await screen.findByTestId("about")).toBeInTheDocument();
    expect(within(screen.getByRole("navigation")).getByTestId("tab-about")).toHaveAttribute("aria-current", "page");
  });

  it("shows a state read error with a retry", async () => {
    let fail = true;
    installHandlers(handlers({ get_setup_state: () => (fail ? Promise.reject("AgentMark cannot find your home folder") : readyState()) }));
    render(<App />);
    expect(await screen.findByTestId("state-error")).toHaveTextContent("cannot find your home folder");
    fail = false;
    fireEvent.click(within(screen.getByTestId("state-error")).getByText("Try again"));
    expect(await screen.findByText("AgentMark is ready")).toBeInTheDocument();
  });
});
