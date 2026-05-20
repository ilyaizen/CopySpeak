#!/usr/bin/env node
// Lightweight status line script — polled by Claude Code, must exit quickly.
import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const CLAUDE_DIR = process.env.CLAUDE_CONFIG_DIR || join(homedir(), ".claude");
const CONFIG_PATH = join(CLAUDE_DIR, "hooks", "copyspeak", "config.json");

let cfg = { enabled: true, engine: null };
if (existsSync(CONFIG_PATH)) {
  try {
    Object.assign(cfg, JSON.parse(readFileSync(CONFIG_PATH, "utf8")));
  } catch {
    // Corrupt config — show default state
  }
}

const label = cfg.engine ?? "copyspeak";
process.stdout.write(cfg.enabled ? `🔊 [${label}]` : "🔇 [copyspeak]");
