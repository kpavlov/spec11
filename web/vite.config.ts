import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";

// One self-contained dist/index.html: the Rust binary embeds it and also serves it as the MCP Apps resource.
export default defineConfig({
  plugins: [svelte(), viteSingleFile()],
  // Rust proxies plain HTTP to this port; the HMR socket goes straight to Vite.
  server: { host: "127.0.0.1", port: 5173, strictPort: true, hmr: { host: "127.0.0.1", clientPort: 5173 } },
  build: { outDir: "dist", emptyOutDir: true },
});
