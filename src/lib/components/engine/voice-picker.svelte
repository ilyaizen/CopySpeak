<script lang="ts">
  import { cn, portal } from "$lib/utils.js";
  import { Check, ChevronDown, RefreshCw, Search } from "@lucide/svelte";
  import type { VoiceCatalogEntry } from "$lib/types";

  let {
    voices,
    value,
    loading = false,
    supportsRefresh = false,
    onselect,
    onrefresh
  }: {
    voices: VoiceCatalogEntry[];
    value: string;
    loading?: boolean;
    supportsRefresh?: boolean;
    onselect: (id: string) => void;
    onrefresh?: () => void;
  } = $props();

  let open = $state(false);
  let query = $state("");
  let triggerRef = $state<HTMLButtonElement | null>(null);
  let panelRef = $state<HTMLDivElement | null>(null);
  let searchRef = $state<HTMLInputElement | null>(null);

  const selected = $derived(voices.find((v) => v.id === value) ?? null);

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

  const GROUP_ORDER: Record<string, number> = { Female: 0, Male: 1 };

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
    if (entries.length <= 1) return [] as { key: string; items: VoiceCatalogEntry[] }[];
    entries.sort((a, b) => {
      const oa = GROUP_ORDER[a[0]] ?? 999;
      const ob = GROUP_ORDER[b[0]] ?? 999;
      return oa === ob ? a[0].localeCompare(b[0]) : oa - ob;
    });
    return entries.map(([key, items]) => ({ key, items }));
  });

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

  function place() {
    if (!triggerRef || !panelRef) return;
    const r = triggerRef.getBoundingClientRect();
    const panelW = Math.max(r.width, 288);
    const left = Math.max(8, Math.min(r.left, window.innerWidth - panelW - 8));
    const ph = panelRef.offsetHeight;
    const below = r.bottom + 6;
    const above = r.top - ph - 6;
    const top = below + ph > window.innerHeight && r.top > ph + 16 ? above : below;
    panelRef.style.width = `${panelW}px`;
    panelRef.style.left = `${left}px`;
    panelRef.style.top = `${Math.max(8, top)}px`;
  }

  function onDocMouseDown(e: MouseEvent) {
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
    place();
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

  function choose(id: string) {
    onselect(id);
    open = false;
    query = "";
  }
</script>

<button
  bind:this={triggerRef}
  type="button"
  onclick={() => (open = !open)}
  class={cn(
    "border-input bg-background focus-visible:ring-ring inline-flex h-9 w-56 items-center justify-between gap-2 rounded-sm border px-3 text-sm shadow-xs focus-visible:ring-1 focus-visible:outline-none",
    open && "ring-ring ring-1"
  )}
>
  <span class="truncate text-left">
    {#if selected}
      <span>{selected.label}</span>
    {:else}
      <span class="text-muted-foreground">Select voice…</span>
    {/if}
  </span>
  <ChevronDown size={14} class="text-muted-foreground shrink-0" />
</button>

{#if open}
  <div bind:this={panelRef} use:portal class="fixed z-50" role="listbox">
    <div
      class="border-border bg-background text-foreground rounded-md border shadow-lg"
    >
      <div class="border-border relative border-b">
        <Search
          size={14}
          class="text-muted-foreground pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2"
        />
        <input
          bind:this={searchRef}
          bind:value={query}
          placeholder="Search voices…"
          class="w-full rounded-t-md py-2 pr-3 pl-8 text-sm outline-none"
        />
      </div>

      <div class="max-h-72 overflow-auto py-1">
        {#if filtered.length === 0}
          <p class="text-muted-foreground px-3 py-2 text-sm">No voices match “{query}”.</p>
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
        <div class="border-border border-t p-1.5">
          <button
            type="button"
            onclick={() => onrefresh?.()}
            disabled={loading}
            class="text-muted-foreground hover:text-foreground inline-flex w-full items-center justify-center gap-1.5 rounded-sm py-1.5 text-xs disabled:opacity-50"
          >
            <RefreshCw size={13} class={loading ? "animate-spin" : ""} />
            {loading ? "Refreshing…" : "Refresh from API"}
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}

{#snippet row(v: VoiceCatalogEntry)}
  <button
    type="button"
    onclick={() => choose(v.id)}
    title={v.description ?? v.label}
    class={cn(
      "flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm",
      v.id === value ? "bg-muted" : "hover:bg-muted"
    )}
  >
    <Check size={14} class={cn("shrink-0", v.id === value ? "opacity-100" : "opacity-0")} />
    <span class="flex-1 truncate">{v.label}</span>
    {#if meta(v)}
      <span class="text-muted-foreground shrink-0 text-xs">{meta(v)}</span>
    {/if}
  </button>
{/snippet}
