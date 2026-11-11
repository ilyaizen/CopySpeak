// Extension build: bundle src/*.ts → dist/*.js (ESM workers need bundled deps).
// Usage: bun build.mjs [chrome|firefox]   (default: chrome)
//   chrome  → dist/          MV3 service worker (ESM); manifest key pins the ID.
//   firefox → dist-firefox/  MV3 event page (classic IIFE); gecko ID; Firefox 153+.
// manifest.json stays the single source. Firefox fields are derived here, never hand-edited.
import { build } from "bun";
import { rm, mkdir } from "node:fs/promises";
import { resolve } from "node:path";

const target = process.argv[2] ?? "chrome";
if (target !== "chrome" && target !== "firefox")
  throw new Error(`Unknown build target "${target}". Use chrome or firefox.`);
const outdir = target === "firefox" ? "dist-firefox" : "dist";

const destination = resolve(outdir);
if (destination !== resolve(import.meta.dirname, outdir))
  throw new Error("Run the extension build from browser-extension.");
await rm(destination, { recursive: true, force: true });
await mkdir(outdir, { recursive: true });

// Firefox 153 is the first release with documentId targeting (tabs.sendMessage,
// scripting.executeScript documentIds, sender.documentId); the worker needs all three.
const FIREFOX_GECKO_ID = "copyspeak-companion@ilyaizen.github.io";
const FIREFOX_MIN_VERSION = "153.0";

const results = await Promise.all([
  build({
    entrypoints: ["src/content.ts"],
    outdir,
    naming: "content.js",
    target: "browser",
    format: "iife",
    minify: true
  }),
  // Firefox background event pages load classic scripts; Chrome's service worker takes ESM.
  build({
    entrypoints: ["src/worker.ts"],
    outdir,
    naming: "worker.js",
    target: "browser",
    format: target === "firefox" ? "iife" : "esm",
    minify: true
  }),
  build({
    entrypoints: ["src/options.ts"],
    outdir,
    naming: "options.js",
    target: "browser",
    format: "esm",
    minify: true
  })
]);
for (const result of results) {
  if (!result.success) throw new AggregateError(result.logs, "Companion bundle failed");
}

// Chrome ships manifest.json verbatim. Firefox gets the derived manifest.
if (target === "firefox") {
  const base = JSON.parse(await Bun.file("manifest.json").text());
  // Chrome-only keys: key pins a Chrome ID; minimum_chrome_version does not apply to Firefox.
  const { key: _key, minimum_chrome_version: _min, background: _bg, ...rest } = base;
  const manifest = {
    ...rest,
    background: { scripts: ["worker.js"] },
    browser_specific_settings: {
      gecko: {
        id: FIREFOX_GECKO_ID,
        strict_min_version: FIREFOX_MIN_VERSION,
        data_collection_permissions: { required: ["none"] }
      }
    }
  };
  await Bun.write(`${outdir}/manifest.json`, JSON.stringify(manifest, null, 2) + "\n");
} else {
  await Bun.write(`${outdir}/manifest.json`, Bun.file("manifest.json"));
}
for (const file of ["options.html", "options.css"])
  await Bun.write(`${outdir}/${file}`, Bun.file(file));
await mkdir(`${outdir}/icons`, { recursive: true });
for (const size of [32, 64, 128]) {
  await Bun.write(
    `${outdir}/icons/${size}.png`,
    Bun.file(`../src-tauri/icons/${size}x${size}.png`)
  );
}
console.log(`${outdir}/ ready (${target}): companion scripts, settings page, and CopySpeak icons`);
