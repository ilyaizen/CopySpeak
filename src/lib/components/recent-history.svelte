<script lang="ts">
  import { onMount } from "svelte";
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
  import { Play, Square, Layers, Trash2, Clock } from "@lucide/svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Spinner } from "$lib/components/ui/spinner/index.js";
  import { historyStore } from "$lib/stores/history-store.svelte.js";
  import { playbackStore } from "$lib/stores/playback-store.svelte";
  import { groupHistoryReadings, historyVoiceLabel } from "$lib/models/history";
  import type { AppConfig, EngineCatalogEntry, VoiceProfile } from "$lib/types";
  import { isTauri } from "$lib/services/tauri";
  import { _ } from "svelte-i18n";

  interface Props {
    limit?: number;
    onSuccess?: (message: string) => void;
    onError?: (error: string) => void;
  }

  let { limit = 5, onSuccess, onError }: Props = $props();
  type Reading = ReturnType<typeof groupHistoryReadings>[number];
  let actionInProgress = $state<string | null>(null);
  let readingToDelete = $state<Reading | null>(null);
  let selectedReading = $state<Reading | null>(null);
  let actionError = $state<string | null>(null);
  let profiles = $state<VoiceProfile[]>([]);
  let engines = $state<EngineCatalogEntry[]>([]);

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
  const playbackBusy = $derived(playbackStore.isPlaying || playbackStore.isSynthesizing);

  async function handlePlay(reading: Reading) {
    actionInProgress = reading.id;
    actionError = null;
    try {
      if (playbackStore.historyReadingId === reading.id && playbackStore.isPlaying) {
        playbackStore.handleStop();
        await invoke("stop_speaking");
        return;
      }
      if (playbackStore.isPlaying) {
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
    actionInProgress = reading.id;
    actionError = null;
    try {
      if (reading.batchId) await historyStore.deleteBatch(reading.batchId);
      else await historyStore.deleteItem(reading.items[0].id);
      onSuccess?.("Deleted from history");
    } catch (e) {
      actionError = `Failed to delete: ${e}`;
      onError?.(actionError);
    } finally {
      actionInProgress = null;
    }
  }

  function formatDuration(ms: number) {
    const seconds = Math.round(ms / 1000);
    return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  }
</script>

<section class="flex min-w-0 flex-col gap-4" aria-labelledby="history-heading">
  <div class="flex flex-wrap items-baseline justify-between gap-2">
    <div>
      <h2 id="history-heading" class="text-lg font-bold tracking-tight">{$_("history.title")}</h2>
      <p class="text-muted-foreground text-sm">Your readings, ready to listen again.</p>
    </div>
    {#if historyStore.isLoading}
      <span class="text-muted-foreground text-xs" role="status">{$_("history.loading")}</span>
    {/if}
  </div>

  {#if readings.length === 0 && !historyStore.isLoading}
    <div class="text-muted-foreground flex flex-col items-center gap-3 py-12">
      <Clock class="size-6" />
      <p class="text-sm">{$_("history.empty")}</p>
    </div>
  {:else}
    <ul class="border-border divide-border divide-y border-y">
      {#each readings as reading (reading.id)}
        {@const item = reading.items[0]}
        {@const isCurrent = playbackStore.historyReadingId === reading.id}
        {@const isActive = isCurrent && playbackStore.isPlaying}
        {@const actionLabel = isActive
          ? $_("play.stop")
          : isCurrent
            ? $_("play.replay")
            : $_("play.play")}
        <li class="hover:bg-muted/40 flex min-w-0 items-start gap-3 px-2 py-4 transition-colors sm:px-3">
          <Button
            variant={isActive ? "secondary" : "outline"}
            size="icon"
            class="shrink-0"
            onclick={() => handlePlay(reading)}
            disabled={actionInProgress !== null || playbackStore.isSynthesizing || !reading.hasAudio}
            aria-label={actionLabel}
            title={isActive
              ? actionLabel
              : !reading.hasAudio
                ? $_("history.noAudioFile")
                : actionLabel}
          >
            {#if actionInProgress === reading.id}
              <Spinner />
            {:else if isActive}
              <Square />
            {:else}
              <Play />
            {/if}
          </Button>
          <div class="flex min-w-0 flex-1 flex-col gap-2">
            <button
              type="button"
              class="focus-visible:ring-ring min-w-0 cursor-pointer rounded-sm text-left text-sm leading-relaxed focus-visible:ring-2 focus-visible:outline-none"
              onclick={() => (selectedReading = reading)}
              aria-label="Read full text"
              aria-haspopup="dialog"
              title="Read full text"
            >
              <span class="line-clamp-2 min-h-10 whitespace-pre-wrap wrap-anywhere"
                >{reading.text}</span
              >
            </button>
            <div class="text-muted-foreground flex flex-wrap items-center gap-x-3 gap-y-1 text-xs">
              <time datetime={new Date(reading.timestamp).toISOString()}>
                {new Date(reading.timestamp).toLocaleString(undefined, {
                  dateStyle: "medium",
                  timeStyle: "short"
                })}
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
          </div>
          <Button
            variant="ghost"
            size="icon"
            class="shrink-0"
            onclick={() => (readingToDelete = reading)}
            disabled={actionInProgress !== null || playbackBusy}
            aria-label={$_("history.delete")}
            title={$_("history.delete")}
          >
            <Trash2 />
          </Button>
        </li>
      {/each}
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
