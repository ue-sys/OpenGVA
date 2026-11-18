import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid()],
  // milsymbol's ES module entry: the one that lets a single symbol set be
  // taken (src/milsymbol-2525c.ts). Its package.json does not offer it.
  resolve: {
    alias: {
      "milsymbol/index.mjs": fileURLToPath(new URL("./node_modules/milsymbol/index.mjs", import.meta.url)),
    },
  },
  // Tauri's dev URL (src-tauri/tauri.conf.json) points here.
  server: { port: 5173, strictPort: true },
  // The webview on the image is WebKitGTK; nothing older needs supporting.
  // The one large chunk is the symbol library, which is on its own so that
  // it is not read until a picture with symbols on it is first looked at.
  build: { target: "safari16", chunkSizeWarningLimit: 800 },
});
