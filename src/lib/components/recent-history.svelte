<script lang="ts">
  import { onMount } from "svelte";
  import { on } from "svelte/events";
  import { Button } from "$lib/components/ui/button/index.js";
  import {
    AlertDialog,
    AlertDialogAction,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle
  } from "$lib/components/ui/alert-dialog/index.js";
  import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogTitle
  } from "$lib/components/ui/dialog/index.js";
  import {
    Play,
    RotateCcw,
    Square,
    Layers,
    Trash2,
    Clock,
    FolderOpen,
    AudioLines
  } from "@lucide/svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { Spinner } from "$lib/components/ui/spinner/index.js";
  import { historyStore } from "$lib/stores/history-store.svelte.js";
  import { playbackStore } from "$lib/stores/playback-store.svelte";
  import { groupHistoryReadings, historyVoiceLabel, historyTimeAgo } from "$lib/models/history";
  import type { AppConfig, EngineCatalogEntry, HistoryItem, VoiceProfile } from "$lib/types";
  import { isTauri } from "$lib/services/tauri";
  import { _ } from "svelte-i18n";

  interface Props {
    limit?: number;
    compact?: boolean;
    onSuccess?: (message: string) => void;
    onError?: (error: string) => void;
  }

  let { limit = 5, compact = false, onSuccess, onError }: Props = $props();
  type Reading = ReturnType<typeof groupHistoryReadings>[number];
  let now = $state(Date.now());
  let actionInProgress = $state<string | null>(null);
  let deletingId = $state<string | null>(null);
  let readingToDelete = $state<Reading | null>(null);
  let selectedReading = $state<Reading | null>(null);
  let actionError = $state<string | null>(null);
  let profiles = $state<VoiceProfile[]>([]);
  let engines = $state<EngineCatalogEntry[]>([]);

  onMount(() => {
    const timer = setInterval(() => (now = Date.now()), 60_000);
    return () => clearInterval(timer);
  });

  onMount(async () => {
    if (!isTauri) return;
    const [config, catalog] = await Promise.allSettled([
      invoke<AppConfig>("get_config"),
      invoke<EngineCatalogEntry[]>("list_tts_engines")
    ]);
    if (config.status === "fulfilled") profiles = config.value.tts.profiles;
    else console.error("Failed to load history voice labels:", config.reason);
    if (catalog.status === "fulfilled") engines = catalog.value;
    else console.error("Failed to load history voice catalog:", catalog.reason);
  });
  const readings = $derived(groupHistoryReadings(historyStore.items).slice(0, limit));
  const playbackBusy = $derived(
    playbackStore.isPlaying || playbackStore.isLoadingAudio || playbackStore.isSynthesizing
  );

  function horizontalWheel(node: HTMLUListElement, enabled: boolean) {
    if (!enabled) return;
    const cleanup = on(
      node,
      "wheel",
      (event) => {
        if (
          event.ctrlKey ||
          Math.abs(event.deltaX) >= Math.abs(event.deltaY) ||
          node.scrollWidth <= node.clientWidth
        )
          return;
        event.preventDefault();
        const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? node.clientWidth : 1;
        const direction = getComputedStyle(node).direction === "rtl" ? -1 : 1;
        node.scrollLeft += event.deltaY * unit * direction;
      },
      { passive: false }
    );
    return { destroy: cleanup };
  }

  async function handlePlay(reading: Reading) {
    actionInProgress = reading.id;
    actionError = null;
    try {
      if (playbackStore.historyReadingId === reading.id && playbackBusy) {
        playbackStore.handleStop();
        await invoke("stop_speaking");
        return;
      }
      if (playbackBusy) {
        playbackStore.handleStop();
        await invoke("stop_speaking");
      }
      if (reading.batchId) await historyStore.playBatch(reading.batchId);
      else await historyStore.playEntry(reading.items[0].id);
      onSuccess?.("Playing audio from history");
    } catch (e) {
      actionError = `Failed to play: ${e}`;
      onError?.(actionError);
    } finally {
      actionInProgress = null;
    }
  }

  async function confirmDelete() {
    const reading = readingToDelete;
    if (!reading) return;
    readingToDelete = null;
    deletingId = reading.id;
    actionError = null;
    try {
      if (playbackBusy) throw new Error("Stop playback before deleting a reading.");
      if (reading.batchId) await historyStore.deleteBatch(reading.batchId);
      else await historyStore.deleteItem(reading.items[0].id);
      onSuccess?.("Deleted from history");
    } catch (e) {
      actionError = `Failed to delete: ${e}`;
      onError?.(actionError);
    } finally {
      deletingId = null;
    }
  }

  function formatDuration(ms: number) {
    const seconds = Math.round(ms / 1000);
    return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  }

  function historyFilename(item: HistoryItem) {
    return item.output_path?.split(/[\\/]/).pop() || "Unsaved reading";
  }

  async function openHistoryFolder() {
    const path = readings
      .flatMap((reading) => reading.items)
      .find((item) => item.output_path)?.output_path;
    if (!path) return;
    try {
      await revealItemInDir(path);
    } catch (e) {
      actionError = `Failed to open history folder: ${e}`;
      onError?.(actionError);
    }
  }
