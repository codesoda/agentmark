import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const tailwind = require("tailwindcss");
const autoprefixer = require("autoprefixer");
const EXT = "/Users/chrisraethke/projects/agentmark/packages/extension";
export default defineConfig({
  root: "/tmp/am/cws-harness",
  plugins: [react()],
  resolve: { alias: { "@ext": EXT + "/src" }, dedupe: ["react", "react-dom"] },
  server: { port: 5299, strictPort: true, fs: { allow: ["/tmp/am/cws-harness", "/private/tmp/am/cws-harness", EXT] } },
  css: { postcss: { plugins: [tailwind({ content: [EXT + "/src/**/*.{ts,tsx}", "/tmp/am/cws-harness/*.tsx"] }), autoprefixer()] } },
});
