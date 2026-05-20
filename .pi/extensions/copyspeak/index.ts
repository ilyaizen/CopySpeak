import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { execFileSync, spawn } from "node:child_process";
import { request } from "node:http";
import { existsSync } from "node:fs";
import { join } from "node:path";

type Engine = "cartesia" | "openai" | "elevenlabs" | "local";

type State = {
  enabled: boolean;
  engine: Engine | undefined;
  effect: string | undefined;
  speakAssistant: boolean;
  speakActivity: boolean;
  speakThinking: boolean;
  maxChars: number;
  launchCopySpeak: boolean;
};

const state: State = {
  enabled: envBool("COPYSPEAK_PI_ENABLED", true),
  engine: isEngine(process.env.COPYSPEAK_PI_ENGINE) ? process.env.COPYSPEAK_PI_ENGINE : undefined,
  effect: process.env.COPYSPEAK_PI_EFFECT || undefined,
  speakAssistant: envBool("COPYSPEAK_PI_ASSISTANT", true),
  speakActivity: envBool("COPYSPEAK_PI_ACTIVITY", false),
  speakThinking: envBool("COPYSPEAK_PI_THINKING", false),
  maxChars: Number(process.env.COPYSPEAK_PI_MAX_CHARS || 0),
  launchCopySpeak: envBool("COPYSPEAK_PI_LAUNCH", false)
};

let lastSpoken = "";
let lastSpokenAt = 0;
let speakQueue = Promise.resolve();
let clipboardFailureCount = 0;
let clipboardFailureNotified = false;
let spokenThinkingBlocks = new Set<string>();

export default function (pi: ExtensionAPI) {
  pi.on("session_start", async (_event, ctx) => {
    try {
      if (state.launchCopySpeak) launchCopySpeak();
      ctx.ui.setStatus("copyspeak", statusText());
      ctx.ui.notify(statusText(), "info");
    } catch (error) {
      ctx.ui.setStatus("copyspeak", "voice config failed");
      ctx.ui.notify(`CopySpeak voice setup failed: ${String(error)}`, "error");
    }
  });

  pi.on("agent_start", async (_event, ctx) => {
    spokenThinkingBlocks = new Set();
    if (state.enabled && state.speakActivity) await speakSafe("CopySpeak: agent thinking.", ctx);
  });

  pi.on("message_update", async (event, ctx) => {
    if (!state.enabled || !state.speakThinking) return;
    const streamEvent = (event as any).assistantMessageEvent;
    if (streamEvent?.type !== "thinking_end") return;

    const content =
      streamEvent.content || findThinkingContent((event as any).message, streamEvent.contentIndex);
    if (!content) return;

    const key = `${streamEvent.contentIndex ?? "unknown"}:${content}`;
    if (spokenThinkingBlocks.has(key)) return;
    spokenThinkingBlocks.add(key);

    await speakSafe(content, ctx);
  });

  pi.on("tool_execution_start", async (event) => {
    if (!state.enabled || !state.speakActivity) return;
    const name = (event as any).toolName || (event as any).name || "tool";
    await speakSafe(`Using ${name}.`);
  });

  pi.on("agent_end", async (event, ctx) => {
    if (!state.enabled || !state.speakAssistant) return;
    const message = [...((event as any).messages || [])]
      .reverse()
      .find((message) => message?.role === "assistant");
    const text = prepareText(extractText(message));
    if (text) await speakSafe(text, ctx);
  });

  pi.registerCommand("copyspeak", {
    description:
      "Control CopySpeak voice notifications: on/off/status/test/engine <cartesia|openai|elevenlabs|local>",
    handler: async (args, ctx) => {
      const [cmd, value] = args.trim().split(/\s+/);
      try {
        if (!cmd || cmd === "status") {
          ctx.ui.notify(statusText(), "info");
          return;
        }
        if (cmd === "on") state.enabled = true;
        else if (cmd === "off") state.enabled = false;
        else if (cmd === "test")
          await speakSafe(
            args.replace(/^test\s*/, "") ||
              "CopySpeak voice hook is online with walkie talkie effect.",
            ctx,
            true
          );
        else if (cmd === "engine") {
          if (!isEngine(value))
            throw new Error("engine must be cartesia, openai, elevenlabs, or local");
          state.engine = value;
        } else if (cmd === "activity") state.speakActivity = value !== "off";
        else if (cmd === "assistant") state.speakAssistant = value !== "off";
        else if (cmd === "thinking") state.speakThinking = value !== "off";
        else
          throw new Error(
            "usage: /copyspeak on|off|status|test [text]|engine <engine>|activity on|off|assistant on|off|thinking on|off"
          );
        ctx.ui.setStatus("copyspeak", statusText());
        ctx.ui.notify(statusText(), "info");
      } catch (error) {
        ctx.ui.notify(`CopySpeak voice: ${String(error)}`, "error");
      }
    }
  });
}

