// @vitest-environment node

import { execFile } from "node:child_process";
import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { expect, test } from "vite-plus/test";

const run = promisify(execFile);
const cli = fileURLToPath(new URL("../scripts/copyspeak.mjs", import.meta.url));

async function speakRequest(...flags: string[]) {
  const requests: { method?: string; path?: string; body: string }[] = [];
  const server = createServer(async (request, response) => {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    requests.push({
      method: request.method,
      path: request.url,
      body: Buffer.concat(chunks).toString("utf8")
    });
    response.writeHead(200, { "Content-Type": "application/json" });
    response.end("{}");
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    // SAFETY: listen(0, "127.0.0.1") has completed; this is a bound TCP server, not a pipe.
    const { port } = server.address() as AddressInfo;
    const { stdout } = await run(process.execPath, [cli, "speak", "hello", ...flags], {
      env: { ...process.env, COPYSPEAK_CONTROL_ADDR: `127.0.0.1:${port}` },
      timeout: 5000
    });
    expect(stdout.trim()).toBe("ok");
    expect(requests).toHaveLength(1);
    expect(requests[0].method).toBe("POST");
    expect(requests[0].path).toBe("/speak");
    return JSON.parse(requests[0].body);
  } finally {
    await new Promise<void>((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
    });
  }
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
