<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
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
  import * as DropdownMenu from "$lib/components/ui/dropdown-menu/index.js";
  import {
    Play,
    RotateCcw,
    Square,
    Layers,
    Trash2,
    Clock,
    FolderOpen,
    Ellipsis,
    Copy,
    Pencil
  } from "@lucide/svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { Spinner } from "$lib/components/ui/spinner/index.js";
  import { historyStore } from "$lib/stores/history-store.svelte.js";
  import { playbackStore } from "$lib/stores/playback-store.svelte";
  import { groupHistoryReadings, historyVoiceLabel, historyTimeAgo } from "$lib/models/history";
  import type { AppConfig, EngineCatalogEntry, VoiceProfile } from "$lib/types";
  import { isTauri } from "$lib/services/tauri";
  import { _ } from "svelte-i18n";

  type Reading = ReturnType<typeof groupHistoryReadings>[number];
  interface Props {
    limit?: number;
    compact?: boolean;
    onRestore?: (reading: Reading, regenerate?: boolean) => Promise<void>;
    onSuccess?: (message: string) => void;
    onError?: (error: string) => void;
  }
  let { limit = 3, compact = false, onRestore, onSuccess, onError }: Props = $props();
  let now = $state(Date.now());
  let actionInProgress = $state<string | null>(null);
  let readingsToDelete = $state<Reading[]>([]);
  let selectedIds = $state<string[]>([]);
  let readingToRename = $state<Reading | null>(null);
  let name = $state("");
  let actionError = $state<string | null>(null);
  let message = $state("");
  let profiles = $state<VoiceProfile[]>([]);
  let engines = $state<EngineCatalogEntry[]>([]);
  let search = $state("");
  let engineFilter = $state("");
  let statusFilter = $state("");

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
  const allReadings = $derived(groupHistoryReadings(historyStore.items));
  const engineOptions = $derived(
    [...new Set(historyStore.items.map((item) => item.tts_engine))].sort()
  );
  const readings = $derived(
    allReadings
      .filter((reading) => {
        if (compact) return true;
        const item = reading.items[0];
        const query = search.trim().toLocaleLowerCase();
        return (
          (!query ||
            `${reading.title} ${reading.text} ${historyVoiceLabel(item, profiles, engines)} ${item.voice}`
              .toLocaleLowerCase()
              .includes(query)) &&
          (!engineFilter || item.tts_engine === engineFilter) &&
          (!statusFilter || (statusFilter === "failed" ? !reading.success : reading.hasAudio))
        );
      })
      .slice(0, limit)
  );
  const selectedReadings = $derived(readings.filter((reading) => selectedIds.includes(reading.id)));
  const playbackBusy = $derived(
    playbackStore.isPlaying || playbackStore.isLoadingAudio || playbackStore.isSynthesizing
  );
  const busy = $derived(actionInProgress !== null);

  async function runAction(reading: Reading, action: () => Promise<void>, success = "") {
    actionInProgress = reading.id;
    actionError = null;
    message = "";
    try {
      await action();
      message = success;
      if (success) onSuccess?.(success);
    } catch (e) {
      actionError = `${e}`;
      onError?.(actionError);
    } finally {
      actionInProgress = null;
    }
  }

  async function restore(reading: Reading, regenerate = false) {
    await runAction(reading, async () => {
      if (onRestore) await onRestore(reading, regenerate);
      else
        await goto(
          `/?reading=${encodeURIComponent(reading.id)}${regenerate ? "&regenerate=1" : ""}`
        );
    });
  }

  async function handlePlay(reading: Reading) {
    await runAction(reading, async () => {
      const stopOnly = playbackStore.historyReadingId === reading.id && playbackBusy;
      if (playbackBusy) {
        playbackStore.handleStop();
        await invoke("stop_speaking");
      }
      if (stopOnly) return;
      if (reading.batchId) await historyStore.playBatch(reading.batchId);
      else await historyStore.playEntry(reading.items[0].id);
    });
  }

  async function confirmDelete() {
    const targets = readingsToDelete;
    readingsToDelete = [];
    if (!targets.length) return;
    await runAction(
      targets[0],
      async () => {
        if (playbackBusy) throw new Error("Stop playback before deleting readings.");
        for (const reading of targets) {
          if (reading.batchId) await historyStore.deleteBatch(reading.batchId);
          else await historyStore.deleteItem(reading.items[0].id);
          selectedIds = selectedIds.filter((id) => id !== reading.id);
        }
      },
      "Deleted from history"
    );
  }

  async function renameReading() {
    const reading = readingToRename;
    if (!reading) return;
    await runAction(
      reading,
      async () => {
        const title = name.trim();
        await invoke("rename_history_reading", { entryId: reading.items[0].id, title });
        for (const item of reading.items) {
          historyStore.updateItem(item.id, { metadata: { ...item.metadata, title } });
        }
        readingToRename = null;
      },
      "Reading renamed"
    );
  }

  function formatDuration(ms: number) {
    const seconds = Math.round(ms / 1000);
    return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  }
