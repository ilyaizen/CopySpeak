#!/usr/bin/env node

import { writeFile } from "node:fs/promises";

const base = process.env.COPYSPEAK_CONTROL_ADDR?.startsWith("http")
  ? process.env.COPYSPEAK_CONTROL_ADDR
  : `http://${process.env.COPYSPEAK_CONTROL_ADDR || "127.0.0.1:43117"}`;

function die(message, code = 1) {
  console.error(message);
  process.exit(code);
}

async function request(path, options = {}) {
  let response;
  try {
    response = await fetch(`${base}${path}`, {
      ...options,
      headers: { "Content-Type": "application/json", ...options.headers }
    });
  } catch {
    die(`CopySpeak control server is not reachable at ${base}`);
  }
  const text = await response.text();
  if (!response.ok) {
    die(text || `HTTP ${response.status}`);
  }
  return text ? JSON.parse(text) : null;
}

function argValue(args, names) {
  for (let i = 0; i < args.length; i++) {
    if (names.includes(args[i])) return args[i + 1];
  }
  return undefined;
}

function has(args, names) {
  return args.some((arg) => names.includes(arg));
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) chunks.push(chunk);
  return Buffer.concat(chunks).toString("utf8");
}

function printProfiles(profiles) {
  for (const profile of profiles) {
    const active = profile.active ? "*" : " ";
    console.log(
      `${active} ${profile.id}\t${profile.name}\t${profile.engine}\t${profile.voice || ""}`
    );
  }
}

const [cmd, subcmd, ...rest] = process.argv.slice(2);

if (!cmd || has([cmd], ["-h", "--help"])) {
  console.log(`copyspeak health
copyspeak speak --profile pi "hello"
copyspeak speak -p pi --stdin [--wait] [--ack <path>]
copyspeak profiles list|use <id>|show <id>
copyspeak engines list
copyspeak voices list --engine elevenlabs`);
  process.exit(0);
}

if (cmd === "health") {
  console.log(JSON.stringify(await request("/health"), null, 2));
} else if (cmd === "speak") {
  const args = [subcmd, ...rest].filter(Boolean);
  const profile = argValue(args, ["--profile", "-p"]);
  const stdin = has(args, ["--stdin"]);
  const persist = has(args, ["--persist", "--set-active"]);
  const wait = has(args, ["--wait"]);
  const ack = argValue(args, ["--ack"]);
  const text = stdin
    ? await readStdin()
    : args
        .filter((arg, index) => {
          const previous = args[index - 1];
          return (
            !["--profile", "-p", "--ack"].includes(previous) &&
            !arg.startsWith("--") &&
            arg !== profile
          );
        })
        .join(" ");
  if (!text.trim()) die("text is required");
  await request("/speak", {
    method: "POST",
    body: JSON.stringify({
      text,
      profile,
      persist_selection: persist || undefined,
      wait: wait || undefined
    })
  });
  if (ack) {
    // Hermes' command-TTS contract needs a non-empty audio file even though
    // CopySpeak already spoke; write a tiny valid silent WAV (~10 ms).
    const silence = Buffer.alloc(320); // 100 samples, 16 kHz mono 16-bit
    const header = Buffer.alloc(44);
    header.write("RIFF", 0, "ascii");
    header.writeUInt32LE(36 + silence.length, 4);
    header.write("WAVE", 8, "ascii");
    header.write("fmt ", 12, "ascii");
    header.writeUInt32LE(16, 16);
    header.writeUInt16LE(1, 20); // PCM
    header.writeUInt16LE(1, 22); // mono
    header.writeUInt32LE(16000, 24);
    header.writeUInt32LE(32000, 28); // byte rate
    header.writeUInt16LE(2, 32); // block align
    header.writeUInt16LE(16, 34); // bits per sample
    header.write("data", 36, "ascii");
    header.writeUInt32LE(silence.length, 40);
    await writeFile(ack, Buffer.concat([header, silence]));
  }
  console.log("ok");
} else if (cmd === "profiles") {
  if (subcmd === "list") {
    printProfiles(await request("/profiles"));
  } else if (subcmd === "use") {
    const profile = rest[0];
    if (!profile) die("profile id is required");
    await request("/profiles/active", { method: "POST", body: JSON.stringify({ profile }) });
    console.log("ok");
  } else if (subcmd === "show") {
    const profile = rest[0];
    if (!profile) die("profile id is required");
    console.log(JSON.stringify(await request(`/profiles/${encodeURIComponent(profile)}`), null, 2));
  } else {
    die("expected profiles list|use|show");
  }
} else if (cmd === "engines" && subcmd === "list") {
  console.log(JSON.stringify(await request("/engines"), null, 2));
} else if (cmd === "voices" && subcmd === "list") {
  const engine = argValue(rest, ["--engine", "-e"]);
  if (!engine) die("--engine is required");
  console.log(
    JSON.stringify(await request(`/engines/${encodeURIComponent(engine)}/voices`), null, 2)
  );
} else {
  die("unknown command");
}
