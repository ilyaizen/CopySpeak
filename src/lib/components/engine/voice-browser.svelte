<script lang="ts">
  import { _ } from "svelte-i18n";
  import { cn } from "#lib/utils.js";
  import {
    Drawer,
    DrawerContent,
    DrawerTitle,
    DrawerClose
  } from "#lib/components/ui/drawer/index.js";
  import { Select } from "#lib/components/ui/select/index.js";
  import { invoke } from "@tauri-apps/api/core";
  import {
    Check,
    ChevronDown,
    LoaderCircle,
    Play,
    RefreshCw,
    Search,
    Square,
    X
  } from "@lucide/svelte";
  import type { VoiceCatalogEntry, VoiceProfile } from "#lib/types";

  interface Props {
    voices: VoiceCatalogEntry[];
    profile: VoiceProfile;
    value: string;
    /** Saved label for `value`, shown when the list does not contain it. */
    valueLabel?: string | null;
    loading?: boolean;
    supportsRefresh?: boolean;
    /** Last refresh failure, shown until the next success. */
    error?: string | null;
    onselect: (id: string) => void;
    onrefresh?: () => void;
  }

  let {
    voices,
    profile,
    value,
    valueLabel = null,
    loading = false,
    supportsRefresh = false,
    error = null,
    onselect,
    onrefresh
  }: Props = $props();

  // Fish and the like put gender in tags; the Gender filter owns these words.
  const GENDER_TAGS = new Set(["male", "female"]);
  const STYLE_LIMIT = 8;
  const ROW_TAG_LIMIT = 4;

  let open = $state(false);
  let query = $state("");
  let genderFilter = $state<string | null>(null);
  let languageFilter = $state<string | null>(null);
  let styleFilter = $state<string | null>(null);
  // Keyboard-highlighted row, as an index into `filtered`; -1 = none.
  let activeRow = $state(-1);
  let playingId = $state<string | null>(null);
  let loadingId = $state<string | null>(null);
  let failedProviderIds = $state<string[]>([]);
  let previewNote = $state<"failed" | null>(null);
  let previewRequest = 0;
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
  const style = $derived(styleFilter && styleCounts.has(styleFilter) ? styleFilter : null);
  // The chosen style stays selectable even when it drops out of the top tags.
  const styleOptions = $derived(style ? [...new Set([style, ...topStyles])] : topStyles);
  const filtersActive = $derived(
    query.trim() !== "" || gender !== null || language !== null || style !== null
  );

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return voices.filter((v) => {
      if (gender && v.gender?.toLowerCase() !== gender) return false;
      if (language && v.language !== language) return false;
      if (style && !v.tags.some((t) => t.toLowerCase() === style)) return false;
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
    void styleFilter;
    const selectedIndex = filtered.findIndex((v) => v.id === value);
    visibleCount = Math.max(PAGE_SIZE, selectedIndex + 1);
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

  $effect(() => () => stopPreview());

  function distinct(values: (string | null | undefined)[]): string[] {
    const set = new Set<string>();
    for (const v of values) if (v) set.add(v);
    return [...set].sort((a, b) => a.localeCompare(b));
  }

  function cap(s: string): string {
    return s.charAt(0).toUpperCase() + s.slice(1);
  }

  function tagText(tag: string): string {
    return cap(tag.replaceAll("_", " "));
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
    query = "";
    genderFilter = null;
    languageFilter = null;
    styleFilter = null;
    open = true;
  }

  function choose(id: string) {
    stopPreview();
    onselect(id);
    open = false;
  }

  function audioMime(base64: string, extension: string): string {
    const bytes = atob(base64.slice(0, 24));
    if (bytes.startsWith("RIFF") && bytes.slice(8, 12) === "WAVE") return "audio/wav";
    if (bytes.startsWith("OggS")) return "audio/ogg";
    if (bytes.startsWith("fLaC")) return "audio/flac";
    if (
      bytes.startsWith("ID3") ||
      (bytes.charCodeAt(0) === 0xff && (bytes.charCodeAt(1) & 0xe0) === 0xe0)
    )
      return "audio/mpeg";
    if (extension === "wav") return "audio/wav";
    if (extension === "mp3") return "audio/mpeg";
    if (extension === "ogg" || extension === "opus") return "audio/ogg";
    if (extension === "flac") return "audio/flac";
    throw new Error("Unsupported preview audio format");
  }

  async function togglePreview(v: VoiceCatalogEntry) {
    const audio = audioRef;
    if (!audio) return;
    if (playingId === v.id || loadingId === v.id) {
      stopPreview();
      return;
    }
    stopPreview();
    previewNote = null;
    const request = ++previewRequest;
    const generate = !v.preview_url || failedProviderIds.includes(v.id);
    try {
      let src = v.preview_url;
      if (generate) {
        loadingId = v.id;
        const result = await invoke<{ audio_base64: string; extension: string }>("preview_voice", {
          profile: $state.snapshot(profile),
          voice: v.id
        });
        if (request !== previewRequest) return;
        src = `data:${audioMime(result.audio_base64, result.extension)};base64,${result.audio_base64}`;
      }
      if (!src || request !== previewRequest) return;
      loadingId = null;
      playingId = v.id;
      audio.src = src;
      await audio.play();
    } catch {
      if (request !== previewRequest) return;
      loadingId = null;
      playingId = null;
      previewNote = "failed";
      if (!generate) failedProviderIds = [...failedProviderIds, v.id];
    }
  }

  function stopPreview() {
    previewRequest++;
    audioRef?.pause();
    playingId = null;
    loadingId = null;
  }

  function onSearchKey(e: KeyboardEvent) {
    const last = filtered.length - 1;
    if (last < 0) return;
    if (e.key === "Enter") {
      e.preventDefault();
      const row = filtered[activeRow];
      if (!row) return;
      if (e.shiftKey) {
        if (row.preview_url && !failedProviderIds.includes(row.id)) togglePreview(row);
      } else choose(row.id);
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
      const voice = voices.find((v) => v.id === playingId);
      if (voice?.preview_url && audioRef?.src === voice.preview_url) {
        failedProviderIds = [...failedProviderIds, voice.id];
      }
      previewRequest++;
      playingId = null;
      previewNote = "failed";
    }
  }}
></audio>

<Drawer bind:open>
  <DrawerContent
    class="h-[min(85dvh,720px)] max-h-[85dvh] [&>div:first-child]:!hidden"
    onOpenAutoFocus={(e: Event) => {
      e.preventDefault();
      searchRef?.focus();
    }}
    onCloseAutoFocus={(e: Event) => {
      e.preventDefault();
      triggerRef?.focus();
    }}
  >
    <header class="flex shrink-0 flex-wrap items-center gap-2 px-5 pt-3">
      <DrawerTitle class="sr-only">{$_("voicePicker.title")}</DrawerTitle>
      <div class="border-border relative min-w-28 flex-1 rounded-sm border">
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
          aria-label={$_("voicePicker.search")}
          placeholder={$_("voicePicker.search")}
          class="w-full bg-transparent py-2 ps-9 pe-3 text-sm outline-none"
        />
      </div>
      {#if languages.length > 1}
        <Select
          options={[
            { value: "", label: $_("voicePicker.allLanguages") },
            ...languages.map((l) => ({ value: l, label: l }))
          ]}
          value={language ?? ""}
          onchange={(e) => (languageFilter = e.currentTarget.value || null)}
          aria-label={$_("voicePicker.language")}
          class="[field-sizing:content] h-9 w-auto max-w-28 ps-2 pe-7 text-xs"
        />
      {/if}
      {#if genders.length > 1}
        <Select
          options={[
            { value: "", label: $_("voicePicker.allGenders") },
            ...genders.map((g) => ({ value: g, label: cap(g) }))
          ]}
          value={gender ?? ""}
          onchange={(e) => (genderFilter = e.currentTarget.value || null)}
          aria-label={$_("voicePicker.gender")}
          class="[field-sizing:content] h-9 w-auto max-w-28 ps-2 pe-7 text-xs"
        />
      {/if}
      {#if styleOptions.length > 0}
        <Select
          options={[
            { value: "", label: $_("voicePicker.allStyles") },
            ...styleOptions.map((t) => ({ value: t, label: tagText(t) }))
          ]}
          value={style ?? ""}
          onchange={(e) => (styleFilter = e.currentTarget.value || null)}
          aria-label={$_("voicePicker.style")}
          class="[field-sizing:content] h-9 w-auto max-w-28 ps-2 pe-7 text-xs"
        />
      {/if}
      <span class="sr-only">
        {$_("voicePicker.count", { values: { shown: filtered.length, total: voices.length } })}
      </span>
      {#if supportsRefresh}
        <button
          type="button"
          aria-label={$_("voicePicker.refresh")}
          title={$_("voicePicker.refresh")}
          disabled={loading}
          onclick={() => onrefresh?.()}
          class="text-muted-foreground hover:text-foreground focus-visible:ring-ring inline-flex size-9 shrink-0 items-center justify-center rounded-sm focus-visible:ring-2 disabled:opacity-50"
        >
          <RefreshCw size={16} class={loading ? "animate-spin" : ""} />
        </button>
      {/if}
      <DrawerClose
        aria-label={$_("common.close")}
        class="text-muted-foreground hover:text-foreground focus-visible:ring-ring inline-flex size-9 items-center justify-center rounded-sm focus-visible:ring-2"
      >
        <X size={16} />
      </DrawerClose>
    </header>

    <p class="text-muted-foreground mx-5 mt-2 shrink-0 text-xs">
      {$_("voicePicker.generateCost")}
    </p>

    {#if (error && !loading) || previewNote === "failed" || loadingId}
      <div class="mx-5 mt-2 flex shrink-0 flex-col gap-1.5">
        {#if error && !loading}
          <p class="text-destructive line-clamp-2 text-xs" title={error}>
            {$_("voicePicker.refreshFailed", { values: { error } })}
          </p>
        {/if}
        {#if previewNote === "failed"}
          <p class="text-destructive text-xs" role="alert">{$_("voicePicker.previewFailed")}</p>
        {/if}
        {#if loadingId}
          <p class="text-muted-foreground text-xs" role="status">
            {$_("voicePicker.generatingPreview")}
          </p>
        {/if}
      </div>
    {/if}

    <div
      bind:this={listRef}
      id="{uid}-voices"
      role="listbox"
      class="min-h-0 flex-1 overflow-auto px-3 py-1"
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
          <div role="presentation" class="relative rounded-sm">
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
                "flex w-full min-w-0 flex-col gap-0.5 rounded-sm px-3 py-1 pe-12 text-start text-sm",
                v.id === value ? "bg-muted" : "hover:bg-muted",
                i === activeRow && "ring-ring ring-1 ring-inset"
              )}
            >
              <span class="flex w-full min-w-0 items-center gap-2">
                <Check
                  size={14}
                  class={cn("shrink-0", v.id === value ? "opacity-100" : "opacity-0")}
                />
                <span class="min-w-0 shrink truncate">{v.label}</span>
                {#if chips.shown.length > 0}
                  <span class="flex min-w-0 flex-1 items-center gap-1 overflow-hidden">
                    {#each chips.shown as c, ci (ci)}
                      <span
                        class="border-border text-muted-foreground shrink-0 rounded-sm border px-1.5 text-[0.7rem]"
                        >{c}</span
                      >
                    {/each}
                    {#if chips.more > 0}
                      <span class="text-muted-foreground text-[0.7rem]">+{chips.more}</span>
                    {/if}
                  </span>
                {/if}
                {#if v.language}
                  <span class="text-muted-foreground ms-auto shrink-0 text-xs">{v.language}</span>
                {/if}
              </span>
              {#if v.description}
                <span class="text-muted-foreground line-clamp-1 ps-6 text-xs">{v.description}</span>
              {/if}
            </button>
            <button
              type="button"
              aria-label="{playingId === v.id || loadingId === v.id
                ? $_('voicePicker.previewStop')
                : !v.preview_url || failedProviderIds.includes(v.id)
                  ? $_('voicePicker.generatePreview')
                  : $_('voicePicker.previewPlay')}: {v.label}"
              title={playingId === v.id || loadingId === v.id
                ? $_("voicePicker.previewStop")
                : !v.preview_url || failedProviderIds.includes(v.id)
                  ? $_("voicePicker.generatePreview")
                  : $_("voicePicker.previewPlay")}
              onclick={() => togglePreview(v)}
              class="text-muted-foreground hover:text-foreground hover:bg-background focus-visible:ring-ring absolute end-2 top-1/2 inline-flex size-8 -translate-y-1/2 items-center justify-center rounded-sm focus-visible:ring-2"
            >
              {#if loadingId === v.id}
                <LoaderCircle size={16} class="animate-spin" />
              {:else if playingId === v.id}
                <Square size={12} />
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
  </DrawerContent>
</Drawer>
