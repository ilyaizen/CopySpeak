<script lang="ts">
  import { _ } from "svelte-i18n";
  import { cn, portal } from "#lib/utils.js";
  import { Check, ChevronDown, Pencil, RefreshCw, Search } from "@lucide/svelte";
  import type { VoiceCatalogEntry } from "#lib/types";

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
  }: {
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
  } = $props();

  let open = $state(false);
  let query = $state("");
  let triggerRef = $state<HTMLButtonElement | null>(null);
  let panelRef = $state<HTMLDivElement | null>(null);
  let searchRef = $state<HTMLInputElement | null>(null);
  let listRef = $state<HTMLDivElement | null>(null);
  // Keyboard-highlighted row, as an index into `ordered`; -1 = none.
  let activeRow = $state(-1);
  const uid = $props.id();

  const selected = $derived(voices.find((v) => v.id === value) ?? null);
  // A saved voice the current list lacks (a custom/cloned id, or a list from
  // before a refresh): keep showing it instead of the "Select voice" placeholder.
  const missingLabel = $derived(
    !selected && value.trim() !== "" ? valueLabel?.trim() || value : null
  );

  // Grouping priority: region locale (Edge en-US/en-GB/…) wins over gender
  // (OpenAI/Google/Cartesia/ElevenLabs) — region is the more useful cluster for
  // Edge, so gender shows as per-row meta instead of becoming the header.
  const useLocale = $derived(voices.some((v) => (v.language ?? "").includes("-")));
  const useGender = $derived(!useLocale && voices.some((v) => v.gender));
  const useLanguage = $derived(!useLocale && !useGender && voices.some((v) => v.language));

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return voices;
    return voices.filter((v) =>
      [v.label, v.id, v.gender, v.language, v.description]
        .filter((s): s is string => Boolean(s))
        .some((s) => s.toLowerCase().includes(q))
    );
  });

  interface GroupOrderMap {
    readonly [group: string]: number;
  }
  const GROUP_ORDER: GroupOrderMap = { Female: 0, Male: 1 };

  const groups = $derived.by(() => {
    const map = new Map<string, VoiceCatalogEntry[]>();
    for (const v of filtered) {
      const key = useLocale
        ? (v.language ?? "Other")
        : useGender
          ? cap(v.gender ?? "Other")
          : useLanguage
            ? (v.language ?? "Other")
            : "All";
      const arr = map.get(key);
      if (arr) arr.push(v);
      else map.set(key, [v]);
    }
    const entries = [...map.entries()];
    if (entries.length <= 1) return [];
    entries.sort((a, b) => {
      const oa = GROUP_ORDER[a[0]] ?? 999;
      const ob = GROUP_ORDER[b[0]] ?? 999;
      return oa === ob ? a[0].localeCompare(b[0]) : oa - ob;
    });
    return entries.map(([key, items]) => ({ key, items }));
  });

  // Rows in on-screen order (grouping reorders them), for keyboard navigation.
  const ordered = $derived(groups.length === 0 ? filtered : groups.flatMap((g) => g.items));
  const rowIndex = $derived(new Map(ordered.map((v, i) => [v.id, i])));
  const optionId = (i: number) => `${uid}-voice-${i}`;

  // Each new filter (and each open) starts at the saved voice, else the top row.
  $effect(() => {
    if (!open) return;
    void query;
    const i = ordered.findIndex((v) => v.id === value);
    activeRow = i >= 0 ? i : ordered.length > 0 ? 0 : -1;
  });

  $effect(() => {
    if (!open || activeRow < 0) return;
    listRef
      ?.querySelector(`#${CSS.escape(optionId(activeRow))}`)
      ?.scrollIntoView({ block: "nearest" });
  });

  function onSearchKey(e: KeyboardEvent) {
    const last = ordered.length - 1;
    if (last < 0) return;
    let next = activeRow;
    if (e.key === "ArrowDown") next = activeRow < last ? activeRow + 1 : last;
    else if (e.key === "ArrowUp") next = activeRow > 0 ? activeRow - 1 : 0;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = last;
    else if (e.key === "Enter") {
      e.preventDefault();
      if (activeRow >= 0) choose(ordered[activeRow].id);
      return;
    } else return;
    e.preventDefault();
    activeRow = next;
  }

  function cap(s: string): string {
    return s.charAt(0).toUpperCase() + s.slice(1);
  }

  function meta(v: VoiceCatalogEntry): string {
    // Suppress whichever field is already the group key to avoid the group
    // header being repeated on every row.
    const genderPart = !useGender && v.gender ? cap(v.gender) : null;
    const langPart = !useLanguage && !useLocale && v.language ? v.language : null;
    return [genderPart, langPart].filter(Boolean).join(" · ");
  }

  const GAP = 6;
  const MARGIN = 8;

  // Open on the side with room. When neither side fits the whole panel, cap
  // its height to the larger side so it never runs past the window edge; the
  // list scrolls inside it.
  function place() {
    if (!triggerRef || !panelRef) return;
    const r = triggerRef.getBoundingClientRect();
    const panelW = Math.min(Math.max(r.width, 288), window.innerWidth - MARGIN * 2);
    const left = Math.max(MARGIN, Math.min(r.left, window.innerWidth - panelW - MARGIN));
    panelRef.style.maxHeight = "";
    const natural = panelRef.offsetHeight;
    const below = window.innerHeight - r.bottom - GAP - MARGIN;
    const above = r.top - GAP - MARGIN;
    const openUp = natural > below && above > below;
    const room = Math.max(openUp ? above : below, 0);
    const height = Math.min(natural, room);
    panelRef.style.width = `${panelW}px`;
    panelRef.style.left = `${left}px`;
    panelRef.style.maxHeight = `${room}px`;
    panelRef.style.top = `${openUp ? r.top - GAP - height : r.bottom + GAP}px`;
  }

  function onDocMouseDown(e: MouseEvent) {
    // SAFETY: every dispatched DOM event target is a Node; EventTarget is only the static type.
    const t = e.target as Node | null;
    if (!t) return;
    if (triggerRef?.contains(t) || panelRef?.contains(t)) return;
    open = false;
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") open = false;
  }

  $effect(() => {
    if (!open) return;
    searchRef?.focus();
    window.addEventListener("scroll", place, true);
    window.addEventListener("resize", place);
    document.addEventListener("mousedown", onDocMouseDown);
    document.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("scroll", place, true);
      window.removeEventListener("resize", place);
      document.removeEventListener("mousedown", onDocMouseDown);
      document.removeEventListener("keydown", onKey);
    };
  });

  // The panel's height changes while open (search filter, a refresh landing,
  // an error line): re-place so it stays inside the window and, when opened
  // upward, anchored to the trigger.
  $effect(() => {
    if (!open) return;
    void filtered.length;
    void loading;
    void error;
    void missingLabel;
    place();
  });

  function choose(id: string) {
    onselect(id);
    open = false;
    query = "";
    triggerRef?.focus();
  }
