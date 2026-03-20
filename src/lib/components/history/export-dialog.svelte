<script lang="ts">
  import {
    AlertDialog,
    AlertDialogContent,
    AlertDialogHeader,
    AlertDialogTitle,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogAction,
    AlertDialogCancel
  } from "$lib/components/ui/alert-dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Calendar, Download, FileText, X } from "@lucide/svelte";
  import type { HistoryExportOptions } from "$lib/types";
  import { cn } from "$lib/utils.js";

  interface Props {
    open?: boolean;
    onOpenChange?: (open: boolean) => void;
    onExport?: (options: HistoryExportOptions) => void;
  }

  let { open = false, onOpenChange, onExport }: Props = $props();

  let dateFrom = $state("");
  let dateTo = $state("");
  let filename = $state("");
  let includeMetadata = $state(true);

  const dateToday = new Date().toISOString().split("T")[0];

  const hasValidDates = $derived(!!dateFrom && !!dateTo);

  const filenameValue = $derived(
    filename || `copyspeak_export_${new Date().toISOString().split("T")[0]}`
  );

  const exportOptions = $derived<HistoryExportOptions>({
    format: "json",
    include_metadata: includeMetadata,
    date_from: dateFrom ? new Date(dateFrom).getTime() : undefined,
    date_to: dateTo ? new Date(dateTo).getTime() + 86399999 : undefined,
    filters: {
      date_from: dateFrom ? new Date(dateFrom).getTime() : undefined,
      date_to: dateTo ? new Date(dateTo).getTime() + 86399999 : undefined
    }
  });

  $effect(() => {
    if (dateFrom) {
      dateTo = dateTo || dateToday;
    }
  });

  $effect(() => {
    if (!open) {
      dateFrom = "";
      dateTo = "";
      filename = "";
      includeMetadata = true;
    }
  });

  function handleOpenChange(newOpen: boolean) {
    onOpenChange?.(newOpen);
  }

  function handleExport() {
    if (!hasValidDates) {
      return;
    }
    onExport?.(exportOptions);
    onOpenChange?.(false);
  }
</script>

<AlertDialog {open} onOpenChange={handleOpenChange}>
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle class="flex items-center gap-2">
        <Download class="h-5 w-5" />
        Export History
      </AlertDialogTitle>
      <AlertDialogDescription>
        Export your speech history as a JSON file with optional date range filtering.
      </AlertDialogDescription>
    </AlertDialogHeader>

    <div class="space-y-4 py-4">
      <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
        <div class="space-y-2">
          <Label for="export-date-from" class="flex items-center gap-2">
            <Calendar class="h-3 w-3" />
            Date From
          </Label>
          <div class="relative">
            <Input
              id="export-date-from"
              type="date"
              bind:value={dateFrom}
              max={dateTo || dateToday}
            />
            {#if dateFrom}
              <button
                type="button"
                class="hover:bg-muted text-muted-foreground hover:text-foreground absolute top-1 right-1 flex h-7 w-7 items-center justify-center rounded"
                onclick={() => (dateFrom = "")}
              >
                <X class="h-3 w-3" />
              </button>
            {/if}
          </div>
        </div>

        <div class="space-y-2">
          <Label for="export-date-to" class="flex items-center gap-2">
            <Calendar class="h-3 w-3" />
            Date To
          </Label>
          <div class="relative">
            <Input
              id="export-date-to"
              type="date"
              bind:value={dateTo}
              min={dateFrom}
              max={dateToday}
            />
            {#if dateTo}
              <button
                type="button"
                class="hover:bg-muted text-muted-foreground hover:text-foreground absolute top-1 right-1 flex h-7 w-7 items-center justify-center rounded"
                onclick={() => (dateTo = "")}
              >
                <X class="h-3 w-3" />
              </button>
            {/if}
          </div>
        </div>
      </div>

      <div class="space-y-2">
        <Label for="export-filename" class="flex items-center gap-2">
          <FileText class="h-3 w-3" />
          Filename (without extension)
        </Label>
        <Input
          id="export-filename"
          type="text"
          bind:value={filename}
          placeholder="copyspeak_export_2025-02-15"
        />
        <p class="text-muted-foreground text-xs">
          File will be saved as: <span class="font-mono">{filenameValue}.json</span>
        </p>
      </div>

      <div class="flex items-center gap-2">
        <input
          type="checkbox"
          id="export-metadata"
          bind:checked={includeMetadata}
          class="border-border bg-background text-primary focus:ring-ring h-4 w-4 rounded"
        />
        <Label for="export-metadata" class="text-card-foreground cursor-pointer text-sm">
          Include full metadata (timestamps, TTS settings, etc.)
        </Label>
      </div>

      {#if hasValidDates}
        <div class="bg-muted rounded-md p-3">
          <p class="text-muted-foreground text-xs">
            Exporting entries from <strong>{dateFrom}</strong> to
            <strong>{dateTo}</strong>
          </p>
        </div>
      {/if}
    </div>

    <AlertDialogFooter>
      <AlertDialogCancel>Cancel</AlertDialogCancel>
      <AlertDialogAction
        onclick={handleExport}
        disabled={!hasValidDates}
        class={cn(!hasValidDates && "cursor-not-allowed opacity-50")}
      >
        <Download class="mr-2 h-4 w-4" />
        Export
      </AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
