<script lang="ts">
  import { _ } from "svelte-i18n";
  import { cn } from "#lib/utils.js";
  import { Sheet, SheetContent, SheetTitle } from "#lib/components/ui/sheet/index.js";
  import { Check, ChevronDown, Pencil, Play, RefreshCw, Search, Square } from "@lucide/svelte";
  import type { VoiceCatalogEntry } from "#lib/types";

  interface Props {
    voices: VoiceCatalogEntry[];
    value: string;
    /** Saved label for `value`, shown when the list does not contain it. */
    valueLabel?: string | null;
    loading?: boolean;
    supportsRefresh?: boolean;
    /** Last refresh failure, shown until the next success. */
    error?: string | null;
    onselect: (id: string) => void;
    onrefresh?: () => void;
    onmanual?: () => void;
  }

  let {
    voices,
    value,
    valueLabel = null,
    loading = false,
    supportsRefresh = false,
    error = null,
    onselect,
    onrefresh,
    onmanual
  }: Props = $props();

  // Fish and the like put gender in tags; the Gender filter owns these words.
  const GENDER_TAGS = new Set(["male", "female"]);
  const STYLE_LIMIT = 8;
  const ROW_TAG_LIMIT = 4;

  type PreviewNote = "noPreview" | "failed";

  let open = $state(false);
  let query = $state("");
  let genderFilter = $state<string | null>(null);
  let languageFilter = $state<string | null>(null);
  let styleFilters = $state<string[]>([]);
  // Keyboard-highlighted row, as an index into `filtered`; -1 = none.
  let activeRow = $state(-1);
  let playingId = $state<string | null>(null);
  let previewNote = $state<PreviewNote | null>(null);
  let triggerRef = $state<HTMLButtonElement | null>(null);
  let searchRef = $state<HTMLInputElement | null>(null);
  let listRef = $state<HTMLDivElement | null>(null);
  let audioRef = $state<HTMLAudioElement | null>(null);
  const uid = $props.id();

  const selected = $derived(voices.find((v) => v.id === value) ?? null);
  // A saved voice the current list lacks (a custom/cloned id, or a list from
  // before a refresh): keep showing it instead of the "Select voice" placeholder.
  const missingLabel = $derived(
    !selected && value.trim() !== "" ? valueLabel?.trim() || value : null
  );

  const genders = $derived(distinct(voices.map((v) => v.gender?.toLowerCase())));
  const languages = $derived(distinct(voices.map((v) => v.language)));
  const styleCounts = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const v of voices) {
      for (const raw of v.tags) {
        const tag = raw.toLowerCase();
        if (GENDER_TAGS.has(tag)) continue;
        counts.set(tag, (counts.get(tag) ?? 0) + 1);
      }
    }
    return counts;
  });
  const topStyles = $derived(
    [...styleCounts.entries()]
      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
      .slice(0, STYLE_LIMIT)
      .map(([tag]) => tag)
  );

  // A filter whose value left the list (after a refresh) is ignored rather
  // than left in place to hide every row.
  const gender = $derived(genderFilter && genders.includes(genderFilter) ? genderFilter : null);
  const language = $derived(
    languageFilter && languages.includes(languageFilter) ? languageFilter : null
  );
  const activeStyles = $derived(styleFilters.filter((s) => styleCounts.has(s)));
  const styleChips = $derived([...new Set([...topStyles, ...activeStyles])]);
  const filtersActive = $derived(
    query.trim() !== "" || gender !== null || language !== null || activeStyles.length > 0
  );

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return voices.filter((v) => {
      if (gender && v.gender?.toLowerCase() !== gender) return false;
      if (language && v.language !== language) return false;
      if (activeStyles.some((s) => !v.tags.some((t) => t.toLowerCase() === s))) return false;
      if (!q) return true;
      return [v.label, v.id, v.gender, v.language, v.description, ...v.tags]
        .filter((s): s is string => Boolean(s))
        .some((s) => s.toLowerCase().includes(q));
    });
  });

  const optionId = (i: number) => `${uid}-voice-${i}`;

  // Long provider lists (1,000+ voices) render one page at a time; "Show more"
  // and keyboard navigation past the rendered rows reveal the next page.
  const PAGE_SIZE = 100;
  let visibleCount = $state(PAGE_SIZE);
  const visible = $derived(filtered.slice(0, visibleCount));
  const hiddenCount = $derived(Math.max(filtered.length - visibleCount, 0));

  // Each open and each filter change restarts at one page.
  $effect(() => {
    if (!open) return;
    void query;
    void gender;
    void language;
    void styleFilters;
    visibleCount = PAGE_SIZE;
  });

  // Each open and each filter change starts at the saved voice, else the top row.
  $effect(() => {
    if (!open) return;
    const i = filtered.findIndex((v) => v.id === value);
    activeRow = i >= 0 ? i : filtered.length > 0 ? 0 : -1;
  });

  $effect(() => {
    if (!open || activeRow < 0) return;
    listRef
      ?.querySelector(`#${CSS.escape(optionId(activeRow))}`)
      ?.scrollIntoView({ block: "nearest" });
  });

  // Closing by any route (Escape, overlay, X, choosing a row) stops the preview.
  $effect(() => {
    if (!open) stopPreview();
  });

  // Unmounting while a preview plays must not leave it running.
  $effect(() => () => audioRef?.pause());

  function distinct(values: (string | null | undefined)[]): string[] {
    const set = new Set<string>();
    for (const v of values) if (v) set.add(v);
    return [...set].sort((a, b) => a.localeCompare(b));
  }

  function cap(s: string): string {
    return s.charAt(0).toUpperCase() + s.slice(1);
  }

  function tagText(tag: string): string {
    return tag.replaceAll("_", " ");
  }

  // Gender first, then tags that are not already shown as gender.
  function rowChips(v: VoiceCatalogEntry) {
    const chips: string[] = [];
    if (v.gender) chips.push(cap(v.gender.toLowerCase()));
    for (const tag of v.tags) {
      if (GENDER_TAGS.has(tag.toLowerCase())) continue;
      chips.push(tagText(tag));
    }
    return {
      shown: chips.slice(0, ROW_TAG_LIMIT),
      more: Math.max(chips.length - ROW_TAG_LIMIT, 0)
    };
  }

  function openSheet() {
    closingForManual = false;
    query = "";
    genderFilter = null;
    languageFilter = null;
    styleFilters = [];
    open = true;
  }

  function toggleStyle(tag: string) {
    styleFilters = activeStyles.includes(tag)
      ? activeStyles.filter((s) => s !== tag)
      : [...activeStyles, tag];
  }

  function choose(id: string) {
    stopPreview();
    onselect(id);
    open = false;
  }

  // Set while closing for "Custom / manual id": the parent takes focus, so the
  // sheet's close must not hand it back to the trigger.
  let closingForManual = false;

  function chooseManual() {
    closingForManual = true;
    open = false;
    onmanual?.();
  }

  function togglePreview(v: VoiceCatalogEntry) {
    if (!audioRef) return;
    if (!v.preview_url) {
      previewNote = "noPreview";
      return;
    }
    if (playingId === v.id) {
      stopPreview();
      return;
    }
    previewNote = null;
    playingId = v.id;
    audioRef.src = v.preview_url;
    audioRef.play().catch(() => {
      // Starting another preview interrupts this one and rejects it too;
      // only the preview still current may report a failure.
      if (playingId === v.id) {
        playingId = null;
        previewNote = "failed";
      }
    });
  }

  function stopPreview() {
    audioRef?.pause();
    playingId = null;
  }

  function onSearchKey(e: KeyboardEvent) {
    const last = filtered.length - 1;
    if (last < 0) return;
    if (e.key === "Enter") {
      e.preventDefault();
      const row = filtered[activeRow];
      if (!row) return;
      if (e.shiftKey) togglePreview(row);
      else choose(row.id);
      return;
    }
    let next = activeRow;
    if (e.key === "ArrowDown") next = activeRow < last ? activeRow + 1 : last;
    else if (e.key === "ArrowUp") next = activeRow > 0 ? activeRow - 1 : 0;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = last;
    else return;
    e.preventDefault();
    // Reaching past the rendered rows reveals the next page; End reveals all.
    if (next >= visibleCount) {
      visibleCount =
        e.key === "End" ? filtered.length : Math.min(visibleCount + PAGE_SIZE, filtered.length);
    }
    activeRow = next;
  }
