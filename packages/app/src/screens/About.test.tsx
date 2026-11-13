import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { SITE_URL } from "../constants";
import { installHandlers, type Handlers } from "../mock/handlers";
import About from "./About";

function setup(overrides: Handlers = {}) {
  const calls = installHandlers({
    app_info: () => ({ version: "0.2.6", updates_enabled: false }),
    open_url: () => undefined,
    check_for_update: () => null,
    ...overrides,
  });
  render(<About />);
  return calls;
}

describe("About", () => {
  it("shows the version and says updates are off without a key", async () => {
    setup();
    expect(await screen.findByTestId("about-version")).toHaveTextContent("Version 0.2.6");
    expect(screen.getByTestId("updates-off")).toHaveTextContent("Automatic updates are off in this build.");
    expect(screen.queryByTestId("check-update")).not.toBeInTheDocument();
  });

  it("opens the website", async () => {
    const calls = setup();
    fireEvent.click(await screen.findByTestId("link-Website"));
    await waitFor(() => expect(calls.open_url).toEqual([{ url: SITE_URL }]));
    expect(SITE_URL).toBe("https://agentmark.app");
  });

  it("checks for updates when they are on", async () => {
    setup({ app_info: () => ({ version: "0.2.6", updates_enabled: true }) });
    fireEvent.click(await screen.findByTestId("check-update"));
    expect(await screen.findByText("AgentMark is up to date.")).toBeInTheDocument();
  });

  it("offers to install an available update", async () => {
    const calls = setup({
      app_info: () => ({ version: "0.2.6", updates_enabled: true }),
      check_for_update: () => ({ version: "0.3.0", notes: null }),
      install_update: () => undefined,
    });
    fireEvent.click(await screen.findByTestId("check-update"));
    expect(await screen.findByText("Version 0.3.0 is available.")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("install-update"));
    await waitFor(() => expect(calls.install_update).toHaveLength(1));
  });

  it("shows an update error", async () => {
    setup({ app_info: () => ({ version: "0.2.6", updates_enabled: true }), check_for_update: () => Promise.reject("offline") });
    fireEvent.click(await screen.findByTestId("check-update"));
    expect(await screen.findByText("offline")).toBeInTheDocument();
  });
});
