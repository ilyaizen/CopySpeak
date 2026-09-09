// Extension build: bundle src/*.ts → dist/*.js (ESM workers need bundled deps).
import { build } from "bun";
import { rm, mkdir } from "node:fs/promises";

await rm("dist", { recursive: true, force: true });
await mkdir("dist", { recursive: true });

await Promise.all([
  build({
    entrypoints: ["src/content.ts"],
    outdir: "dist",
    naming: "content.js",
    target: "browser",
    format: "iife",
    minify: true
  }),
  build({
    entrypoints: ["src/worker.ts"],
    outdir: "dist",
    naming: "worker.js",
    target: "browser",
    format: "esm",
    minify: true
  })
]);

// Static assets ship verbatim.
await Bun.write("dist/manifest.json", Bun.file("manifest.json"));
console.log("dist/ ready: manifest.json, content.js, worker.js");
