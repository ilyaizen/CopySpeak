import { spawnSync } from "node:child_process";
import process from "node:process";

const cargoCheck = spawnSync("cargo", ["--version"], {
  stdio: "ignore",
  shell: true,
  windowsHide: true
});

if (cargoCheck.status !== 0) {
  console.error(
    "Cargo is missing. Install Rust from https://rustup.rs/ and restart your terminal, then run bun run tauri dev again."
  );
  process.exit(1);
}

const args = process.argv.slice(2);
const result = spawnSync("bun", ["x", "tauri", ...args], { stdio: "inherit", shell: true, windowsHide: true });
process.exit(result.status ?? 1);
