<script lang="ts">
  import { cn } from "$lib/utils.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import {
    Copy,
    Check,
    Clock,
    AlertCircle,
    FileText,
    Play,
    RotateCcw,
    Trash2
  } from "@lucide/svelte";
  import { formatHistoryDate } from "$lib/models/history.js";
  import type { HistoryItem } from "$lib/types";
  import { isTauri } from "$lib/services/tauri.js";

  let invoke: typeof import("@tauri-apps/api/core").invoke | null = null;

  // Lazily initialize invoke when first needed
  async function getInvoke() {
    if (!invoke && isTauri) {
      const core = await import("@tauri-apps/api/core");
      invoke = core.invoke;
    }
    return invoke;
  }

  let {
    entry,
    onCopy,
    showCopySuccess = false,
    truncateLength = 200,
    onPlay,
    onReSpeak,
    onDelete
  } = $props<{
    entry: HistoryItem;
    onCopy?: (text: string) => void;
    showCopySuccess?: boolean;
    truncateLength?: number;
    onPlay?: (entry: HistoryItem) => void;
    onReSpeak?: (entry: HistoryItem) => void;
    onDelete?: (entry: HistoryItem) => void;
  }>();

  function truncateText(text: string, maxLength: number): string {
    if (text.length <= maxLength) return text;
    return text.slice(0, maxLength) + "...";
  }

  function formatDuration(ms: number): string {
    if (!ms) return "--";
    const seconds = Math.floor(ms / 1000);
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return `${minutes}m ${remainingSeconds}s`;
  }

  async function handlePlay() {
    if (onPlay) {
      onPlay(entry);
      return;
    }

    if (!entry.output_path) {
      await handleReSpeak();
      return;
    }

    try {
      // Show HUD if enabled (will be ignored if HUD is disabled in config)
      const inv = await getInvoke();
      await inv?.("show_hud_for_playback", { text: entry.text });
      await inv?.("play_history_entry", { entryId: entry.id });
    } catch (e) {
      console.error("Failed to play entry:", e);
      await handleReSpeak();
    }
  }

  async function handleReSpeak() {
    if (onReSpeak) {
      onReSpeak(entry);
      return;
    }

    try {
      const inv = await getInvoke();
      await inv?.("speak_history_entry", { entryId: entry.id });
    } catch (e) {
      console.error("Failed to re-speak entry:", e);
    }
  }

  async function handleDelete() {
    if (onDelete) {
      onDelete(entry);
      return;
    }

    if (!confirm(`Delete this history entry? This action cannot be undone.`)) {
      return;
    }

    try {
      const inv = await getInvoke();
      await inv?.("delete_history_entry", { entryId: entry.id });
      window.location.reload();
    } catch (e) {
      console.error("Failed to delete entry:", e);
    }
  }
</script>

<div
  class={cn(
    "border-border bg-card hover:bg-accent/50 rounded-lg border p-4 shadow-sm transition-colors",
    !entry.success && "border-destructive/50"
  )}
>
  <div class="mb-3 flex items-start justify-between gap-2">
    <div class="flex min-w-0 flex-1 items-center gap-2">
      {#if entry.success}
        <div class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-green-500/20">
          <FileText class="h-3 w-3 text-green-600 dark:text-green-400" />
        </div>
      {:else}
        <div class="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-red-500/20">
          <AlertCircle class="h-3 w-3 text-red-600 dark:text-red-400" />
        </div>
      {/if}
      <div class="min-w-0 flex-1">
        <p class="text-card-foreground truncate text-sm font-medium">
          {entry.id}
        </p>
        <div class="text-muted-foreground flex items-center gap-2 text-xs">
          <Clock class="h-3 w-3" />
          <span>{formatHistoryDate(entry.timestamp)}</span>
        </div>
      </div>
    </div>

    <div class="flex shrink-0 items-center gap-1">
      <Button
        variant="ghost"
        size="sm"
        class="h-7 w-7 p-0"
        onclick={handlePlay}
        title={entry.output_path ? "Play audio" : "Re-speak"}
      >
        {#if entry.output_path}
          <Play class="h-4 w-4" />
        {:else}
          <RotateCcw class="h-4 w-4" />
        {/if}
      </Button>
      <Button variant="ghost" size="sm" class="h-7 w-7 p-0" onclick={() => onCopy?.(entry.text)}>
        {#if showCopySuccess}
          <Check class="h-4 w-4 text-green-500" />
        {:else}
          <Copy class="h-4 w-4" />
        {/if}
      </Button>
      <Button
        variant="ghost"
        size="sm"
        class="text-destructive hover:text-destructive h-7 w-7 p-0"
        onclick={handleDelete}
        title="Delete entry"
      >
        <Trash2 class="h-4 w-4" />
      </Button>
    </div>
  </div>

  <div class="mb-3">
    <p class="text-card-foreground line-clamp-4 text-sm whitespace-pre-wrap">
      {truncateText(entry.text, truncateLength)}
    </p>
  </div>

  <div class="grid grid-cols-2 gap-2 text-xs">
    <div>
      <span class="text-muted-foreground">Engine:</span>
      <span class="text-card-foreground ml-1 font-medium capitalize">
        {entry.tts_engine}
      </span>
    </div>
    <div>
      <span class="text-muted-foreground">Voice:</span>
      <span class="text-card-foreground ml-1 font-medium">
        {entry.voice}
      </span>
    </div>
    <div>
      <span class="text-muted-foreground">Duration:</span>
      <span class="text-card-foreground ml-1 font-medium">
        {formatDuration(entry.duration_ms ?? 0)}
      </span>
    </div>
    <div>
      <span class="text-muted-foreground">Length:</span>
      <span class="text-card-foreground ml-1 font-medium">
        {entry.text_length.toLocaleString()} chars
      </span>
    </div>
    <div>
      <span class="text-muted-foreground">Attempts:</span>
      <span class="text-card-foreground ml-1 font-medium">
        {entry.attempts}
      </span>
    </div>
  </div>

  {#if entry.app_name || entry.output_path}
    <div class="border-border mt-3 border-t pt-3">
      <div class="space-y-1 text-xs">
        {#if entry.app_name}
          <div>
            <span class="text-muted-foreground">App:</span>
            <span class="text-card-foreground ml-1">
              {entry.app_name}
            </span>
          </div>
        {/if}
        {#if entry.output_path}
          <div>
            <span class="text-muted-foreground">Output:</span>
            <span class="text-card-foreground ml-1 block max-w-50 truncate">
              {entry.output_path}
            </span>
          </div>
        {/if}
      </div>
    </div>
  {/if}

  {#if !entry.success && entry.error_message}
    <div class="bg-destructive/10 border-destructive/20 mt-3 rounded border px-3 py-2">
      <p class="text-destructive text-xs">{entry.error_message}</p>
    </div>
  {/if}

  {#if entry.tags && entry.tags.length > 0}
    <div class="mt-3 flex flex-wrap gap-1">
      {#each entry.tags as tag}
        <span
          class="bg-secondary text-secondary-foreground inline-flex items-center rounded-full px-2 py-0.5 text-xs font-medium"
        >
          {tag}
        </span>
      {/each}
    </div>
  {/if}
</div>
