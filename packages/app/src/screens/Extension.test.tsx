import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CONNECTION, HOST_WAITING, STALE_CONNECTION, STEP_DONE } from "../mock/fixtures";
import { installHandlers, type Handlers } from "../mock/handlers";
import type { StepState } from "../steps";
import Extension from "./Extension";

function setup(overrides: Handlers = {}, props: { webStoreUrl?: string; nativeHost?: StepState } = {}) {
  const calls = installHandlers({
    get_connection: () => null,
    show_dev_extension: () => ({ path: "/Users/me/.agentmark/extension", path_display: "~/.agentmark/extension" }),
    add_extension_id: () => STEP_DONE.native_host,
    open_chrome_extensions: () => undefined,
    open_url: () => undefined,
    ...overrides,
  });
  const onNativeHost = vi.fn();
  const onConnection = vi.fn();
  const onDone = vi.fn();
  const view = render(
    <Extension
      nativeHost={props.nativeHost ?? HOST_WAITING}
      onNativeHost={onNativeHost}
      onConnection={onConnection}
      onDone={onDone}
      webStoreUrl={props.webStoreUrl}
    />,
  );
  return { calls, onNativeHost, onConnection, onDone, view };
}

afterEach(() => {
  vi.useRealTimers();
});

describe("Add the Chrome extension", () => {
  it("shows the development path, labelled, while there is no store listing", async () => {
    // The default native host report here is a build with no store ID (waiting).
    setup({}, { webStoreUrl: "" });
    expect(screen.getByTestId("dev-steps")).toHaveTextContent("Development build");
    expect(screen.getByText(/Load unpacked/)).toBeInTheDocument();
    expect(screen.queryByTestId("open-store")).not.toBeInTheDocument();
    expect(await screen.findByTestId("waiting")).toHaveTextContent("Waiting for Chrome…");
  });

  it("shows one store button once the listing exists", async () => {
    const { calls } = setup({}, { webStoreUrl: "https://chromewebstore.google.com/detail/agentmark/x" });
    expect(screen.queryByTestId("dev-steps")).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId("open-store"));
    await waitFor(() => expect(calls.open_url).toEqual([{ url: "https://chromewebstore.google.com/detail/agentmark/x" }]));
  });

  it("turns Waiting into Connected when the liveness file appears", async () => {
    vi.useFakeTimers();
    let connected = false;
    const { onConnection } = setup({ get_connection: () => (connected ? CONNECTION : null) });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(screen.getByTestId("waiting")).toBeInTheDocument();
    connected = true;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(screen.getByTestId("connected")).toHaveTextContent("Chrome extension 0.2.6");
    expect(onConnection).toHaveBeenCalledWith(CONNECTION);
    expect(screen.getByTestId("extension-done")).toBeInTheDocument();
  });

  // Review finding #12: the liveness file stays after Chrome quits.
  it("does not show Connected for a record from before the app started", async () => {
    vi.useFakeTimers();
    let record = STALE_CONNECTION;
    setup({ get_connection: () => record });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(screen.queryByTestId("connected")).not.toBeInTheDocument();
    expect(screen.getByTestId("waiting")).toHaveTextContent("Waiting for Chrome…");
    expect(screen.getByTestId("last-seen")).toHaveTextContent(/^Last connected .+ · version 0\.2\.6\.$/);
    expect(screen.queryByTestId("extension-done")).not.toBeInTheDocument();
    expect(screen.getByTestId("extension-skip")).toBeInTheDocument();

    // The extension connects now: a fresh record.
    record = { ...STALE_CONNECTION, connected_at: new Date().toISOString() };
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(screen.getByTestId("connected")).toBeInTheDocument();
    expect(screen.getByTestId("extension-done")).toBeInTheDocument();
  });

  it("stops polling when the screen closes", async () => {
    vi.useFakeTimers();
    const { calls, view } = setup();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2100);
    });
    const count = calls.get_connection.length;
    expect(count).toBeGreaterThanOrEqual(3);
    view.unmount();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(calls.get_connection.length).toBe(count);
  });

  it("writes and shows the extension folder", async () => {
    setup();
    fireEvent.click(screen.getByTestId("show-folder"));
    expect(await screen.findByTestId("dev-folder")).toHaveTextContent("~/.agentmark/extension");
  });

  it("explains how to open chrome://extensions when Chrome cannot be opened", async () => {
    setup({ open_chrome_extensions: () => Promise.reject("Chrome did not open.") });
    fireEvent.click(screen.getByTestId("open-chrome"));
    expect(await screen.findByText(/type chrome:\/\/extensions in the address bar/)).toBeInTheDocument();
  });

  it("registers a pasted extension ID", async () => {
    const { calls, onNativeHost } = setup();
    fireEvent.change(screen.getByTestId("extension-id"), { target: { value: "abcdefghijklmnopabcdefghijklmnop" } });
    fireEvent.click(screen.getByTestId("register-id"));
    await waitFor(() => expect(onNativeHost).toHaveBeenCalledWith(STEP_DONE.native_host));
    expect(calls.add_extension_id).toEqual([{ id: "abcdefghijklmnopabcdefghijklmnop" }]);
    expect(screen.getByTestId("extension-id")).toHaveValue("");
  });

  it("shows why an ID was refused", async () => {
    const { onNativeHost } = setup({
      add_extension_id: () => Promise.reject("That is not an extension ID. It is 32 letters from a to p, shown on the extension's card."),
    });
    fireEvent.change(screen.getByTestId("extension-id"), { target: { value: "nope" } });
    fireEvent.click(screen.getByTestId("register-id"));
    expect(await screen.findByTestId("id-error")).toHaveTextContent("32 letters");
    expect(onNativeHost).not.toHaveBeenCalled();
  });

  it("asks for no ID when the launch step registered the store ID", () => {
    setup({}, { nativeHost: STEP_DONE.native_host });
    expect(screen.getByTestId("dev-steps")).toHaveTextContent("Development build");
    expect(screen.getByTestId("dev-steps")).toHaveTextContent("Chrome already accepts this copy, so there is nothing to paste.");
    expect(screen.queryByText("Copy the ID from the AgentMark card and paste it here.")).not.toBeInTheDocument();
    // Pasting is a collapsed extra, for a copy with another ID.
    const other = screen.getByTestId("other-id");
    expect(other.tagName).toBe("DETAILS");
    expect(other).not.toHaveAttribute("open");
    expect(other).toHaveTextContent("Loaded a copy with a different ID?");
    expect(other).toContainElement(screen.getByTestId("extension-id"));
    expect(screen.queryByTestId("id-registered")).not.toBeInTheDocument();
  });

  it("registers a different ID from the collapsed option", async () => {
    const { calls, onNativeHost, view } = setup({}, { nativeHost: STEP_DONE.native_host });
    fireEvent.click(screen.getByText("Loaded a copy with a different ID?"));
    fireEvent.change(screen.getByTestId("extension-id"), { target: { value: "abcdefghijklmnopabcdefghijklmnop" } });
    fireEvent.click(screen.getByTestId("register-id"));
    await waitFor(() => expect(onNativeHost).toHaveBeenCalledWith(STEP_DONE.native_host));
    expect(calls.add_extension_id).toEqual([{ id: "abcdefghijklmnopabcdefghijklmnop" }]);
    view.rerender(
      <Extension nativeHost={STEP_DONE.native_host} onNativeHost={onNativeHost} onConnection={vi.fn()} onDone={vi.fn()} webStoreUrl="" />,
    );
    expect(await screen.findByTestId("id-registered")).toHaveTextContent("Registered with Chrome");
  });

  it("keeps the paste step, ticked, after a build with no store ID accepts an ID", () => {
    const { view, onNativeHost } = setup();
    expect(screen.getByText("Copy the ID from the AgentMark card and paste it here.")).toBeInTheDocument();
    expect(screen.queryByTestId("other-id")).not.toBeInTheDocument();
    view.rerender(
      <Extension nativeHost={STEP_DONE.native_host} onNativeHost={onNativeHost} onConnection={vi.fn()} onDone={vi.fn()} webStoreUrl="" />,
    );
    expect(screen.getByText("Copy the ID from the AgentMark card and paste it here.")).toBeInTheDocument();
    expect(screen.getByTestId("id-registered")).toHaveTextContent("Registered with Chrome");
    expect(screen.getByTestId("dev-steps")).not.toHaveTextContent("nothing to paste");
  });

  it("can be skipped", () => {
    const { onDone } = setup();
    fireEvent.click(screen.getByTestId("extension-skip"));
    expect(onDone).toHaveBeenCalled();
  });
});
