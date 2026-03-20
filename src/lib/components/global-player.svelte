<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { cn } from "$lib/utils.js";
  import { playbackStore } from "$lib/stores/playback-store.svelte";
  import { isTauri } from "$lib/services/tauri.js";

  let audioEl = $state<HTMLAudioElement | null>(null);
  let clipboardCopied = $state(false);
  let clipboardTimer: ReturnType<typeof setTimeout> | null = null;

  let isPlaying = $derived(playbackStore.isPlaying);
  let isPaused = $derived(playbackStore.isPaused);
  let isSynthesizing = $derived(playbackStore.isSynthesizing);

  let infoBubble = $derived(isPlaying || isSynthesizing || clipboardCopied);

  let unlistenClipboard: (() => void) | null = null;

  onMount(async () => {
    playbackStore.setAudioElement(audioEl);
    await playbackStore.setupListeners();

    if (isTauri) {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        unlistenClipboard = await listen("clipboard-change", () => {
          clipboardCopied = true;
          if (clipboardTimer) clearTimeout(clipboardTimer);
          clipboardTimer = setTimeout(() => {
            clipboardCopied = false;
          }, 2000);
        });
      } catch {
        // Ignore
      }
    }
  });

  onDestroy(() => {
    playbackStore.teardownListeners();
    playbackStore.setAudioElement(null);
    if (unlistenClipboard) unlistenClipboard();
    if (clipboardTimer) clearTimeout(clipboardTimer);
  });
</script>

<!-- svelte-ignore a11y_media_has_caption -->
<audio bind:this={audioEl} style="display:none"></audio>

{#if infoBubble}
  <div
    class={cn(
      "fixed right-4 bottom-14 z-50 flex items-center gap-2 rounded-full px-4 py-2 shadow-lg transition-colors",
      isPlaying || isSynthesizing
        ? "bg-primary text-primary-foreground"
        : "bg-secondary text-secondary-foreground"
    )}
  >
    <div
      class={cn(
        "h-2 w-2 rounded-full",
        isPlaying || isSynthesizing
          ? cn("bg-primary-foreground", (isSynthesizing || !isPaused) && "animate-pulse")
          : "bg-muted-foreground"
      )}
    ></div>
    <span class="text-sm font-medium">
      {#if isSynthesizing && !isPlaying}
        Processing...
      {:else if isPlaying}
        {isPaused ? "Paused" : "Playing"}
      {:else}
        Copied to Clipboard
      {/if}
    </span>
  </div>
{/if}
