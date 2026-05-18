import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { execFileSync, spawn } from "node:child_process";
import { request } from "node:http";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

type Engine = "cartesia" | "openai" | "elevenlabs" | "local";

type State = {
  enabled: boolean;
  engine: Engine;
  speakAssistant: boolean;
  speakActivity: boolean;
  maxChars: number;
  launchCopySpeak: boolean;
};

const state: State = {
  enabled: envBool("COPYSPEAK_PI_ENABLED", true),
  engine: (process.env.COPYSPEAK_PI_ENGINE as Engine) || "cartesia",
  speakAssistant: envBool("COPYSPEAK_PI_ASSISTANT", true),
  speakActivity: envBool("COPYSPEAK_PI_ACTIVITY", false),
  maxChars: Number(process.env.COPYSPEAK_PI_MAX_CHARS || 700),
  launchCopySpeak: envBool("COPYSPEAK_PI_LAUNCH", true)
};

let lastSpoken = "";
let lastSpokenAt = 0;
let speakQueue = Promise.resolve();
let clipboardFailureCount = 0;
let clipboardFailureNotified = false;

export default function (pi: ExtensionAPI) {
  pi.on("session_start", async (_event, ctx) => {
    try {
      configureCopySpeak(state.engine);
      if (state.launchCopySpeak) launchCopySpeak();
      ctx.ui.setStatus("copyspeak", statusText());
      ctx.ui.notify(`CopySpeak voice ${state.enabled ? "enabled" : "disabled"} (${state.engine}, walkie-talkie)`, "info");
    } catch (error) {
      ctx.ui.setStatus("copyspeak", "voice config failed");
      ctx.ui.notify(`CopySpeak voice setup failed: ${String(error)}`, "error");
    }
  });

  pi.on("agent_start", async (_event, ctx) => {
    if (state.enabled && state.speakActivity) await speakSafe("CopySpeak: agent thinking.", ctx);
  });

  pi.on("tool_execution_start", async (event) => {
    if (!state.enabled || !state.speakActivity) return;
    const name = (event as any).toolName || (event as any).name || "tool";
    await speakSafe(`Using ${name}.`);
  });

  pi.on("message_end", async (event, ctx) => {
    if (!state.enabled || !state.speakAssistant) return;
    if ((event as any).message?.role !== "assistant") return;
    const text = cleanForSpeech(extractText((event as any).message)).slice(0, state.maxChars);
    if (text) await speakSafe(text, ctx);
  });

  pi.registerCommand("copyspeak-voice", {
    description: "Control CopySpeak voice notifications: on/off/status/test/engine <cartesia|openai|elevenlabs|local>",
    handler: async (args, ctx) => {
      const [cmd, value] = args.trim().split(/\s+/);
      try {
        if (!cmd || cmd === "status") {
          ctx.ui.notify(statusText(), "info");
          return;
        }
        if (cmd === "on") state.enabled = true;
        else if (cmd === "off") state.enabled = false;
        else if (cmd === "test") await speakSafe(args.replace(/^test\s*/, "") || "CopySpeak voice hook is online with walkie talkie effect.", ctx, true);
        else if (cmd === "engine") {
          if (!isEngine(value)) throw new Error("engine must be cartesia, openai, elevenlabs, or local");
          state.engine = value;
          configureCopySpeak(state.engine);
        } else if (cmd === "activity") state.speakActivity = value !== "off";
        else if (cmd === "assistant") state.speakAssistant = value !== "off";
        else throw new Error("usage: /copyspeak-voice on|off|status|test [text]|engine <engine>|activity on|off|assistant on|off");
        ctx.ui.setStatus("copyspeak", statusText());
        ctx.ui.notify(statusText(), "info");
      } catch (error) {
        ctx.ui.notify(`CopySpeak voice: ${String(error)}`, "error");
      }
    }
  });
}

function statusText() {
  return `${state.enabled ? "voice on" : "voice off"} · ${state.engine} · walkie`;
}

async function speakSafe(text: string, ctx?: any, force = false) {
  speakQueue = speakQueue
    .catch(() => undefined)
    .then(() => speak(text, force))
    .catch((error) => {
      clipboardFailureCount++;
      ctx?.ui?.setStatus?.("copyspeak", "voice failed");
      if (!clipboardFailureNotified) {
        clipboardFailureNotified = true;
        ctx?.ui?.notify?.(`CopySpeak voice failed; voice disabled: ${String(error)}`, "error");
      }
      if (clipboardFailureCount >= 2) state.enabled = false;
    });
  await speakQueue;
}

