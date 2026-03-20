// @see docs_internal/event-system.md

import type { UnlistenFn } from "@tauri-apps/api/event";
import { historyStore } from "$lib/stores/history-store.svelte";

let unlistenHistory: UnlistenFn | null = null;
let unlistenSpeak: UnlistenFn | null = null;
let isListening = false;

export async function startHistoryEventListeners(): Promise<void> {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
    return;
  }

  if (isListening) {
    return;
  }

  try {
    const { listen } = await import("@tauri-apps/api/event");

    unlistenHistory = await listen<void>("history-updated", async () => {
      await historyStore.refresh();
    });

    unlistenSpeak = await listen<{ text: string }>("speak-request", async () => {
      setTimeout(async () => {
        await historyStore.refresh();
      }, 100);
    });

    isListening = true;
  } catch (error) {
    console.error("[history-events] Failed to start event listeners:", error);
  }
}

export async function stopHistoryEventListeners(): Promise<void> {
  if (unlistenHistory) {
    unlistenHistory();
    unlistenHistory = null;
  }
  if (unlistenSpeak) {
    unlistenSpeak();
    unlistenSpeak = null;
  }
  isListening = false;
}

export function isListeningForHistoryEvents(): boolean {
  return isListening;
}

export async function refreshHistory(): Promise<void> {
  await historyStore.refresh();
}
