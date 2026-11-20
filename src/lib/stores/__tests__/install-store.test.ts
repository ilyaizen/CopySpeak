import { beforeEach, expect, it, vi } from "vite-plus/test";
import { installStore } from "../install-store.svelte";

const { listeners } = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>()
}));
vi.mock("#lib/services/tauri.js", () => ({ isTauri: true }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, callback: (event: { payload: unknown }) => void) => {
    listeners.set(name, callback);
    return () => listeners.delete(name);
  })
}));

type Progress = { engine: string; line?: string | null; done?: boolean; exit_code?: number | null };

function emitProgress(payload: Progress) {
  listeners.get("install-progress")?.({
    payload: { line: null, done: false, exit_code: null, ...payload }
  });
}

// The store is a singleton. Detach the listener before each test and use a
// unique engine id per test so run state never leaks between tests.
beforeEach(() => {
  installStore.destroy();
});

it("refuses a second start while the engine is running and keeps the first run", async () => {
  expect(await installStore.start("store-test-running", ["amy"])).toBe(true);
  emitProgress({ engine: "store-test-running", line: "[STEP] voice:amy" });

  expect(await installStore.start("store-test-running", ["joe"])).toBe(false);

  const run = installStore.run("store-test-running");
  expect(run?.running).toBe(true);
  expect(Object.keys(run?.voiceStatus ?? {})).toEqual(["amy"]);
  expect(run?.voiceStatus.amy).toBe("installing");
});

it("allows a new start once a run has settled", async () => {
  await installStore.start("store-test-settled", []);
  emitProgress({ engine: "store-test-settled", done: true, exit_code: 0 });

  expect(await installStore.start("store-test-settled", [])).toBe(true);
  expect(installStore.isRunning("store-test-settled")).toBe(true);
});

it("a done event with exit 0 clears running and marks voices done", async () => {
  await installStore.start("store-test-done", ["amy"]);
  emitProgress({ engine: "store-test-done", done: true, exit_code: 0 });

  const run = installStore.run("store-test-done");
  expect(run?.running).toBe(false);
  expect(run?.exitCode).toBe(0);
  expect(run?.voiceStatus.amy).toBe("done");
});

it("fail settles a launch error: clears running, sets exit -1, fails pending voices", async () => {
  await installStore.start("store-test-fail", ["amy", "joe"]);
  installStore.fail("store-test-fail", "[ERROR] install did not start: boom");

  const run = installStore.run("store-test-fail");
  expect(run?.running).toBe(false);
  expect(run?.exitCode).toBe(-1);
  expect(run?.lines).toEqual(["[ERROR] install did not start: boom"]);
  expect(run?.voiceStatus).toEqual({ amy: "failed", joe: "failed" });
});

it("a start after fail is allowed, so Retry is not blocked", async () => {
  await installStore.start("store-test-retry", []);
  installStore.fail("store-test-retry", "[ERROR] install did not start: boom");

  expect(await installStore.start("store-test-retry", [])).toBe(true);
  expect(installStore.isRunning("store-test-retry")).toBe(true);
});
