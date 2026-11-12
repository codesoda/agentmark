import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri loads http://localhost:1430 in `tauri dev` (tauri.conf.json devUrl).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1430, strictPort: true },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "safari14",
  },
});
