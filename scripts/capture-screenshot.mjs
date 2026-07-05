/**
 * Capture a fresh screenshot of the CopySpeak Tauri window and update references.
 *
 * Prerequisites:
 *   - Tauri dev server running: `bun run tauri dev`
 *   - The CopySpeak window must be visible (not minimized to tray)
 *
 * What it does:
 *   1. Reads the current version from src-tauri/tauri.conf.json
 *   2. Calls screenshot-window.ps1 to capture the window
 *   3. Saves to static/screen-v{version}.png
 *   4. Patches src/lib/components/landing/screenshots.svelte to reference the new file
 *
 * Usage:
 *   node scripts/capture-screenshot.mjs              # capture + update reference
 *   node scripts/capture-screenshot.mjs --no-update  # capture only, don't patch svelte
 */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, "..");

const TAURI_CONF = resolve(projectRoot, "src-tauri/tauri.conf.json");
const SCREENSHOTS_SVELTE = resolve(
  projectRoot,
  "src/lib/components/landing/screenshots.svelte"
);
const SCREENSHOT_SCRIPT = resolve(projectRoot, "scripts/screenshot-window.ps1");
const STATIC_DIR = resolve(projectRoot, "static");

// --- 1. Read version from tauri.conf.json ---
function getVersion() {
  const conf = JSON.parse(readFileSync(TAURI_CONF, "utf-8"));
  return conf.version;
}

// --- 2. Capture screenshot via PowerShell ---
function captureScreenshot(outPath) {
  const result = spawnSync(
    "pwsh",
    [
      "-NoProfile",
      "-File",
      SCREENSHOT_SCRIPT,
      "-WindowTitle",
      "CopySpeak",
      "-OutPath",
      outPath,
    ],
    { stdio: "inherit", shell: false }
  );

  if (result.status !== 0) {
    console.error(
      "\nScreenshot capture failed. Make sure:\n" +
        "  1. Tauri dev is running: bun run tauri dev\n" +
        "  2. The CopySpeak window is visible (not minimized to tray)\n"
    );
    process.exit(result.status ?? 1);
  }
}

// --- 3. Patch screenshots.svelte to reference the new filename ---
function updateSvelteReference(filename) {
  if (!existsSync(SCREENSHOTS_SVELTE)) {
    console.log(`Skipping Svelte reference update (file not found: ${SCREENSHOTS_SVELTE})`);
    return;
  }

  const content = readFileSync(SCREENSHOTS_SVELTE, "utf-8");
  const updated = content.replace(
    /src="\/screen-v[^"]+\.png"/,
    `src="/${filename}"`
  );

  if (updated === content) {
    console.log("Svelte reference already up to date or pattern not found.");
    return;
  }

  writeFileSync(SCREENSHOTS_SVELTE, updated);
  console.log(`Updated screenshots.svelte → /${filename}`);
}

// --- Main ---
const version = getVersion();
const filename = `screen-v${version}.png`;
const outPath = resolve(STATIC_DIR, filename);
const shouldUpdate = !process.argv.includes("--no-update");

console.log(`CopySpeak v${version}`);
console.log(`Capturing to static/${filename}...\n`);

captureScreenshot(outPath);

if (shouldUpdate) {
  console.log("\nUpdating Svelte reference...");
  updateSvelteReference(filename);
}

console.log(`\nDone. Screenshot: static/${filename}`);
