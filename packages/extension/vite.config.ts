import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { viteStaticCopy } from "vite-plugin-static-copy";

const __dirname = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [
    react(),
    {
      // Content scripts run as classic scripts, the MAIN-world one in the page's
      // own global scope. Wrap them so their top-level names cannot collide
      // with the page's.
      name: "wrap-content-scripts",
      generateBundle(_options, bundle) {
        for (const file of Object.values(bundle)) {
          if (file.type === "chunk" && file.fileName.startsWith("content/")) {
            file.code = `(() => {\n${file.code}\n})();\n`;
          }
        }
      },
    },
    viteStaticCopy({
      targets: [
        { src: "manifest.json", dest: "." },
      ],
    }),
  ],
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        popup: resolve(__dirname, "src/popup/index.html"),
        sidepanel: resolve(__dirname, "src/sidepanel/index.html"),
        background: resolve(__dirname, "src/background/service-worker.ts"),
        // Content scripts are loaded as classic scripts: each entry must bundle
        // to one self-contained file (no shared chunks, no imports, no exports).
        "content-posts": resolve(__dirname, "src/content/posts.entry.ts"),
        "content-linkedin-main": resolve(__dirname, "src/content/linkedin-main.entry.ts"),
      },
      output: {
        entryFileNames: (chunkInfo) => {
          if (chunkInfo.name === "background") {
            return "background.js";
          }
          if (chunkInfo.name.startsWith("content-")) {
            return "content/[name].js";
          }
          return "assets/[name]-[hash].js";
        },
      },
    },
  },
});
