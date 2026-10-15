// History store refreshes on backend events; listeners are deduplicated and skip non-Tauri runs.

import type { UnlistenFn } from "@tauri-apps/api/event";
import { historyStore } from "#lib/stores/history-store.svelte";

// The registration in flight or done. Stop takes it before awaiting, so a stop
// that lands mid-start (HMR remounting the layout) still unlistens what that
// start registers, and the next start registers afresh.
let listeners: Promise<UnlistenFn[]> | null = null;

export async function startHistoryEventListeners(): Promise<void> {
  if (!("window" in globalThis) || !("__TAURI_INTERNALS__" in window)) {
    return;
  }

  listeners ??= (async () => {
    const unlisteners: UnlistenFn[] = [];
    try {
      const { listen } = await import("@tauri-apps/api/event");

      unlisteners.push(
        await listen<void>("history-updated", async () => {
          await historyStore.refresh();
        })
      );

      unlisteners.push(
        await listen<{ text: string }>("speak-request", async () => {
          setTimeout(async () => {
            await historyStore.refresh();
          }, 100);
        })
      );
    } catch (error) {
      console.error("[history-events] Failed to start event listeners:", error);
    }
    return unlisteners;
  })();
  await listeners;
}

export async function stopHistoryEventListeners(): Promise<void> {
  const pending = listeners;
  listeners = null;
  for (const unlisten of (await pending) ?? []) unlisten();
}

export async function refreshHistory(): Promise<void> {
  await historyStore.refresh();
}
