/**
 * Captures README screenshots from the real Svelte UI (demo mode).
 *
 * Usage:
 *   pnpm capture-screenshots
 *
 * Starts the web dev server itself when one is not already listening. Every
 * screen is fed canned data by src/lib/screenshotDemo.ts, so the images are
 * reproducible and contain no real files.
 */
import { mkdir } from "node:fs/promises";
import { spawn } from "node:child_process";
import { join } from "node:path";
import { chromium } from "playwright";

const ROOT = join(import.meta.dirname, "..");
const OUT_DIR = join(ROOT, "docs", "screenshots");
const BASE = "http://localhost:1420";

const WELCOME_VIEWPORT = { width: 880, height: 660 };
/** Tall enough for the sidebar's tabs, metadata and shortcut bar */
const WORKSPACE_VIEWPORT = { width: 1320, height: 1120 };
/** The video sidebar also carries the trim panel and its notes */
const VIDEO_VIEWPORT = { width: 1320, height: 1440 };
/** The settings step is one long form; unrolled it needs the room */
const SETTINGS_VIEWPORT = { width: 1200, height: 1900 };

async function waitForServer(maxMs = 45000) {
  const start = Date.now();
  while (Date.now() - start < maxMs) {
    try {
      const res = await fetch(BASE);
      if (res.ok) return;
    } catch {
      /* retry */
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error("Dev server not running — start it with: pnpm dev:web");
}

function startDevServer() {
  // dev:web, not dev: the latter launches the native window.
  const child = spawn("pnpm", ["dev:web"], {
    shell: process.platform === "win32",
    cwd: ROOT,
    stdio: "ignore",
    detached: true,
  });
  child.unref();
  return child;
}

async function captureWorkspace(page, mode, filename) {
  await page.setViewportSize(
    mode === "workspace-video" ? VIDEO_VIEWPORT : WORKSPACE_VIEWPORT,
  );
  await page.goto(`${BASE}/?screenshot=${mode}`, { waitUntil: "networkidle" });
  await page.waitForSelector(`[data-screenshot-ready='${mode}']`, { timeout: 15000 });

  if (mode === "workspace-video") {
    await page.waitForSelector("video.preview-media", { timeout: 15000 });
    await page.waitForFunction(() => {
      const video = document.querySelector("video.preview-media");
      return video instanceof HTMLVideoElement && video.readyState >= 2 && video.duration > 0;
    });
    await page.waitForTimeout(700);
  } else {
    await page.waitForSelector("img.preview-media", { timeout: 15000 });
    await page.waitForTimeout(400);
  }

  await page.locator(".app-shell").screenshot({
    path: join(OUT_DIR, filename),
  });
}

/** The batch panel is a modal: capture the dialog, not the dimmed page. */
async function captureBatch(page, mode, filename) {
  await page.setViewportSize(
    mode === "batch-settings" ? SETTINGS_VIEWPORT : WORKSPACE_VIEWPORT,
  );
  await page.goto(`${BASE}/?screenshot=${mode}`, { waitUntil: "networkidle" });
  await page.waitForSelector(".batch-card", { timeout: 15000 });
  if (mode === "batch-settings") {
    await page.waitForSelector(".batch-settings", { timeout: 15000 });
    // The settings list scrolls inside the dialog; unroll it for the shot.
    await page.addStyleTag({ content: ".batch-body { max-height: none !important; }" });
  } else if (mode === "batch-select") {
    await page.waitForSelector(".batch-tile", { timeout: 15000 });
  } else if (mode === "batch-done") {
    await page.waitForSelector(".batch-summary-savings", { timeout: 15000 });
  } else {
    await page.waitForSelector(".batch-item-list", { timeout: 15000 });
  }
  await page.waitForTimeout(500);
  await page.locator(".batch-card").screenshot({ path: join(OUT_DIR, filename) });
}

async function main() {
  await mkdir(OUT_DIR, { recursive: true });

  let startedDev = false;
  try {
    await waitForServer(2000);
  } catch {
    startDevServer();
    startedDev = true;
    await waitForServer();
  }

  const browser = await chromium.launch();
  const page = await browser.newPage({ deviceScaleFactor: 2 });

  await page.setViewportSize(WELCOME_VIEWPORT);
  await page.goto(`${BASE}/?screenshot=welcome`, { waitUntil: "networkidle" });
  await page.waitForSelector("[data-screenshot-ready='welcome']", { timeout: 15000 });
  await page.locator(".welcome-card").screenshot({
    path: join(OUT_DIR, "welcome.png"),
  });

  await captureWorkspace(page, "workspace", "workspace.png");
  await captureWorkspace(page, "workspace-video", "workspace-video.png");
  await captureBatch(page, "batch-select", "batch-select.png");
  await captureBatch(page, "batch-settings", "batch-settings.png");
  await captureBatch(page, "batch-progress", "batch-progress.png");
  await captureBatch(page, "batch-done", "batch-done.png");

  await browser.close();
  console.log("Screenshots saved to docs/screenshots/");

  if (startedDev) {
    console.log("Note: a background dev server may still be running on :1420");
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
