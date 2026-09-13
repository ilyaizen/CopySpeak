import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const directory = mkdtempSync(join(tmpdir(), "copy-speak-anti-slop-"));
const config = join(directory, "oxlint.json");
const allowed = join(directory, "allowed.ts");
const rejected = join(directory, "rejected.ts");

writeFileSync(
  config,
  JSON.stringify({
    jsPlugins: [
      {
        name: "anti-slop",
        specifier: resolve("tools/oxlint/anti-slop/index.ts")
      }
    ],
    rules: { "anti-slop/no-shape-in-symbol-names": "error" }
  })
);
writeFileSync(allowed, "offline.createWaveShaper();\n");
writeFileSync(rejected, "const createWaveShaper = () => {};\n");

function lint(file) {
  return spawnSync(process.execPath, ["x", "oxlint", "--config", config, "--no-ignore", file], {
    encoding: "utf8"
  });
}

try {
  const allowedResult = lint(allowed);
  if (allowedResult.status !== 0) throw new Error(allowedResult.stderr || allowedResult.stdout);

  const rejectedResult = lint(rejected);
  const output = rejectedResult.stderr + rejectedResult.stdout;
  if (rejectedResult.status === 0 || !output.includes('Rename symbol "createWaveShaper"')) {
    throw new Error(`Expected local createWaveShaper to be rejected.\n${output}`);
  }
} finally {
  rmSync(directory, { recursive: true, force: true });
}
