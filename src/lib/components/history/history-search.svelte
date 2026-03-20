<script lang="ts">
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Search, Calendar, X, Filter } from "@lucide/svelte";
  import type { HistoryFilters, HistoryItem } from "$lib/types";

  let props = $props<{
    items: HistoryItem[];
    onFiltersChange?: (filters: HistoryFilters) => void;
    class?: string;
  }>();

  // svelte-ignore state_referenced_locally
  let { onFiltersChange, class: className } = props;

  let searchText = $state("");
  let dateFrom = $state("");
  let dateTo = $state("");

  const filters = $derived<HistoryFilters>({
    search_text: searchText || undefined,
    date_from: dateFrom ? new Date(dateFrom).getTime() : undefined,
    date_to: dateTo ? new Date(dateTo).getTime() + 86399999 : undefined
  });

  const hasFilters = $derived(!!searchText || !!dateFrom || !!dateTo);

  $effect(() => {
    onFiltersChange?.(filters);
  });

  function resetFilters() {
    searchText = "";
    dateFrom = "";
    dateTo = "";
  }

  function handleSearchKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      searchText = "";
    }
  }

  const dateToday = new Date().toISOString().split("T")[0];
</script>

<div class={className}>
  <div class="border-border bg-card rounded-lg border p-4 shadow-sm">
    <div class="mb-4 flex items-center justify-between">
      <div class="flex items-center gap-2">
        <Filter class="text-muted-foreground h-4 w-4" />
        <h3 class="text-card-foreground text-sm font-medium">Filters</h3>
      </div>
      {#if hasFilters}
        <Button variant="ghost" size="sm" onclick={resetFilters} class="h-7 px-2 text-xs">
          <X class="mr-1 h-3 w-3" />
          Clear
        </Button>
      {/if}
    </div>

    <div class="space-y-4">
      <div class="space-y-2">
        <Label for="search-text" class="flex items-center gap-2">
          <Search class="h-3 w-3" />
          Search Text
        </Label>
        <div class="relative">
          <Input
            id="search-text"
            bind:value={searchText}
            placeholder="Search in text content..."
            onkeydown={handleSearchKeyDown}
            class="pr-8"
          />
          {#if searchText}
            <Button
              variant="ghost"
              size="icon-sm"
              class="absolute top-1 right-1 h-7 w-7"
              onclick={() => (searchText = "")}
            >
              <X class="h-3 w-3" />
            </Button>
          {/if}
        </div>
      </div>

      <div class="grid grid-cols-1 gap-4 md:grid-cols-2">
        <div class="space-y-2">
          <Label for="date-from" class="flex items-center gap-2">
            <Calendar class="h-3 w-3" />
            Date From
          </Label>
          <div class="relative">
            <Input id="date-from" type="date" bind:value={dateFrom} max={dateTo || dateToday} />
            {#if dateFrom}
              <Button
                variant="ghost"
                size="icon-sm"
                class="absolute top-1 right-1 h-7 w-7"
                onclick={() => (dateFrom = "")}
              >
                <X class="h-3 w-3" />
              </Button>
            {/if}
          </div>
        </div>

        <div class="space-y-2">
          <Label for="date-to" class="flex items-center gap-2">
            <Calendar class="h-3 w-3" />
            Date To
          </Label>
          <div class="relative">
            <Input id="date-to" type="date" bind:value={dateTo} min={dateFrom} max={dateToday} />
            {#if dateTo}
              <Button
                variant="ghost"
                size="icon-sm"
                class="absolute top-1 right-1 h-7 w-7"
                onclick={() => (dateTo = "")}
              >
                <X class="h-3 w-3" />
              </Button>
            {/if}
          </div>
        </div>
      </div>

      {#if hasFilters}
        <div class="border-border border-t pt-3">
          <div class="text-muted-foreground space-y-1 text-xs">
            <p>Active filters:</p>
            <ul class="m-0 flex list-none flex-wrap gap-2 p-0">
              {#if searchText}
                <li
                  class="bg-secondary text-secondary-foreground inline-flex items-center gap-1 rounded px-2 py-1 text-xs"
                >
                  Text: "{searchText.slice(0, 20)}{searchText.length > 20 ? "..." : ""}"
                </li>
              {/if}
              {#if dateFrom}
                <li
                  class="bg-secondary text-secondary-foreground inline-flex items-center gap-1 rounded px-2 py-1 text-xs"
                >
                  From: {dateFrom}
                </li>
              {/if}
              {#if dateTo}
                <li
                  class="bg-secondary text-secondary-foreground inline-flex items-center gap-1 rounded px-2 py-1 text-xs"
                >
                  To: {dateTo}
                </li>
              {/if}
            </ul>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>
