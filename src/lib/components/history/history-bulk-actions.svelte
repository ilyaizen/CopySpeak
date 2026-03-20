<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Trash2, Download, CheckSquare, Square, X } from "@lucide/svelte";

  let {
    totalItems = 0,
    selectedCount = 0,
    isAllSelected = false,
    isSomeSelected = false,
    onSelectAll,
    onClearSelection,
    onDeleteSelected,
    onExportSelected,
    disabled = false,
    class: className
  } = $props<{
    totalItems: number;
    selectedCount: number;
    isAllSelected: boolean;
    isSomeSelected: boolean;
    onSelectAll: () => void;
    onClearSelection: () => void;
    onDeleteSelected: () => void;
    onExportSelected: () => void;
    disabled?: boolean;
    class?: string;
  }>();

  let showDeleteConfirm = $state(false);

  function handleDeleteClick() {
    if (showDeleteConfirm) {
      onDeleteSelected();
      showDeleteConfirm = false;
    } else {
      showDeleteConfirm = true;
    }
  }

  $effect(() => {
    if (selectedCount === 0) {
      showDeleteConfirm = false;
    }
  });
</script>

<div class={className}>
  <div
    class="border-border bg-card flex items-center justify-between rounded-lg border p-4 shadow-sm"
  >
    <div class="flex items-center gap-3">
      <button
        type="button"
        disabled={disabled || totalItems === 0}
        class="text-card-foreground flex items-center gap-2 text-sm disabled:cursor-not-allowed disabled:opacity-50"
        onclick={onSelectAll}
      >
        {#if isAllSelected}
          <CheckSquare class="h-4 w-4" />
        {:else if isSomeSelected}
          <Square class="h-4 w-4" />
        {:else}
          <Square class="h-4 w-4 opacity-50" />
        {/if}
        <span>
          {isAllSelected
            ? "Selected all"
            : selectedCount > 0
              ? `${selectedCount} selected`
              : "Select all"}
        </span>
      </button>

      {#if selectedCount > 0}
        <div class="bg-border h-4 w-px"></div>
        <Button
          variant="ghost"
          size="sm"
          {disabled}
          onclick={onClearSelection}
          class="h-7 px-2 text-xs"
        >
          <X class="mr-1 h-3 w-3" />
          Clear selection
        </Button>
      {/if}
    </div>

    {#if selectedCount > 0}
      <div class="flex items-center gap-2">
        <Button variant="outline" size="sm" {disabled} onclick={onExportSelected} class="h-7">
          <Download class="mr-1 h-3 w-3" />
          Export ({selectedCount})
        </Button>

        <Button
          variant={showDeleteConfirm ? "destructive" : "outline"}
          size="sm"
          {disabled}
          onclick={handleDeleteClick}
          class="h-7"
        >
          {#if showDeleteConfirm}
            <CheckSquare class="mr-1 h-3 w-3" />
            Confirm delete
          {:else}
            <Trash2 class="mr-1 h-3 w-3" />
            Delete ({selectedCount})
          {/if}
        </Button>
      </div>
    {/if}
  </div>

  {#if selectedCount > 0}
    <div class="text-muted-foreground mt-2 text-xs">
      {selectedCount} item{selectedCount !== 1 ? "s" : ""} selected
    </div>
  {/if}
</div>
