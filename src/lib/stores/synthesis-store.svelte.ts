import { writable } from "svelte/store";
import { isTauri } from "$lib/services/tauri.js";

export const synthesisStore = writable({
  isSynthesizing: false
});

export async function setupSynthesisListener() {
  if (!isTauri) return null;

  try {
    const { listen } = await import("@tauri-apps/api/event");
    return (
      (await listen) <
      boolean >
      ("synthesis-state-change",
      (event) => {
        synthesisStore.update((state) => ({
          ...state,
          isSynthesizing: event.payload
        }));
      })
    );
  } catch (error) {
    console.error("Failed to setup synthesis state listener", error);
    return null;
  }
}
