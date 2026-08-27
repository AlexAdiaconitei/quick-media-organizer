// @ts-expect-error node builtins are untyped here, as with `process` below
import { rmSync } from "node:fs";

import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";

/**
 * The demo photo and clip exist so `pnpm capture-screenshots` can drive the
 * real UI against the dev server. They are ~2.6 MB and nothing in a shipped
 * build can reach them (see `getScreenshotMode`), so they are removed from the
 * production output rather than travelling inside every installer.
 *
 * Runs after SvelteKit's adapter, which writes `build/` from its own
 * `closeBundle`; plugins registered later see the finished directory.
 */
const DEV_ONLY_ASSETS = ["demo-sunset.jpg", "demo-video.mp4"];

/** @returns {import("vite").Plugin} */
function stripDevOnlyAssets() {
  return {
    name: "strip-dev-only-assets",
    apply: "build",
    closeBundle() {
      for (const name of DEV_ONLY_ASSETS) {
        // @ts-expect-error import.meta.dirname is a nodejs global
        rmSync(`${import.meta.dirname}/build/${name}`, { force: true });
      }
    },
  };
}

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [sveltekit(), stripDevOnlyAssets()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    open: false,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