</script>

{#snippet chip(label: string, pressed: boolean, onpick: () => void)}
  <button
    type="button"
    aria-pressed={pressed}
    onclick={onpick}
    class={cn(
      "rounded-full border px-2.5 py-0.5 text-xs transition-colors",
      pressed
        ? "border-foreground bg-foreground text-background"
        : "border-border text-muted-foreground hover:text-foreground"
    )}
  >
    {label}
  </button>
{/snippet}

<button
  bind:this={triggerRef}
  type="button"
  aria-haspopup="dialog"
  aria-expanded={open}
  onclick={openSheet}
  class={cn(
    "border-input bg-background focus-visible:ring-ring inline-flex h-9 w-56 items-center justify-between gap-2 rounded-sm border px-3 text-sm shadow-xs focus-visible:ring-1 focus-visible:outline-none",
    open && "ring-ring ring-1"
  )}
>
  <span class="truncate text-start" title={missingLabel ? value : undefined}>
    {#if selected}
      <span>{selected.label}</span>
    {:else if missingLabel}
      <span>{missingLabel}</span>
    {:else}
      <span class="text-muted-foreground">{$_("voicePicker.select")}</span>
    {/if}
  </span>
  <ChevronDown size={14} class="text-muted-foreground shrink-0" />
</button>

<!-- One element for every row, so a second preview stops the first. -->
<audio
  bind:this={audioRef}
  preload="none"
  onended={() => (playingId = null)}
  onerror={() => {
    if (playingId) {
      playingId = null;
      previewNote = "failed";
    }
  }}
></audio>

<Sheet bind:open>
  <SheetContent
    onOpenAutoFocus={(e: Event) => {
      e.preventDefault();
      searchRef?.focus();
    }}
    onCloseAutoFocus={(e: Event) => {
      e.preventDefault();
      if (!closingForManual) triggerRef?.focus();
      closingForManual = false;
    }}
  >
    <header class="border-border flex shrink-0 items-baseline gap-3 border-b px-4 py-3 pr-12">
      <SheetTitle class="flex-1 text-sm font-semibold">{$_("voicePicker.title")}</SheetTitle>
      <span class="text-muted-foreground text-xs tabular-nums">
        {$_("voicePicker.count", { values: { shown: filtered.length, total: voices.length } })}
      </span>
    </header>

    <div class="border-border relative shrink-0 border-b">
      <Search
        size={14}
        class="text-muted-foreground pointer-events-none absolute start-3 top-1/2 -translate-y-1/2"
      />
      <input
        bind:this={searchRef}
        bind:value={query}
        onkeydown={onSearchKey}
        role="combobox"
        aria-expanded="true"
        aria-controls="{uid}-voices"
        aria-autocomplete="list"
        aria-activedescendant={activeRow >= 0 ? optionId(activeRow) : undefined}
        placeholder={$_("voicePicker.search")}
        class="w-full bg-transparent py-2.5 ps-9 pe-3 text-sm outline-none"
      />
    </div>

    <div class="border-border flex shrink-0 flex-col gap-2 border-b px-4 py-3">
      {#if genders.length > 1}
        <div class="flex flex-wrap items-center gap-1.5">
          <span class="text-muted-foreground w-20 shrink-0 text-xs">{$_("voicePicker.gender")}</span
          >
          {@render chip($_("voicePicker.all"), gender === null, () => (genderFilter = null))}
          {#each genders as g (g)}
            {@render chip(cap(g), gender === g, () => (genderFilter = g))}
          {/each}
        </div>
      {/if}
      {#if languages.length > 1}
        <div class="flex flex-wrap items-center gap-1.5">
          <span class="text-muted-foreground w-20 shrink-0 text-xs"
            >{$_("voicePicker.language")}</span
          >
          {@render chip($_("voicePicker.all"), language === null, () => (languageFilter = null))}
          {#each languages as l (l)}
            {@render chip(l, language === l, () => (languageFilter = l))}
          {/each}
        </div>
      {/if}
      {#if styleChips.length > 0}
        <div class="flex flex-wrap items-center gap-1.5">
          <span class="text-muted-foreground w-20 shrink-0 text-xs">{$_("voicePicker.style")}</span>
          {#each styleChips as tag (tag)}
            {@render chip(tagText(tag), activeStyles.includes(tag), () => toggleStyle(tag))}
          {/each}
        </div>
      {/if}
    </div>

    <div
      bind:this={listRef}
      id="{uid}-voices"
      role="listbox"
      class="min-h-0 flex-1 overflow-auto py-1"
    >
      {#if missingLabel && !filtersActive}
        <div class="bg-muted flex w-full items-center gap-2 px-4 py-2 text-sm" title={value}>
          <Check size={14} class="shrink-0" />
          <span class="flex-1 truncate">{missingLabel}</span>
          <span class="text-muted-foreground shrink-0 text-xs">{$_("voicePicker.notInList")}</span>
        </div>
      {/if}
      {#if filtered.length === 0}
        <p class="text-muted-foreground px-4 py-3 text-sm">
          {query.trim()
            ? $_("voicePicker.noMatch", { values: { query } })
            : $_("voicePicker.noFilterMatch")}
        </p>
      {:else}
        {#each visible as v, i (v.id)}
          {@const chips = rowChips(v)}
          <!-- The preview button sits beside the option, not inside it: a
               listbox option must not contain interactive children. -->
          <div role="presentation" class="relative">
            <button
              type="button"
              role="option"
              id={optionId(i)}
              aria-selected={v.id === value}
              tabindex="-1"
              title={v.description ?? v.label}
              onclick={() => choose(v.id)}
              onmousemove={() => (activeRow = i)}
              class={cn(
                "flex w-full min-w-0 flex-col gap-1 px-4 py-2 pe-12 text-start text-sm",
                v.id === value ? "bg-muted" : "hover:bg-muted",
                i === activeRow && "ring-ring ring-1 ring-inset"
              )}
            >
              <span class="flex w-full min-w-0 items-center gap-2">
                <Check
                  size={14}
                  class={cn("shrink-0", v.id === value ? "opacity-100" : "opacity-0")}
                />
                <span class="flex-1 truncate">{v.label}</span>
                {#if v.language}
                  <span class="text-muted-foreground shrink-0 text-xs">{v.language}</span>
                {/if}
              </span>
              {#if chips.shown.length > 0}
                <span class="flex flex-wrap items-center gap-1 ps-6">
                  {#each chips.shown as c, ci (ci)}
                    <span
                      class="border-border text-muted-foreground rounded-sm border px-1.5 text-[0.7rem]"
                      >{c}</span
                    >
                  {/each}
                  {#if chips.more > 0}
                    <span class="text-muted-foreground text-[0.7rem]">+{chips.more}</span>
                  {/if}
                </span>
              {/if}
            </button>
            <button
              type="button"
              aria-hidden="true"
              tabindex="-1"
              disabled={!v.preview_url}
              title={playingId === v.id
                ? $_("voicePicker.previewStop")
                : $_("voicePicker.previewPlay")}
              onclick={() => togglePreview(v)}
              class="text-muted-foreground hover:text-foreground hover:bg-background absolute end-2 top-1/2 inline-flex size-7 -translate-y-1/2 items-center justify-center rounded-sm disabled:opacity-30"
            >
              {#if playingId === v.id}
                <Square size={12} fill="currentColor" />
              {:else}
                <Play size={12} />
              {/if}
            </button>
          </div>
        {/each}
        {#if hiddenCount > 0}
          <button
            type="button"
            onclick={() => (visibleCount = Math.min(visibleCount + PAGE_SIZE, filtered.length))}
            class="text-muted-foreground hover:text-foreground mx-auto my-1 block rounded-sm px-3 py-2 text-xs"
          >
            {$_("voicePicker.showMore", { values: { count: hiddenCount } })}
          </button>
        {/if}
      {/if}
    </div>

    <footer class="border-border flex shrink-0 flex-col gap-1.5 border-t px-4 py-3">
      <div class="flex items-center justify-between gap-2">
        {#if onmanual}
          <button
            type="button"
            onclick={chooseManual}
            class="text-muted-foreground hover:text-foreground inline-flex items-center gap-1.5 rounded-sm text-xs"
          >
            <Pencil size={13} class="shrink-0" />
            {$_("voicePicker.customId")}
          </button>
        {:else}
          <span></span>
        {/if}
        {#if supportsRefresh}
          <button
            type="button"
            onclick={() => onrefresh?.()}
            disabled={loading}
            class="text-muted-foreground hover:text-foreground inline-flex shrink-0 items-center gap-1.5 rounded-sm px-1.5 py-1 text-xs disabled:opacity-50"
          >
            <RefreshCw size={13} class={loading ? "animate-spin" : ""} />
            {$_("voicePicker.refresh")}
          </button>
        {/if}
      </div>
      {#if error && !loading}
        <p class="text-destructive line-clamp-2 text-xs" title={error}>
          {$_("voicePicker.refreshFailed", { values: { error } })}
        </p>
      {/if}
      {#if previewNote === "noPreview"}
        <p class="text-muted-foreground text-xs">{$_("voicePicker.noPreview")}</p>
      {:else if previewNote === "failed"}
        <p class="text-destructive text-xs">{$_("voicePicker.previewFailed")}</p>
      {/if}
      <p class="text-muted-foreground text-[0.7rem]">{$_("voicePicker.shiftEnterHint")}</p>
    </footer>
  </SheetContent>
</Sheet>