function statusText() {
  const power = state.enabled ? "on" : "off";
  const overrides = [
    state.speakAssistant === false ? "assistant off" : undefined,
    state.speakThinking === true ? "thinking on" : undefined,
    state.speakActivity === true ? "activity on" : undefined
  ].filter(Boolean);
  const detail = overrides.length ? ` (${overrides.join(", ")})` : "";
  return `copyspeak ${power}${detail}`;
}

async function speakSafe(text: string, ctx?: any, force = false) {
  const cleaned = prepareText(text);
  if (!cleaned) return;
  if (!force && shouldSkipDuplicate(cleaned)) return;

  speakQueue = speakQueue
    .catch(() => undefined)
    .then(async () => {
      const response = await speak(cleaned);
      showProcessedText(response, cleaned, ctx);
    })
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

async function speak(text: string): Promise<unknown> {
  const response = await postSpeak(text);
  clipboardFailureCount = 0;
  clipboardFailureNotified = false;
  return response;
}

function shouldSkipDuplicate(text: string) {
  const now = Date.now();
  if (text === lastSpoken && now - lastSpokenAt < 120000) return true;
  lastSpoken = text;
  lastSpokenAt = now;
  return false;
}

async function postSpeak(text: string): Promise<unknown> {
  const body = JSON.stringify({ text, engine: state.engine, effect: state.effect });
  const url = new URL(process.env.COPYSPEAK_CONTROL_URL || "http://127.0.0.1:43117/speak");

  return await new Promise<unknown>((resolve, reject) => {
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
        res.on("data", (chunk) => {
          responseBody += chunk;
        });
        res.on("end", () => {
          if (res.statusCode && res.statusCode >= 200 && res.statusCode < 300) {
            resolve(parseJson(responseBody));
          } else reject(new Error(`HTTP ${res.statusCode}: ${responseBody}`));
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

function findThinkingContent(message: any, contentIndex: number | undefined): string {
  const content = message?.content;
  if (!Array.isArray(content) || contentIndex == null) return "";
  const part = content[contentIndex];
  return part?.type === "thinking" ? part.thinking || part.text || "" : "";
}

function extractText(message: any): string {
  const content = message?.content;
  if (typeof content === "string") return content;
  if (!Array.isArray(content)) return "";

  return content
    .map((part) => {
      if (typeof part === "string") return part;
      if (part?.type === "thinking") {
        const thinking = part.thinking || part.text || "";
        if (!state.speakThinking || hasSpokenThinkingContent(thinking)) return "";
        return thinking;
      }
      if (part?.type === "text") return part.text || "";
      return part?.text || "";
    })
    .join("\n");
}

function hasSpokenThinkingContent(content: string): boolean {
  return [...spokenThinkingBlocks].some((entry) => entry.endsWith(`:${content}`));
}

function prepareText(text: string): string {
  const cleaned = cleanForSpeech(text);
  return state.maxChars > 0 ? truncateAtBoundary(cleaned, state.maxChars) : cleaned;
}

function cleanForSpeech(text: string): string {
  return text
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/https?:\/\/\S+/g, " link ")
    .replace(/[#*_>~|]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

function truncateAtBoundary(text: string, max: number): string {
  if (max <= 0 || text.length <= max) return text;
  const slice = text.slice(0, max);
  const boundary = Math.max(
    slice.lastIndexOf(". "),
    slice.lastIndexOf("! "),
    slice.lastIndexOf("? "),
    slice.lastIndexOf("\n")
  );
  return boundary > max * 0.5 ? slice.slice(0, boundary + 1) : slice;
}

function showProcessedText(response: unknown, originalText: string, ctx?: any) {
  const processedText = extractProcessedText(response);
  if (!processedText || processedText === originalText) return;

  ctx?.ui?.setWidget?.("copyspeak-processed", splitWidgetLines(processedText));
  ctx?.ui?.notify?.(`CopySpeak post-processed text:\n${processedText}`, "info");
}

function extractProcessedText(response: unknown): string {
  if (!response || typeof response !== "object") return "";
  const record = response as Record<string, unknown>;
  const value =
    record.post_processed_text ??
    record.postProcessedText ??
    record.processed_text ??
    record.processedText ??
    record.spoken_text ??
    record.spokenText ??
    record.text;
  return typeof value === "string" ? value.trim() : "";
}

function splitWidgetLines(text: string): string[] {
  const lines = text.split(/\r?\n/).flatMap((line) => {
    if (line.length <= 100) return [line];
    const chunks: string[] = [];
    for (let i = 0; i < line.length; i += 100) chunks.push(line.slice(i, i + 100));
    return chunks;
  });
  return ["CopySpeak post-processed:", ...lines.slice(0, 12)];
}

function parseJson(text: string): unknown {
  if (!text.trim()) return undefined;
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

function isEngine(value: string | undefined): value is Engine {
  return value === "cartesia" || value === "openai" || value === "elevenlabs" || value === "local";
}

function envBool(name: string, fallback: boolean) {
  const value = process.env[name];
  if (value == null || value === "") return fallback;
  return !/^(0|false|no|off)$/i.test(value);
}
