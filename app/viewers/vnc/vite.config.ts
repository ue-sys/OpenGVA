import { defineConfig } from "vite";

export default defineConfig({
  // Tauri's dev URL (src-tauri/tauri.conf.json) points here. Not Vite's usual
  // port: that is the HMI's.
  server: { port: 5183, strictPort: true },
  // The webview on the image is WebKitGTK; nothing older needs supporting.
  // Nothing much older would do: noVNC waits, as it is loaded, to find out
  // what the browser can decode.
  build: { target: "safari16" },
});
