<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
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
    Play,
    RotateCcw,
    Trash2,
    Clock,
    ChevronDown,
    ChevronUp,
    ChevronRight
  } from "@lucide/svelte";
  import { historyStore } from "$lib/stores/history-store.svelte.js";
  import type { HistoryItem } from "$lib/types";
  import { _ } from "svelte-i18n";

  interface Props {
    limit?: number;
    onSuccess?: (message: string) => void;
    onError?: (error: string) => void;
  }

  let { limit = 5, onSuccess, onError }: Props = $props();

  let actionInProgress = $state<string | null>(null);
  let itemToDelete = $state<HistoryItem | null>(null);
  let batchToDelete = $state<string | null>(null);
  let expandedItems = $state<Set<string>>(new Set());
  let expandedBatches = $state<Set<string>>(new Set());

  function toggleExpand(itemId: string) {
    if (expandedItems.has(itemId)) {
      expandedItems.delete(itemId);
      expandedItems = new Set(expandedItems);
    } else {
      expandedItems = new Set(expandedItems.add(itemId));
    }
  }

  function toggleBatchExpand(batchId: string) {
    if (expandedBatches.has(batchId)) {
      expandedBatches.delete(batchId);
      expandedBatches = new Set(expandedBatches);
    } else {
      expandedBatches = new Set(expandedBatches.add(batchId));
    }
  }

  interface GroupedItem {
    type: "single";
    item: HistoryItem;
    timestamp: number;
  }

  interface GroupedBatch {
    type: "batch";
    batchId: string;
    items: HistoryItem[];
    timestamp: number;
    tts_engine: string;
    voice: string;
    success: boolean;
    totalFragments: number;
  }

  // Get recent items sorted descending by timestamp, grouped by batch_id
  const recentGrouped = $derived(() => {
    const sortedItems = [...historyStore.items].sort((a, b) => b.timestamp - a.timestamp);
    const seenBatchIds = new Set<string>();
    const result: Array<GroupedItem | GroupedBatch> = [];

    for (const item of sortedItems) {
      if (item.batch_id && !seenBatchIds.has(item.batch_id)) {
        seenBatchIds.add(item.batch_id);
        const batchItems = sortedItems.filter((i) => i.batch_id === item.batch_id);
        const totalFragments = batchItems.length;
        const firstItem = batchItems.reduce((a, b) => (a.timestamp < b.timestamp ? a : b));

        result.push({
          type: "batch",
          batchId: item.batch_id,
          items: batchItems.sort((a, b) => {
            const idxA = (a.metadata?.fragment_index as number) ?? 0;
            const idxB = (b.metadata?.fragment_index as number) ?? 0;
            return idxA - idxB;
          }),
          timestamp: firstItem.timestamp,
          tts_engine: item.tts_engine,
          voice: item.voice,
          success: batchItems.every((i) => i.success),
          totalFragments
        });
      } else if (!item.batch_id) {
        result.push({
          type: "single",
          item,
          timestamp: item.timestamp
        });
      }
    }

    return result.sort((a, b) => b.timestamp - a.timestamp).slice(0, limit);
  });

  async function handlePlay(item: HistoryItem) {
    if (!item.output_path) {
      await handleReSpeak(item);
      return;
    }

    actionInProgress = item.id;
    try {
      await historyStore.playEntry(item.id);
      onSuccess?.("Playing audio from history");
    } catch (e) {
      onError?.(`Failed to play: ${e}`);
    } finally {
      actionInProgress = null;
    }
  }

  async function handlePlayBatch(batchId: string) {
    actionInProgress = batchId;
    try {
      await historyStore.playBatch(batchId);
      onSuccess?.("Playing all fragments");
    } catch (e) {
      onError?.(`Failed to play batch: ${e}`);
    } finally {
      actionInProgress = null;
    }
  }

  async function handleReSpeak(item: HistoryItem) {
    actionInProgress = item.id;
    try {
      await historyStore.reSpeakEntry(item.id);
      onSuccess?.("Re-synthesizing and playing");
    } catch (e) {
      onError?.(`Failed to re-speak: ${e}`);
    } finally {
      actionInProgress = null;
    }
  }

  async function confirmDelete() {
    if (batchToDelete) {
      actionInProgress = batchToDelete;
      const bid = batchToDelete;
      batchToDelete = null;
      itemToDelete = null;
      try {
        await historyStore.deleteBatch(bid);
        onSuccess?.("Deleted batch from history");
      } catch (e) {
        onError?.(`Failed to delete batch: ${e}`);
      } finally {
        actionInProgress = null;
      }
    } else if (itemToDelete) {
      const item = itemToDelete;
      itemToDelete = null;
      actionInProgress = item.id;
      try {
        await historyStore.deleteItem(item.id);
        onSuccess?.("Deleted from history");
      } catch (e) {
        onError?.(`Failed to delete: ${e}`);
      } finally {
        actionInProgress = null;
      }
    }
  }

  function extractBatchInfo(item: HistoryItem): { position: number; total: number } | null {
    if (
      item.metadata &&
      typeof item.metadata.fragment_index === "number" &&
      typeof item.metadata.fragment_total === "number"
    ) {
      return {
        position: (item.metadata.fragment_index as number) + 1,
        total: item.metadata.fragment_total as number
      };
    }
    const match = item.text.match(/\((\d+) of (\d+)\)\s*$/);
    if (match) {
      return { position: parseInt(match[1], 10), total: parseInt(match[2], 10) };
    }
    return null;
  }

  function formatSynthesisDuration(ms: number): string {
    if (ms < 1000) return `${ms}ms`;
    const seconds = ms / 1000;
    if (seconds < 60) return `${seconds.toFixed(1)}s`;
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = (seconds % 60).toFixed(1);
    return `${minutes}m ${remainingSeconds}s`;
  }

  function getSynthesisMs(item: HistoryItem): number | null {
    if (item.metadata && typeof item.metadata.synthesis_ms === "number") {
      return item.metadata.synthesis_ms as number;
    }
    return null;
  }

  function truncateText(text: string, maxLen: number): string {
    if (text.length <= maxLen) return text;
    return text.substring(0, maxLen - 3) + "...";
  }