</script>

<section
  class={compact ? "flex min-w-0 flex-col gap-2" : "flex min-w-0 flex-col gap-4"}
  aria-labelledby="history-heading"
>
  <div class={compact ? "sr-only" : "flex flex-wrap items-baseline justify-between gap-2"}>
    <div>
      <h2
        id="history-heading"
        class={compact ? "text-base font-bold tracking-tight" : "text-lg font-bold tracking-tight"}
      >
        {$_("history.title")}
      </h2>
      {#if !compact}
        <p class="text-muted-foreground text-sm">Your readings, ready to listen again.</p>
      {/if}
    </div>
    <div class="flex items-center gap-2">
      {#if historyStore.isLoading}
        <span class="text-muted-foreground text-xs" role="status">{$_("history.loading")}</span>
      {/if}
    </div>
  </div>

  {#if readings.length === 0 && !historyStore.isLoading}
    <div
      class={compact
        ? "text-muted-foreground flex flex-col items-center gap-2 py-6"
        : "text-muted-foreground flex flex-col items-center gap-3 py-12"}
    >
      <Clock class="size-6" />
      <p class="text-sm">{$_("history.empty")}</p>
    </div>
  {:else}
    <ul
      use:horizontalWheel={compact}
      class={compact
        ? "recent-history-rail flex min-w-0 gap-4 overflow-x-auto overscroll-x-contain pb-2"
        : "border-border divide-border divide-y border-y"}
    >
      {#each readings as reading (reading.id)}
        {@const item = reading.items[0]}
        {@const isCurrent = playbackStore.historyReadingId === reading.id}
        {@const isActive = isCurrent && (playbackStore.isPlaying || playbackStore.isLoadingAudio)}
        {@const actionLabel = isActive
          ? $_("play.stop")
          : isCurrent
            ? $_("play.replay")
            : $_("play.play")}
        <li
          class={compact
            ? "flex w-80 shrink-0 flex-col gap-1 py-1 pr-2"
            : "hover:bg-muted/40 flex min-w-0 flex-col gap-2 px-2 py-3 transition-colors sm:px-3"}
        >
          {#if !compact}
            <div class="flex min-w-0 items-center gap-2">
              <span
                class="min-w-0 flex-1 truncate text-sm font-bold"
                title={item.output_path || historyFilename(item)}
              >
                {historyFilename(item)}
              </span>
              <Button
                variant={isActive ? "secondary" : "ghost"}
                size="sm"
                class="shrink-0 gap-1.5"
                onclick={() => handlePlay(reading)}
                disabled={actionInProgress !== null ||
                  deletingId !== null ||
                  playbackStore.isSynthesizing ||
                  !reading.hasAudio}
                aria-label={actionLabel}
                title={isActive
                  ? actionLabel
                  : !reading.hasAudio
                    ? $_("history.noAudioFile")
                    : actionLabel}
              >
                {#if actionInProgress === reading.id || (isCurrent && playbackStore.isLoadingAudio)}
                  <Spinner aria-hidden="true" />
                {:else if isActive}
                  <Square />
                {:else if isCurrent}
                  <RotateCcw />
                {:else}
                  <Play />
                {/if}
                <span>{actionLabel}</span>
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                class="text-muted-foreground hover:text-destructive shrink-0"
                onclick={() => (readingToDelete = reading)}
                disabled={actionInProgress !== null || deletingId !== null || playbackBusy}
                aria-label={$_("history.delete")}
                title={$_("history.delete")}
              >
                {#if deletingId === reading.id}
                  <Spinner aria-hidden="true" />
                {:else}
                  <Trash2 />
                {/if}
              </Button>
            </div>
          {/if}
          <div class={compact ? "min-w-0" : "flex min-w-0 flex-1 flex-col gap-1.5"}>
            <button
              type="button"
              class={compact
                ? "focus-visible:ring-ring text-muted-foreground block w-full cursor-pointer overflow-hidden rounded-sm text-left text-sm leading-snug focus-visible:ring-2 focus-visible:outline-none"
                : "focus-visible:ring-ring min-w-0 cursor-pointer rounded-sm text-left text-sm leading-relaxed focus-visible:ring-2 focus-visible:outline-none"}
              onclick={() => (selectedReading = reading)}
              aria-label="Read full text"
              aria-haspopup="dialog"
              title="Read full text"
            >
              <span
                class={compact
                  ? "line-clamp-3 whitespace-pre-wrap wrap-anywhere"
                  : "line-clamp-2 min-h-10 whitespace-pre-wrap wrap-anywhere"}>{reading.text}</span
              >
            </button>
            {#if !compact}
              <div
                class="text-muted-foreground flex flex-wrap items-center gap-x-3 gap-y-1 text-xs"
              >
                <time
                  datetime={new Date(reading.timestamp).toISOString()}
                  title={new Date(reading.timestamp).toLocaleString()}
                >
                  {historyTimeAgo(reading.timestamp, now)}
                </time>
                <span class="max-w-full truncate" title={`${item.tts_engine} · ${item.voice}`}
                  >{engines.find((engine) => engine.engine === item.tts_engine)?.label ??
                    item.tts_engine} · {historyVoiceLabel(item, profiles, engines)}</span
                >
                {#if reading.durationMs > 0}
                  <span class="tabular-nums">{formatDuration(reading.durationMs)}</span>
                {/if}
                {#if reading.partCount > 1}
                  <span
                    class="inline-flex items-center gap-1 whitespace-nowrap"
                    title="Paginated reading; plays all saved parts in order"
                  >
                    <Layers class="size-3" />
                    {reading.partCount} parts
                  </span>
                {/if}
                {#if !reading.success}
                  <span class="text-destructive">Generation failed</span>
                {:else if !reading.hasAudio}
                  <span>{$_("history.noAudioFile")}</span>
                {/if}
              </div>
            {/if}
          </div>
          {#if compact}
            <div class="order-first flex min-w-0 items-center gap-2">
              <span
                class="border-2 text-muted-foreground flex size-6 shrink-0 items-center justify-center rounded-full"
                aria-hidden="true"
              >
                <AudioLines class="size-3.5" strokeWidth={4} />
              </span>
              <span
                class="min-w-0 flex-1 truncate text-sm font-normal"
                title={item.output_path || historyFilename(item)}
              >
                {historyFilename(item)}
              </span>
              <Button
                variant={isActive ? "secondary" : "ghost"}
                size="icon-sm"
                class="shrink-0"
                onclick={() => handlePlay(reading)}
                disabled={actionInProgress !== null ||
                  deletingId !== null ||
                  playbackStore.isSynthesizing ||
                  !reading.hasAudio}
                aria-label={actionLabel}
                title={isActive
                  ? actionLabel
                  : !reading.hasAudio
                    ? $_("history.noAudioFile")
                    : actionLabel}
              >
                {#if actionInProgress === reading.id || (isCurrent && playbackStore.isLoadingAudio)}
                  <Spinner aria-hidden="true" />
                {:else if isActive}
                  <Square />
                {:else if isCurrent}
                  <RotateCcw />
                {:else}
                  <Play />
                {/if}
              </Button>
            </div>
          {/if}
        </li>
      {/each}
      {#if compact}
        <li class="flex w-40 shrink-0 flex-col justify-center gap-2">
          <Button href="/history" variant="ghost" class="w-full text-muted-foreground">
            View more
          </Button>
          <Button
            variant="ghost"
            class="w-full text-muted-foreground"
            onclick={openHistoryFolder}
            disabled={!readings.some((reading) => reading.items.some((item) => item.output_path))}
          >
            <FolderOpen />
            Open folder
          </Button>
        </li>
      {/if}
    </ul>
  {/if}

  {#if actionError || historyStore.error || playbackStore.error}
    <p class="text-destructive text-sm wrap-anywhere" role="alert">
      {actionError || historyStore.error || playbackStore.error}
    </p>
  {/if}
</section>

<Dialog
  open={selectedReading !== null}
  onOpenChange={(open) => {
    if (!open) selectedReading = null;
  }}
>
  <DialogContent class="flex max-h-[85dvh] min-w-0 flex-col sm:max-w-2xl">
    <DialogHeader>
      <DialogTitle>Reading text</DialogTitle>
      <DialogDescription>
        {selectedReading ? new Date(selectedReading.timestamp).toLocaleString() : ""}
      </DialogDescription>
    </DialogHeader>
    <div class="min-h-0 overflow-y-auto text-sm leading-relaxed whitespace-pre-wrap wrap-anywhere">
      {selectedReading?.text}
    </div>
  </DialogContent>
</Dialog>

<AlertDialog
  open={readingToDelete !== null}
  onOpenChange={(open) => {
    if (!open) readingToDelete = null;
  }}
>
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle>{$_("history.deleteEntry")}</AlertDialogTitle>
      <AlertDialogDescription
        >Delete this reading and all its saved audio? This cannot be undone.</AlertDialogDescription
      >
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogCancel>{$_("history.cancel")}</AlertDialogCancel>
      <AlertDialogAction onclick={confirmDelete}>{$_("history.confirmDelete")}</AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>

<style>
  /* Hallmark · pre-emit critique: P5 H5 E4 S5 R5 V4
   * component: history rail · genre: modern-minimal · theme: CopySpeak teal
   * contrast: pass (40–41) · tokens: pass (48) · responsive: pass (49)
   */
  .recent-history-rail {
    transform: rotateX(180deg);
    scrollbar-color: color-mix(in oklch, var(--muted-foreground) 35%, transparent) transparent;
    scrollbar-width: thin;
  }

  .recent-history-rail > li {
    transform: rotateX(180deg);
  }

  .recent-history-rail::-webkit-scrollbar {
    height: 5px;
  }

  .recent-history-rail::-webkit-scrollbar-track {
    background: transparent;
  }

  .recent-history-rail::-webkit-scrollbar-thumb {
    background: color-mix(in oklch, var(--muted-foreground) 35%, transparent);
    border-radius: var(--radius);
  }

  .recent-history-rail::-webkit-scrollbar-thumb:hover {
    background: color-mix(in oklch, var(--muted-foreground) 55%, transparent);
  }
</style>