</script>

<button
  bind:this={triggerRef}
  type="button"
  aria-haspopup="listbox"
  aria-expanded={open}
  onclick={() => (open = !open)}
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

{#if open}
  <div
    bind:this={panelRef}
    use:portal
    class="border-border bg-background text-foreground fixed z-50 flex flex-col overflow-hidden rounded-md border shadow-lg"
  >
    <div class="border-border relative shrink-0 border-b">
      <Search
        size={14}
        class="text-muted-foreground pointer-events-none absolute start-2.5 top-1/2 -translate-y-1/2"
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
        class="w-full rounded-t-md py-2 ps-8 pe-3 text-sm outline-none"
      />
    </div>

    <div
      bind:this={listRef}
      id="{uid}-voices"
      role="listbox"
      class="max-h-72 min-h-0 flex-1 overflow-auto py-1"
    >
      {#if missingLabel && !query.trim()}
        <div class="bg-muted flex w-full items-center gap-2 px-3 py-1.5 text-sm" title={value}>
          <Check size={14} class="shrink-0" />
          <span class="flex-1 truncate">{missingLabel}</span>
          <span class="text-muted-foreground shrink-0 text-xs">{$_("voicePicker.notInList")}</span>
        </div>
      {/if}
      {#if onmanual}
        <button
          type="button"
          onclick={() => {
            onmanual();
            open = false;
            query = "";
          }}
          class="text-muted-foreground hover:bg-muted hover:text-foreground flex w-full items-center gap-2 px-3 py-1.5 text-start text-sm"
        >
          <Pencil size={14} class="shrink-0" />
          <span class="flex-1 truncate">{$_("voicePicker.customId")}</span>
        </button>
      {/if}
      {#if filtered.length === 0}
        <p class="text-muted-foreground px-3 py-2 text-sm">
          {$_("voicePicker.noMatch", { values: { query } })}
        </p>
      {:else if groups.length === 0}
        {#each filtered as v (v.id)}
          {@render row(v)}
        {/each}
      {:else}
        {#each groups as g (g.key)}
          <div
            class="text-muted-foreground px-3 py-1 text-xs font-semibold tracking-wide uppercase"
          >
            {g.key}
          </div>
          {#each g.items as v (v.id)}
            {@render row(v)}
          {/each}
        {/each}
      {/if}
    </div>

    {#if supportsRefresh}
      <div class="border-border shrink-0 border-t px-3 py-1.5">
        <div class="flex items-center justify-end gap-2">
          <button
            type="button"
            onclick={() => onrefresh?.()}
            disabled={loading}
            class="text-muted-foreground hover:text-foreground inline-flex shrink-0 items-center gap-1.5 rounded-sm px-1.5 py-1 text-xs disabled:opacity-50"
          >
            <RefreshCw size={13} class={loading ? "animate-spin" : ""} />
            {$_("voicePicker.refresh")}
          </button>
        </div>
        {#if error && !loading}
          <p class="text-destructive mt-1 line-clamp-2 text-xs" title={error}>
            {$_("voicePicker.refreshFailed", { values: { error } })}
          </p>
        {/if}
      </div>
    {/if}
  </div>
{/if}

{#snippet row(v: VoiceCatalogEntry)}
  {@const i = rowIndex.get(v.id) ?? -1}
  <button
    type="button"
    id={optionId(i)}
    role="option"
    aria-selected={v.id === value}
    tabindex="-1"
    onclick={() => choose(v.id)}
    onmousemove={() => (activeRow = i)}
    title={v.description ?? v.label}
    class={cn(
      "flex w-full items-center gap-2 px-3 py-1.5 text-start text-sm",
      v.id === value ? "bg-muted" : "hover:bg-muted",
      i === activeRow && "ring-ring ring-1 ring-inset"
    )}
  >
    <Check size={14} class={cn("shrink-0", v.id === value ? "opacity-100" : "opacity-0")} />
    <span class="flex-1 truncate">{v.label}</span>
    {#if meta(v)}
      <span class="text-muted-foreground shrink-0 text-xs">{meta(v)}</span>
    {/if}
  </button>
{/snippet}
