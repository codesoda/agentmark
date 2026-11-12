import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import type { View } from "./components/Header";
import "./index.css";

async function start() {
  let initialView: View | undefined;
  // In a plain browser (vite dev without Tauri) use the mock commands, so the
  // screens can be previewed and screenshotted. Not part of production builds.
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    const { installDevMock } = await import("./dev-mock");
    initialView = installDevMock();
  }
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App initialView={initialView} />
    </StrictMode>,
  );
}

void start();