</script>

<section class="flex min-w-0 flex-col gap-3" aria-labelledby="history-heading">
  <div class="flex flex-wrap items-baseline justify-between gap-2">
    <h2 id="history-heading" class="text-base font-bold tracking-tight">
      {compact ? "Recent history" : $_("history.title")}
    </h2>
    {#if compact}
      <a
        href="/history"
        class="text-muted-foreground hover:text-foreground rounded-sm text-sm underline-offset-4 hover:underline focus-visible:outline-2"
        >View all history</a
      >
    {:else}
      <span class="text-muted-foreground text-sm"
        >{readings.length} of {allReadings.length} readings</span
      >
    {/if}
  </div>
  {#if !compact}
    <div class="flex flex-wrap items-center gap-2">
      <Input
        type="search"
        aria-label="Search history"
        placeholder="Search text, name or voice…"
        bind:value={search}
        class="min-w-40 flex-1"
      />
      <select
        aria-label="Filter by engine"
        bind:value={engineFilter}
        class="border-input bg-background h-9 max-w-full rounded-md border px-2 text-sm"
      >
        <option value="">All engines</option>
        {#each engineOptions as engine}
          <option value={engine}
            >{engines.find((entry) => entry.engine === engine)?.label ?? engine}</option
          >
        {/each}
      </select>
      <select
        aria-label="Filter by status"
        bind:value={statusFilter}
        class="border-input bg-background h-9 rounded-md border px-2 text-sm"
      >
        <option value="">All statuses</option>
        <option value="audio">Saved audio</option>
        <option value="failed">Failed</option>
      </select>
    </div>
    {#if readings.length}
      <div class="flex flex-wrap items-center justify-between gap-2 text-sm">
        <label class="flex items-center gap-2">
          <input
            type="checkbox"
            aria-label="Select all visible readings"
            disabled={busy || playbackBusy}
            checked={selectedReadings.length === readings.length}
            indeterminate={selectedReadings.length > 0 && selectedReadings.length < readings.length}
            onchange={(event) =>
              (selectedIds = event.currentTarget.checked
                ? readings.map((reading) => reading.id)
                : [])}
          />
          Select all
        </label>
        <Button
          variant="ghost"
          size="sm"
          disabled={!selectedReadings.length || busy || playbackBusy}
          onclick={() => (readingsToDelete = [...selectedReadings])}
        >
          <Trash2 /> Delete selected ({selectedReadings.length})
        </Button>
      </div>
    {/if}
  {/if}
  {#if historyStore.isLoading}<p class="text-muted-foreground text-sm" role="status">
      {$_("history.loading")}
    </p>{/if}
  {#if readings.length === 0 && !historyStore.isLoading}
    <div class="text-muted-foreground flex items-center justify-center gap-2 py-8">
      <Clock class="size-5" aria-hidden="true" />
      <p class="text-sm">
        {allReadings.length ? "No readings match these filters." : $_("history.empty")}
      </p>
    </div>
  {:else}
    <ul class="border-border divide-border min-w-0 divide-y border-y">
      {#each readings as reading (reading.id)}
        {@const item = reading.items[0]}
        {@const isCurrent = playbackStore.historyReadingId === reading.id}
        {@const isActive = isCurrent && playbackBusy}
        {@const actionLabel = isActive
          ? $_("play.stop")
          : isCurrent
            ? $_("play.replay")
            : $_("play.play")}
        <li
          class="hover:bg-muted/40 flex min-w-0 items-center gap-2 py-3 transition-colors sm:gap-3 sm:px-2"
        >
          {#if !compact}
            <input
              type="checkbox"
              aria-label={`Select ${reading.title || reading.text}`}
              disabled={busy || playbackBusy}
              checked={selectedIds.includes(reading.id)}
              onchange={(event) =>
                (selectedIds = event.currentTarget.checked
                  ? [...selectedIds, reading.id]
                  : selectedIds.filter((id) => id !== reading.id))}
            />
          {/if}
          <button
            type="button"
            class="focus-visible:ring-ring min-w-0 flex-1 cursor-pointer rounded-sm text-start focus-visible:ring-2 focus-visible:outline-none disabled:cursor-default disabled:opacity-50"
            onclick={() => restore(reading)}
            disabled={busy || playbackBusy}
            aria-label={`Restore reading: ${reading.title || reading.text}`}
            title="Restore text, voice and speed"
          >
            {#if reading.title}<span class="mb-1 block truncate text-sm font-semibold"
                >{reading.title}</span
              >{/if}
            <span class="line-clamp-2 text-sm leading-relaxed whitespace-pre-wrap wrap-anywhere"
              >{reading.text}</span
            >
            <span
              class="text-muted-foreground mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs"
            >
              <span class="max-w-full truncate" title={`${item.tts_engine} · ${item.voice}`}>
                {historyVoiceLabel(item, profiles, engines)} · {engines.find(
                  (entry) => entry.engine === item.tts_engine
                )?.label ?? item.tts_engine}
              </span>
              {#if reading.durationMs > 0}<span class="tabular-nums"
                  >· {formatDuration(reading.durationMs)}</span
                >{/if}
              <time
                datetime={new Date(reading.timestamp).toISOString()}
                title={new Date(reading.timestamp).toLocaleString()}
                >· {historyTimeAgo(reading.timestamp, now)}</time
              >
              {#if reading.partCount > 1}<span class="inline-flex items-center gap-1"
                  ><Layers class="size-3" aria-hidden="true" />{reading.partCount} parts</span
                >{/if}
              {#if !reading.success}<span class="text-destructive">Generation failed</span>
              {:else if !reading.hasAudio}<span>{$_("history.noAudioFile")}</span>{/if}
            </span>
          </button>
          <Button
            variant={isActive ? "secondary" : "ghost"}
            size="icon-sm"
            class="shrink-0"
            onclick={() => handlePlay(reading)}
            disabled={busy || playbackStore.isSynthesizing || !reading.hasAudio}
            aria-label={actionLabel}
            title={!reading.hasAudio ? $_("history.noAudioFile") : actionLabel}
          >
            {#if actionInProgress === reading.id || (isCurrent && playbackStore.isLoadingAudio)}<Spinner
                aria-hidden="true"
              />
            {:else if isActive}<Square />{:else if isCurrent}<RotateCcw />{:else}<Play />{/if}
          </Button>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger
              aria-label={`Actions for ${reading.title || reading.text}`}
              disabled={busy}
              class="hover:bg-muted focus-visible:ring-ring flex size-8 shrink-0 items-center justify-center rounded-md focus-visible:ring-2 disabled:opacity-50"
            >
              <Ellipsis class="size-4" aria-hidden="true" />
            </DropdownMenu.Trigger>
            <DropdownMenu.Content align="end">
              <DropdownMenu.Item
                onclick={() => {
                  readingToRename = reading;
                  name = reading.title;
                }}><Pencil />Rename</DropdownMenu.Item
              >
              <DropdownMenu.Item
                onclick={() =>
                  runAction(
                    reading,
                    () => navigator.clipboard.writeText(reading.text),
                    "Text copied"
                  )}><Copy />Copy text</DropdownMenu.Item
              >
              <DropdownMenu.Item
                disabled={!item.output_path}
                onclick={() =>
                  runAction(reading, async () => {
                    if (item.output_path) await revealItemInDir(item.output_path);
                  })}><FolderOpen />Reveal file</DropdownMenu.Item
              >
              <DropdownMenu.Item disabled={playbackBusy} onclick={() => restore(reading, true)}
                ><RotateCcw />Regenerate</DropdownMenu.Item
              >
              <DropdownMenu.Separator />
              <DropdownMenu.Item
                variant="destructive"
                disabled={playbackBusy}
                onclick={() => (readingsToDelete = [reading])}><Trash2 />Delete</DropdownMenu.Item
              >
            </DropdownMenu.Content>
          </DropdownMenu.Root>
        </li>
      {/each}
    </ul>
  {/if}
  {#if message}<p role="status" class="text-muted-foreground text-sm">{message}</p>{/if}
  {#if actionError || historyStore.error || playbackStore.error}
    <p class="text-destructive text-sm wrap-anywhere" role="alert">
      {actionError || historyStore.error || playbackStore.error}
    </p>
  {/if}
</section>

<Dialog
  open={readingToRename !== null}
  onOpenChange={(open) => {
    if (!open && !busy) readingToRename = null;
  }}
>
  <DialogContent>
    <DialogHeader>
      <DialogTitle>Rename reading</DialogTitle>
      <DialogDescription
        >Give this reading a name. Leave blank to show only its text. Audio filenames stay the same.</DialogDescription
      >
    </DialogHeader>
    <form
      class="flex flex-col gap-3"
      onsubmit={(event) => {
        event.preventDefault();
        renameReading();
      }}
    >
      <label for="reading-name" class="text-sm">Name</label>
      <Input id="reading-name" bind:value={name} maxlength={200} disabled={busy} />
      {#if actionError}<p role="alert" class="text-destructive text-sm">{actionError}</p>{/if}
      <Button type="submit" disabled={busy}>Save name</Button>
    </form>
  </DialogContent>
</Dialog>

<AlertDialog
  open={readingsToDelete.length > 0}
  onOpenChange={(open) => {
    if (!open) readingsToDelete = [];
  }}
>
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle
        >Delete {readingsToDelete.length === 1
          ? "reading"
          : `${readingsToDelete.length} readings`}?</AlertDialogTitle
      >
      <AlertDialogDescription
        >Delete the selected readings and all their saved audio parts? This cannot be undone.</AlertDialogDescription
      >
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogCancel>{$_("history.cancel")}</AlertDialogCancel>
      <AlertDialogAction onclick={confirmDelete}>{$_("history.confirmDelete")}</AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
