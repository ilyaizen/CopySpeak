// Extension build: bundle src/*.ts → dist/*.js (ESM workers need bundled deps).
import { build } from "bun";
import { rm, mkdir } from "node:fs/promises";
import { resolve } from "node:path";

const destination = resolve("dist");
if (destination !== resolve(import.meta.dirname, "dist"))
  throw new Error("Run the extension build from browser-extension.");
await rm(destination, { recursive: true, force: true });
await mkdir("dist", { recursive: true });

const results = await Promise.all([
  build({
    entrypoints: ["src/content.ts"],
    outdir: "dist",
    naming: "content.js",
    target: "browser",
    format: "iife",
    minify: true
  }),
  build({
    entrypoints: ["src/worker.ts", "src/options.ts"],
    outdir: "dist",
    naming: "[name].js",
    target: "browser",
    format: "esm",
    minify: true
  })
]);
for (const result of results) {
  if (!result.success) throw new AggregateError(result.logs, "Companion bundle failed");
}

// Static assets ship verbatim.
await Bun.write("dist/manifest.json", Bun.file("manifest.json"));
for (const file of ["options.html", "options.css"]) await Bun.write(`dist/${file}`, Bun.file(file));
await mkdir("dist/icons", { recursive: true });
for (const size of [32, 64, 128]) {
  await Bun.write(`dist/icons/${size}.png`, Bun.file(`../src-tauri/icons/${size}x${size}.png`));
}
console.log("dist/ ready: companion scripts, settings page, and CopySpeak icons");
