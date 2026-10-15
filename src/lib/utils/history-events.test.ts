import { expect, it, vi } from "vite-plus/test";
import { listen } from "@tauri-apps/api/event";
import { startHistoryEventListeners, stopHistoryEventListeners } from "./history-events";

const { live, ipc } = vi.hoisted(() => {
  let open = () => {};
  const done = new Promise<void>((resolve) => (open = resolve));
  return { live: new Map<string, Set<() => void>>(), ipc: { done, open: () => open() } };
});
vi.mock("#lib/stores/history-store.svelte", () => ({ historyStore: { refresh: vi.fn() } }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, callback: () => void) => {
    await ipc.done;
    const set = live.get(name) ?? new Set();
    live.set(name, set.add(callback));
    return () => set.delete(callback);
  })
}));

it("keeps one listener per event when HMR stops and restarts it mid-start", async () => {
  Object.assign(window, { __TAURI_INTERNALS__: {} });

  // HMR destroys the layout while its start awaits `listen`, then mounts a new one.
  const first = startHistoryEventListeners();
  await vi.waitFor(() => expect(listen).toHaveBeenCalled());
  const stop = stopHistoryEventListeners();
  const second = startHistoryEventListeners();
  ipc.open();
  await Promise.all([first, stop, second]);

  expect(live.get("speak-request")?.size).toBe(1);
  expect(live.get("history-updated")?.size).toBe(1);

  await stopHistoryEventListeners();
  expect(live.get("speak-request")?.size).toBe(0);
  expect(live.get("history-updated")?.size).toBe(0);
});