</script>

<div class="space-y-3">
  <div class="flex items-center justify-between">
    <h2 class="text-card-foreground flex items-center gap-2 font-medium">
      <Clock class="h-4 w-4" />
      {$_("history.title")}
    </h2>
    {#if historyStore.isLoading}
      <span class="text-muted-foreground text-xs">{$_("history.loading")}</span>
    {/if}
  </div>

  {#if recentGrouped().length === 0}
    <p class="text-muted-foreground py-4 text-center text-sm italic">{$_("history.empty")}</p>
  {:else}
    <div class="space-y-2">
      {#each recentGrouped() as grouped (grouped.type === "batch" ? grouped.batchId : grouped.item.id)}
        {#if grouped.type === "single"}
          {@const item = grouped.item}
          <div
            class="border-border bg-card hover:bg-accent/20 overflow-hidden rounded-lg border transition-colors"
          >
            <div
              class="bg-muted/30 border-border/50 flex items-center justify-between gap-2 border-b px-3 py-2"
            >
              <div
                class="text-muted-foreground flex min-w-0 items-center gap-1.5 overflow-hidden text-xs"
              >
                <span
                  class="h-1.5 w-1.5 shrink-0 rounded-full {item.success
                    ? 'bg-green-500'
                    : 'bg-red-500'}"
                ></span>
                <span class="text-foreground shrink-0 font-medium">
                  {item.output_path ? item.output_path.split(/[/\\]/).pop() : item.tts_engine}
                </span>
              </div>
              <div class="flex shrink-0 items-center gap-0.5">
                {#if getSynthesisMs(item)}
                  <span class="text-muted-foreground mr-1 text-xs whitespace-nowrap">
                    {formatSynthesisDuration(getSynthesisMs(item)!)}
                  </span>
                {/if}
                <Button
                  variant="ghost"
                  size="icon"
                  class="h-6 w-6"
                  onclick={() => handlePlay(item)}
                  disabled={actionInProgress === item.id}
                  title={item.output_path ? $_("history.playSaved") : $_("history.reSynthesize")}
                >
                  <Play class="h-3 w-3" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  class="h-6 w-6"
                  onclick={() => handleReSpeak(item)}
                  disabled={actionInProgress === item.id}
                  title={$_("history.reSynthesizeTooltip")}
                >
                  <RotateCcw class="h-3 w-3" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  class="h-6 w-6"
                  onclick={() => (itemToDelete = item)}
                  disabled={actionInProgress === item.id}
                  title={$_("history.delete")}
                >
                  <Trash2 class="h-3 w-3" />
                </Button>
              </div>
            </div>
            <button
              type="button"
              class="relative w-full cursor-pointer text-left"
              onclick={() => toggleExpand(item.id)}
              title={expandedItems.has(item.id) ? "Click to collapse" : "Click to expand"}
            >
              <pre
                class="text-card-foreground overflow-hidden px-3 py-2 pr-8 font-mono text-xs leading-relaxed whitespace-pre-wrap {expandedItems.has(
                  item.id
                )
                  ? ''
                  : 'line-clamp-1'}">
{item.text}</pre>
              <span
                class="text-muted-foreground hover:text-foreground absolute right-2 bottom-1 rounded p-0.5 transition-colors"
              >
                {#if expandedItems.has(item.id)}
                  <ChevronUp class="h-3 w-3" />
                {:else}
                  <ChevronDown class="h-3 w-3" />
                {/if}
              </span>
            </button>
          </div>
        {:else}
          {@const batch = grouped}
          <div class="border-border bg-card overflow-hidden rounded-lg border transition-colors">
            <button
              type="button"
              class="bg-muted/30 border-border/50 hover:bg-accent/10 flex w-full items-center justify-between gap-2 border-b px-3 py-2 text-left"
              onclick={() => toggleBatchExpand(batch.batchId)}
            >
              <div class="flex min-w-0 items-center gap-1.5">
                <ChevronRight
                  class="h-3 w-3 shrink-0 transition-transform {expandedBatches.has(batch.batchId)
                    ? 'rotate-90'
                    : ''}"
                />
                <span
                  class="h-1.5 w-1.5 shrink-0 rounded-full {batch.success
                    ? 'bg-green-500'
                    : 'bg-red-500'}"
                ></span>
                <span class="text-foreground shrink-0 text-xs font-medium">{batch.tts_engine}</span>
                <Badge variant="secondary" class="h-4 shrink-0 px-1.5 py-0 text-[10px]">
                  {$_("history.fragments", { values: { count: batch.totalFragments } })}
                </Badge>
              </div>
              <div class="flex shrink-0 items-center gap-1">
                <span class="text-muted-foreground text-xs">
                  {new Date(batch.timestamp).toLocaleTimeString()}
                </span>
              </div>
            </button>

            {#if expandedBatches.has(batch.batchId)}
              <div class="border-border/50 border-t">
                <!-- Batch actions -->
                <div
                  class="bg-muted/10 border-border/30 flex items-center justify-between gap-2 border-b px-3 py-2"
                >
                  <span class="text-muted-foreground text-xs">
                    {batch.voice}
                  </span>
                  <div class="flex shrink-0 items-center gap-1">
                    <Button
                      variant="ghost"
                      size="sm"
                      class="h-6 px-2 text-xs"
                      onclick={(e) => {
                        e.stopPropagation();
                        handlePlayBatch(batch.batchId);
                      }}
                      disabled={actionInProgress === batch.batchId}
                      title={$_("history.playAllTooltip")}
                    >
                      <Play class="mr-1 h-3 w-3" />
                      {$_("history.playAll")}
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-6 w-6"
                      onclick={(e) => {
                        e.stopPropagation();
                        batchToDelete = batch.batchId;
                      }}
                      disabled={actionInProgress === batch.batchId}
                      title={$_("history.deleteAllFragments")}
                    >
                      <Trash2 class="h-3 w-3" />
                    </Button>
                  </div>
                </div>

                <!-- Fragment list -->
                <div class="max-h-64 overflow-y-auto">
                  {#each batch.items as fragment (fragment.id)}
                    {@const batchInfo = extractBatchInfo(fragment)}
                    <div
                      class="hover:bg-accent/10 border-border/30 flex items-center justify-between gap-2 border-b px-3 py-1.5 last:border-b-0"
                    >
                      <div class="flex min-w-0 flex-1 items-center gap-1.5">
                        {#if batchInfo}
                          <Badge variant="outline" class="h-4 shrink-0 px-1 text-[9px]">
                            {batchInfo.position}/{batchInfo.total}
                          </Badge>
                        {/if}
                        <span class="truncate text-xs">
                          {truncateText(fragment.text, 60)}
                        </span>
                      </div>
                      <div class="flex shrink-0 items-center gap-0.5">
                        {#if getSynthesisMs(fragment)}
                          <span class="text-muted-foreground mr-1 text-[10px]">
                            {formatSynthesisDuration(getSynthesisMs(fragment)!)}
                          </span>
                        {/if}
                        <Button
                          variant="ghost"
                          size="icon"
                          class="h-5 w-5"
                          onclick={(e) => {
                            e.stopPropagation();
                            handlePlay(fragment);
                          }}
                          disabled={actionInProgress === fragment.id || !fragment.output_path}
                          title={fragment.output_path
                            ? $_("history.playThisFragment")
                            : $_("history.noAudioFile")}
                        >
                          <Play class="h-2.5 w-2.5" />
                        </Button>
                      </div>
                    </div>
                  {/each}
                </div>
              </div>
            {/if}
          </div>
        {/if}
      {/each}
    </div>
  {/if}

  {#if historyStore.error}
    <p class="text-destructive text-sm">
      {historyStore.error}
    </p>
  {/if}
</div>

<AlertDialog
  open={!!(itemToDelete || batchToDelete)}
  onOpenChange={(open) => {
    if (!open) {
      itemToDelete = null;
      batchToDelete = null;
    }
  }}
>
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle
        >{batchToDelete ? $_("history.deleteBatch") : $_("history.deleteEntry")}</AlertDialogTitle
      >
      <AlertDialogDescription>
        {#if batchToDelete}
          {$_("history.deleteBatchDescription")}
        {:else if itemToDelete?.output_path}
          {$_("history.deleteEntryWithFile")}
        {:else}
          {$_("history.deleteEntryOnly")}
        {/if}
      </AlertDialogDescription>
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogCancel>{$_("history.cancel")}</AlertDialogCancel>
      <AlertDialogAction onclick={confirmDelete}>{$_("history.confirmDelete")}</AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
