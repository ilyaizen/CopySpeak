#!/usr/bin/env node
import { execFileSync, spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { request } from "node:http";
import { dirname, join } from "node:path";

const ENGINES = new Set(["cartesia", "openai", "elevenlabs", "local"]);

const CLAUDE_DIR = process.env.CLAUDE_CONFIG_DIR || join(homedir(), ".claude");
const CONFIG_PATH = join(CLAUDE_DIR, "hooks", "copyspeak", "config.json");

const DEFAULT_CONFIG = {
  enabled: true,
  engine: null,
  effect: null,
  max_chars: 700,
  launch: false
};

function loadConfig() {
  let cfg = { ...DEFAULT_CONFIG };

  if (existsSync(CONFIG_PATH)) {
    try {
      cfg = { ...DEFAULT_CONFIG, ...JSON.parse(readFileSync(CONFIG_PATH, "utf8")) };
    } catch {
      // Corrupt config — use defaults
    }
  } else {
    // First run — write defaults so skills can edit the file
    try {
      mkdirSync(dirname(CONFIG_PATH), { recursive: true });
      writeFileSync(CONFIG_PATH, JSON.stringify(cfg, null, 2), "utf8");
    } catch {
      // Non-fatal — proceed with in-memory defaults
    }
  }

  // Env vars override config file (backward compat / CI override)
  const envEnabled = process.env.COPYSPEAK_CLAUDE_ENABLED ?? process.env.COPYSPEAK_PI_ENABLED;
  if (envEnabled != null && envEnabled !== "") {
    cfg.enabled = !/^(0|false|no|off)$/i.test(envEnabled);
  }

  const envEngine = process.env.COPYSPEAK_CLAUDE_ENGINE ?? process.env.COPYSPEAK_PI_ENGINE;
  if (ENGINES.has(envEngine)) cfg.engine = envEngine;

  const envEffect = process.env.COPYSPEAK_CLAUDE_EFFECT ?? process.env.COPYSPEAK_PI_EFFECT;
  if (envEffect) cfg.effect = envEffect;

  const envMaxChars = process.env.COPYSPEAK_CLAUDE_MAX_CHARS ?? process.env.COPYSPEAK_PI_MAX_CHARS;
  if (envMaxChars) cfg.max_chars = Number(envMaxChars);

  const envLaunch = process.env.COPYSPEAK_CLAUDE_LAUNCH ?? process.env.COPYSPEAK_PI_LAUNCH;
  if (envLaunch != null && envLaunch !== "") {
    cfg.launch = !/^(0|false|no|off)$/i.test(envLaunch);
  }

  return cfg;
}

const config = loadConfig();
const hookInput = await readStdinJson();

if (!config.enabled) {
  console.log("🔇 copyspeak disabled");
  process.exit(0);
}

if (config.launch) launchCopySpeak();

const transcriptPath = hookInput?.transcript_path;
const text = truncateAtBoundary(
  cleanForSpeech(findLastAssistantText(transcriptPath)),
  config.max_chars
);

if (!text) process.exit(0);

try {
  await postSpeak(text, config.engine, config.effect);
  const label = config.engine ?? "default";
  console.log(`🔊 copyspeak · ${text.length} chars [${label}]`);
} catch (error) {
  if (error.code === "ECONNREFUSED") {
    console.log("⚠️ copyspeak · app not running");
  } else {
    console.error(`copyspeak hook: ${String(error)}`);
  }
  process.exit(0);
}

// --- helpers ---

async function readStdinJson() {
  let input = "";
  for await (const chunk of process.stdin) input += chunk;
  if (!input.trim()) return undefined;
  try {
    return JSON.parse(input);
  } catch {
    return undefined;
  }
}

function findLastAssistantText(transcriptPath) {
  if (!transcriptPath || !existsSync(transcriptPath)) return "";

  let lastText = "";
  const lines = readFileSync(transcriptPath, "utf8").trim().split(/\r?\n/);
  for (const line of lines) {
    if (!line.trim()) continue;
    try {
      const entry = JSON.parse(line);
      const message = entry.message || entry;
      if (message?.role !== "assistant") continue;
      const t = extractText(message);
      if (t) lastText = t;
    } catch {
      // Ignore malformed transcript lines
    }
  }

  return lastText;
}

function extractText(message) {
  const content = message?.content;
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return "";
  return content
    .map((part) => {
      if (typeof part === "string") return part;
      if (part?.type === "text") return part.text || "";
      return "";
    })
    .join("\n");
}

async function postSpeak(text, engine, effect) {
  const body = JSON.stringify({ text, engine, effect });
  const url = new URL(process.env.COPYSPEAK_CONTROL_URL || "http://127.0.0.1:43117/speak");

  await new Promise((resolve, reject) => {
    const req = request(
      {
        method: "POST",
        hostname: url.hostname,
        port: url.port,
        path: `${url.pathname}${url.search}`,
        headers: {
          "content-type": "application/json",
          "content-length": Buffer.byteLength(body)
        }
      },
      (res) => {
        let responseBody = "";
        res.setEncoding("utf8");
        res.on("data", (chunk) => (responseBody += chunk));
        res.on("end", () => {
          if (res.statusCode >= 200 && res.statusCode < 300) resolve();
          else reject(new Error(`HTTP ${res.statusCode}: ${responseBody}`));
        });
      }
    );
    req.on("error", reject);
    req.end(body);
  });
}

function launchCopySpeak() {
  if (isCopySpeakRunning()) return;
  const exe = process.env.COPYSPEAK_EXE || findBuiltCopySpeak();
  if (!exe || !existsSync(exe)) return;
  spawn(exe, [], { detached: true, stdio: "ignore", windowsHide: true }).unref();
}

function isCopySpeakRunning() {
  if (process.platform !== "win32") return false;
  try {
    const output = execFileSync("tasklist.exe", ["/NH"], {
      encoding: "utf8",
      windowsHide: true
    });
    return /^(?:copyspeak(?:-tts)?|CopySpeak)\.exe\s+/im.test(output);
  } catch {
    return false;
  }
}

function findBuiltCopySpeak() {
  const cwd = process.cwd();
  const candidates = [
    join(cwd, "src-tauri", "target", "release", "copyspeak.exe"),
    join(cwd, "src-tauri", "target", "release", "CopySpeak.exe"),
    join(cwd, "src-tauri", "target", "debug", "copyspeak.exe"),
    join(cwd, "src-tauri", "target", "debug", "CopySpeak.exe")
  ];
  return candidates.find(existsSync);
}

function cleanForSpeech(text) {
  return text
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/https?:\/\/\S+/g, " link ")
    .replace(/[#*_>~|]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function truncateAtBoundary(text, max) {
  if (text.length <= max) return text;
  const slice = text.slice(0, max);
  const boundary = Math.max(
    slice.lastIndexOf(". "),
    slice.lastIndexOf("! "),
    slice.lastIndexOf("? "),
    slice.lastIndexOf("\n")
  );
  return boundary > max * 0.5 ? slice.slice(0, boundary + 1) : slice;
}