async function speak(text: string, force = false) {
  const cleaned = cleanForSpeech(text);
  if (!cleaned) return;
  const now = Date.now();
  if (!force && cleaned === lastSpoken && now - lastSpokenAt < 5000) return;
  lastSpoken = cleaned;
  lastSpokenAt = now;

  await postSpeak(cleaned);
  clipboardFailureCount = 0;
  clipboardFailureNotified = false;
}

async function postSpeak(text: string) {
  const body = JSON.stringify({ text, engine: state.engine, effect: "walkie_talkie" });
  const url = new URL(process.env.COPYSPEAK_CONTROL_URL || "http://127.0.0.1:43117/speak");

  await new Promise<void>((resolve, reject) => {
    const req = request(
      {
        method: "POST",
        hostname: url.hostname,
        port: url.port,
        path: url.pathname,
        headers: {
          "content-type": "application/json",
          "content-length": Buffer.byteLength(body)
        }
      },
      (res) => {
        let responseBody = "";
        res.setEncoding("utf8");
        res.on("data", (chunk) => { responseBody += chunk; });
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

function configureCopySpeak(engine: Engine) {
  const path = configPath();
  const cfg = loadConfig(path);
  cfg.trigger ??= {};
  cfg.trigger.listen_enabled = true;
  cfg.trigger.double_copy_window_ms ??= 1500;
  cfg.trigger.max_text_length ??= 100000;

  cfg.effects ??= {};
  cfg.effects.enabled = true;
  cfg.effects.active_effect = "walkie_talkie";

  cfg.tts ??= {};
  cfg.tts.active_backend = engine;
  cfg.tts.cartesia ??= {};
  cfg.tts.openai ??= {};
  cfg.tts.elevenlabs ??= {};

  if (process.env.CARTESIA_API_KEY) cfg.tts.cartesia.api_key = process.env.CARTESIA_API_KEY;
  if (process.env.OPENAI_API_KEY) cfg.tts.openai.api_key = process.env.OPENAI_API_KEY;
  if (process.env.ELEVENLABS_API_KEY) cfg.tts.elevenlabs.api_key = process.env.ELEVENLABS_API_KEY;

  cfg.tts.cartesia.model_id ??= "sonic-3.5";
  cfg.tts.cartesia.voice_id ??= "f786b574-daa5-4673-aa0c-cbe3e8534c02";
  cfg.tts.cartesia.voice_name ??= "Katie";
  cfg.tts.cartesia.output_format ??= "wav";
  cfg.tts.openai.model ??= "tts-1";
  cfg.tts.openai.voice ??= "alloy";
  cfg.tts.elevenlabs.voice_id ??= "21m00Tcm4TlvDq8ikWAM";
  cfg.tts.elevenlabs.voice_name ??= "Rachel";
  cfg.tts.elevenlabs.model_id ??= "eleven_turbo_v2_5";

  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, JSON.stringify(cfg, null, 2));
}

function configPath() {
  const base = process.env.APPDATA || join(process.env.USERPROFILE || process.env.HOME || ".", "AppData", "Roaming");
  return join(base, "CopySpeak", "config.json");
}

function loadConfig(path: string): any {
  if (!existsSync(path)) return defaultConfig();
  return JSON.parse(readFileSync(path, "utf8"));
}

function defaultConfig(): any {
  return {
    version: "0.1.1",
    general: { start_with_windows: false, start_minimized: true, debug_mode: false, close_behavior: "minimize_to_tray", appearance: "system", update_checks_enabled: true, locale: "en" },
    trigger: { listen_enabled: true, double_copy_window_ms: 1500, max_text_length: 100000 },
    tts: { active_backend: "cartesia", preset: "kitten-tts", command: "py", args_template: ["-3.12", "{home_dir}/kittentts/kittentts-cli.py", "--text", "{raw_text}", "--voice", "{voice}", "--output", "{output}"], voice: "Rosie", openai: {}, elevenlabs: {}, cartesia: {} },
    playback: {}, hud: {}, output: {}, sanitization: {}, pagination: {}, history: {}, hotkey: {},
    effects: { enabled: true, active_effect: "walkie_talkie" }
  };
}

function extractText(message: any): string {
  const content = message?.content;
  if (typeof content === "string") return content;
  if (Array.isArray(content)) return content.map((p) => typeof p === "string" ? p : (p?.text || "")).join("\n");
  return "";
}

function cleanForSpeech(text: string): string {
  return text
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`[^`]*`/g, " ")
    .replace(/\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/https?:\/\/\S+/g, " link ")
    .replace(/[#*_>~|]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function isEngine(value: string | undefined): value is Engine {
  return value === "cartesia" || value === "openai" || value === "elevenlabs" || value === "local";
}

function envBool(name: string, fallback: boolean) {
  const value = process.env[name];
  if (value == null || value === "") return fallback;
  return !/^(0|false|no|off)$/i.test(value);
}

