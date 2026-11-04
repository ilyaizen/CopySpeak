// @vitest-environment node

import { execFile } from "node:child_process";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { expect, test } from "vite-plus/test";

const run = promisify(execFile);
const cli = fileURLToPath(new URL("../scripts/copyspeak.mjs", import.meta.url));

type Captured = { method?: string; path?: string; body: string };

/** Run the CLI against a loopback stand-in for the control server. */
async function runCli(args: string[], reply = "{}") {
  const requests: Captured[] = [];
  const server = createServer(async (request, response) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    requests.push({
      method: request.method,
      path: request.url,
      body: Buffer.concat(chunks).toString("utf8")
    });
    response.writeHead(200, { "Content-Type": "application/json" });
    response.end(reply);
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    // SAFETY: listen(0, "127.0.0.1") has completed; this is a bound TCP server, not a pipe.
    const { port } = server.address() as AddressInfo;
    const { stdout } = await run(process.execPath, [cli, ...args], {
      env: { ...process.env, COPYSPEAK_CONTROL_ADDR: `127.0.0.1:${port}` },
      timeout: 5000
    });
    return { stdout: stdout.trim(), requests };
  } finally {
    await new Promise<void>((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
    });
  }
}

async function speakRequest(...flags: string[]) {
  const { stdout, requests } = await runCli(["speak", "hello", ...flags]);
  expect(stdout).toBe("ok");
  expect(requests).toHaveLength(1);
  expect(requests[0].method).toBe("POST");
  expect(requests[0].path).toBe("/speak");
  return JSON.parse(requests[0].body);
}

test("--post-process sends the canonical request field", async () => {
  expect(await speakRequest("--post-process")).toEqual({ text: "hello", post_process: true });
});

test("--condense remains an alias for --post-process", async () => {
  expect(await speakRequest("--condense")).toEqual({ text: "hello", post_process: true });
});

test("both spellings together send one canonical request field", async () => {
  expect(await speakRequest("--post-process", "--condense")).toEqual({
    text: "hello",
    post_process: true
  });
});

test("omitting both flags leaves post-processing unspecified", async () => {
  expect(await speakRequest()).toEqual({ text: "hello" });
});

test("help documents the current flag and legacy alias", async () => {
  const { stdout } = await run(process.execPath, [cli, "--help"], { timeout: 5000 });
  expect(stdout).toContain("[--post-process]");
  expect(stdout).toContain("--condense is a backward-compatible alias for --post-process.");
});

test("--request-id names the speak without leaking into its text", async () => {
  expect(await speakRequest("--request-id", "hermes-abc123", "--wait")).toEqual({
    text: "hello",
    wait: true,
    request_id: "hermes-abc123"
  });
});

test("stop --request-id sends a scoped stop body", async () => {
  const { requests } = await runCli(["stop", "--request-id", "hermes-abc123"]);
  expect(requests).toHaveLength(1);
  expect(requests[0].method).toBe("POST");
  expect(requests[0].path).toBe("/stop");
  expect(JSON.parse(requests[0].body)).toEqual({ request_id: "hermes-abc123" });
});

test("stop without --request-id stays the bodyless global stop", async () => {
  const { requests } = await runCli(["stop"]);
  expect(requests).toHaveLength(1);
  expect(requests[0].path).toBe("/stop");
  expect(requests[0].body).toBe("");
});

test("a cancelled speak still writes the ack and reports cancelled", async () => {
  const dir = await mkdtemp(join(tmpdir(), "copyspeak-cli-"));
  try {
    const ack = join(dir, "ack.wav");
    const { stdout } = await runCli(
      ["speak", "hello", "--wait", "--request-id", "r1", "--ack", ack],
      JSON.stringify({ ok: true, cancelled: true })
    );
    expect(stdout).toBe("cancelled");
    const wav = await readFile(ack);
    expect(wav.subarray(0, 4).toString("ascii")).toBe("RIFF");
    expect(wav.length).toBe(364);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test("help documents --request-id on speak and stop", async () => {
  const { stdout } = await run(process.execPath, [cli, "--help"], { timeout: 5000 });
  expect(stdout).toContain("copyspeak stop [--request-id <id>]");
  expect(stdout).toContain("[--request-id <id>]");
});
