#!/usr/bin/env node
import { execFileSync, spawn } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { request } from "node:http";
import { join } from "node:path";

const ENGINES = new Set(["cartesia", "openai", "elevenlabs", "local"]);

const state = {
  enabled: envBool("COPYSPEAK_CLAUDE_ENABLED", envBool("COPYSPEAK_PI_ENABLED", true)),
  engine: ENGINES.has(process.env.COPYSPEAK_CLAUDE_ENGINE)
    ? process.env.COPYSPEAK_CLAUDE_ENGINE
    : ENGINES.has(process.env.COPYSPEAK_PI_ENGINE)
      ? process.env.COPYSPEAK_PI_ENGINE
      : undefined,
  effect: process.env.COPYSPEAK_CLAUDE_EFFECT || process.env.COPYSPEAK_PI_EFFECT || undefined,
  maxChars: Number(process.env.COPYSPEAK_CLAUDE_MAX_CHARS || process.env.COPYSPEAK_PI_MAX_CHARS || 700),
  launchCopySpeak: envBool(
    "COPYSPEAK_CLAUDE_LAUNCH",
    envBool("COPYSPEAK_PI_LAUNCH", false)
  )
};

const hookInput = await readStdinJson();

if (!state.enabled) process.exit(0);
if (state.launchCopySpeak) launchCopySpeak();

const transcriptPath = hookInput?.transcript_path;
const text = cleanForSpeech(findLastAssistantText(transcriptPath)).slice(0, state.maxChars);
if (!text) process.exit(0);

try {
  await postSpeak(text);
} catch (error) {
  console.error(`CopySpeak Claude hook failed: ${String(error)}`);
  process.exit(0);
}

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
      const text = extractText(message);
      if (text) lastText = text;
    } catch {
      // Ignore malformed transcript lines. Claude owns this file format.
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

async function postSpeak(text) {
  const body = JSON.stringify({ text, engine: state.engine, effect: state.effect });
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
        res.on("data", (chunk) => {
          responseBody += chunk;
        });
        res.on("end", () => {
          if (res.statusCode && res.statusCode >= 200 && res.statusCode < 300) resolve();
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
    const output = execFileSync("tasklist.exe", ["/FI", "IMAGENAME eq copyspeak.exe", "/NH"], {
      encoding: "utf8",
      windowsHide: true
    });
    return /^copyspeak\.exe\s+/im.test(output);
  } catch {
    return false;
  }
}

function findBuiltCopySpeak() {
  const cwd = process.cwd();
  const candidates = [
    join(cwd, "src-tauri", "target", "release", "copyspeak.exe"),
    join(cwd, "src-tauri", "target", "debug", "copyspeak.exe")
  ];
  return candidates.find(existsSync);
}

function cleanForSpeech(text) {
  return text
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`[^`]*`/g, " ")
    .replace(/\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/https?:\/\/\S+/g, " link ")
    .replace(/[#*_>~|]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function envBool(name, fallback) {
  const value = process.env[name];
  if (value == null || value === "") return fallback;
  return !/^(0|false|no|off)$/i.test(value);
}
