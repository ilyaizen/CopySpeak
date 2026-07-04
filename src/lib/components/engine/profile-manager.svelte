<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { Slider } from "$lib/components/ui/slider/index.js";
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { invoke } from "@tauri-apps/api/core";
  import { Copy, Trash2, Download, Upload, RefreshCw, ExternalLink } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import type {
    AppConfig,
    EngineCatalogEntry,
    EngineOptionDescriptor,
    TtsEngine,
    EffectId,
    VoiceCatalogEntry,
    VoiceProfile
  } from "$lib/types";

  let { localConfig = $bindable() } = $props<{ localConfig: AppConfig }>();

  const ENGINES: TtsEngine[] = [
    "local",
    "http",
    "openai",
    "elevenlabs",
    "cartesia",
    "google",
    "microsoft",
    "edge"
  ];
  const EFFECTS: EffectId[] = ["none", "walkie_talkie", "game_boy"];

  const fallbackEngineOptions = ENGINES.map((e) => ({ value: e, label: e }));
  const effectOptions = EFFECTS.map((e) => ({ value: e, label: e }));

  let fileInput: HTMLInputElement | null = $state(null);
  let catalog = $state<EngineCatalogEntry[]>([]);
  let catalogLoading = $state(false);
  let voicesByEngine = $state<Partial<Record<TtsEngine, VoiceCatalogEntry[]>>>({});
  let voicesLoadingFor = $state<TtsEngine | null>(null);

  const engineOptions = $derived(
    catalog.length
      ? catalog.map((entry) => ({ value: entry.engine, label: entry.label }))
      : fallbackEngineOptions
  );

  const profiles = $derived(localConfig.tts.profiles);
  const activeId = $derived(localConfig.tts.active_profile_id);
  const activeIndex = $derived(profiles.findIndex((p: VoiceProfile) => p.id === activeId));
  const active = $derived(activeIndex >= 0 ? profiles[activeIndex] : null);
  const profileOptions = $derived(profiles.map((p: VoiceProfile) => ({ value: p.id, label: p.name })));

  const activeCatalogEntry = $derived(
    active ? catalog.find((entry) => entry.engine === active.engine) : undefined
  );
  const activeVoiceCatalog = $derived<VoiceCatalogEntry[]>(
    active ? catalogVoicesFor(active.engine as TtsEngine) : []
  );
  const activeVoiceOptions = $derived(
    activeVoiceCatalog.map((voice: VoiceCatalogEntry) => ({ value: voice.id, label: voice.label }))
  );

  $effect(() => {
    void loadEngineCatalog();
  });

  async function loadEngineCatalog() {
    if (catalogLoading || catalog.length > 0) return;
    catalogLoading = true;
    try {
      const entries = (await invoke("list_tts_engines")) as EngineCatalogEntry[];
      catalog = entries;
      const next: Partial<Record<TtsEngine, VoiceCatalogEntry[]>> = {};
      for (const entry of entries) {
        next[entry.engine] = entry.voices;
      }
      voicesByEngine = next;
    } catch (err) {
      toast.error(`Could not load engine catalog: ${err}`);
    } finally {
      catalogLoading = false;
    }
  }

  async function refreshVoices(engine: TtsEngine) {
    voicesLoadingFor = engine;
    try {
      const voices = (await invoke("list_tts_voices", { engine })) as VoiceCatalogEntry[];
      voicesByEngine = { ...voicesByEngine, [engine]: voices };
      toast.success(`Loaded ${voices.length} voice${voices.length === 1 ? "" : "s"}`);
    } catch (err) {
      toast.error(`Voice refresh failed: ${err}`);
    } finally {
      voicesLoadingFor = null;
    }
  }

  function optionValue(profile: VoiceProfile, descriptor: EngineOptionDescriptor): unknown {
    const options = profile.engine_options;
    if (options && typeof options === "object" && !Array.isArray(options)) {
      const existing = (options as Record<string, unknown>)[descriptor.key];
      if (existing !== undefined && existing !== null) return existing;
    }
    return descriptor.default_value;
  }

  function optionInputValue(profile: VoiceProfile, descriptor: EngineOptionDescriptor): string {
    const value = optionValue(profile, descriptor);
    if (Array.isArray(value)) return value.join("\n");
    return String(value ?? "");
  }

  function catalogVoicesFor(engine: TtsEngine): VoiceCatalogEntry[] {
    return voicesByEngine[engine] ?? [];
  }

  function setOptionValue(index: number, key: string, value: unknown) {
    const profile = localConfig.tts.profiles[index];
    const current = profile.engine_options;
    const base = current && typeof current === "object" && !Array.isArray(current) ? current : {};
    profile.engine_options = {
      ...base,
      engine: profile.engine,
      [key]: value
    } as VoiceProfile["engine_options"];
  }

  function setVoice(index: number, voiceId: string) {
    const profile = localConfig.tts.profiles[index];
    profile.voice = voiceId;
    const match = catalogVoicesFor(profile.engine).find((voice) => voice.id === voiceId);
    profile.voice_label = match?.label;
  }

  function resetEngineOptions(index: number, engine: TtsEngine) {
    const entry = catalog.find((item) => item.engine === engine);
    if (!entry) return;
    const defaults: Record<string, unknown> = { engine };
    for (const option of entry.options) {
      if (option.default_value !== null) defaults[option.key] = option.default_value;
    }
    localConfig.tts.profiles[index].engine_options = defaults as VoiceProfile["engine_options"];
  }

  function selectProfile(id: string) {
    localConfig.tts.active_profile_id = id;
  }

  function onEngineChange(engine: TtsEngine) {
    if (activeIndex < 0) return;
    localConfig.tts.profiles[activeIndex].engine = engine;
    resetEngineOptions(activeIndex, engine);
    const firstVoice = catalogVoicesFor(engine)[0];
    if (firstVoice && !localConfig.tts.profiles[activeIndex].voice) {
      setVoice(activeIndex, firstVoice.id);
    }
  }

  function makeId(): string {
    return `profile-${crypto.randomUUID().slice(0, 8)}`;
  }

  function duplicateActive() {
    if (!active) return;
    const copy: VoiceProfile = JSON.parse(JSON.stringify(active));
    copy.id = makeId();
    copy.name = `${active.name} copy`;
    localConfig.tts.profiles = [...profiles, copy];
    selectProfile(copy.id);
    toast.success("Profile duplicated");
  }

  function deleteActive() {
    if (!active || active.id === "default") return;
    localConfig.tts.profiles = profiles.filter((p: VoiceProfile) => p.id !== active.id);
    selectProfile(localConfig.tts.profiles[0]?.id ?? "default");
    toast.success("Profile deleted");
  }

  function isValidProfile(p: unknown): p is VoiceProfile {
    if (!p || typeof p !== "object") return false;
    const o = p as Record<string, unknown>;
    return (
      typeof o.id === "string" &&
      typeof o.name === "string" &&
      typeof o.engine === "string" &&
      ENGINES.includes(o.engine as TtsEngine) &&
      typeof o.voice === "string" &&
      Number.isFinite(o.speed) &&
      Number.isFinite(o.pitch) &&
      typeof o.effects === "object" &&
      o.effects !== null &&
      EFFECTS.includes((o.effects as Record<string, unknown>).active_effect as EffectId)
    );
  }

  function exportActive() {
    if (!active) return;
    // Profiles never carry API keys, so this export is safe to share.
    const json = JSON.stringify({ schema_version: 2, ...active }, null, 2);
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${active.id}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }

  async function onFileSelected(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    try {
      const parsed = JSON.parse(await file.text());
      const { schema_version, ...rest } = parsed;
      if (!isValidProfile(rest)) {
        toast.error("Invalid profile JSON");
        return;
      }
      const imported = rest as VoiceProfile;
      // Avoid id collisions.
      if (profiles.some((p: VoiceProfile) => p.id === imported.id)) {
        imported.id = makeId();
      }
      localConfig.tts.profiles = [...profiles, imported];
      selectProfile(imported.id);
      toast.success(`Imported profile "${imported.name}"`);
    } catch (err) {
      toast.error(`Import failed: ${err}`);
    } finally {
      input.value = "";
    }
  }
</script>

<div class="border-border overflow-hidden rounded-lg border">
  <!-- Header: title + actions -->
  <div class="bg-muted/50 border-border flex items-center justify-between border-b p-4">
    <div>
      <h2 class="text-lg font-semibold">Voice Profiles</h2>
      <p class="text-muted-foreground mt-1 text-sm">
        Named presets bundling engine, voice, speed, pitch and effect.
      </p>
    </div>
    <div class="flex gap-1.5">
      <Button variant="outline" size="sm" onclick={duplicateActive} title="Duplicate">
        <Copy size={14} />
      </Button>
      <Button variant="outline" size="sm" onclick={exportActive} title="Export">
        <Download size={14} />
      </Button>
      <Button variant="outline" size="sm" onclick={() => fileInput?.click()} title="Import">
        <Upload size={14} />
      </Button>
      <Button
        variant="outline"
        size="sm"
        onclick={deleteActive}
        disabled={!active || active.id === "default"}
        title="Delete"
      >
        <Trash2 size={14} />
      </Button>
    </div>
  </div>

  <input
    bind:this={fileInput}
    type="file"
    accept="application/json"
    class="hidden"
    onchange={onFileSelected}
  />

  <div class="space-y-1 p-4">
    <SettingRow label="Active Profile">
      <Select
        options={profileOptions}
        value={activeId}
        onchange={(e) => selectProfile((e.target as HTMLSelectElement).value)}
        class="w-56"
      />
    </SettingRow>

    {#if active}
      <SettingRow label="Name">
        <Input bind:value={localConfig.tts.profiles[activeIndex].name} class="w-56" />
      </SettingRow>

      <SettingRow label="Engine">
        <Select
          options={engineOptions}
          value={active.engine}
          onchange={(e) => onEngineChange((e.target as HTMLSelectElement).value as TtsEngine)}
          class="w-56"
        />
      </SettingRow>

      <SettingRow label="Manual Voice" tooltip="Voice id / name. Blank uses the provider default.">
        <Input
          value={localConfig.tts.profiles[activeIndex].voice}
          placeholder="provider default"
          onchange={(e) => setVoice(activeIndex, (e.target as HTMLInputElement).value)}
          class="w-56"
        />
      </SettingRow>

      <SettingRow label="Speed">
        <div class="flex w-56 items-center gap-2">
          <span class="text-muted-foreground w-12 shrink-0 text-right text-xs tabular-nums">
            {active.speed.toFixed(2)}x
          </span>
          <Slider
            value={active.speed}
            min={0.5}
            max={2}
            step={0.05}
            onchange={(v) => (localConfig.tts.profiles[activeIndex].speed = v)}
          />
        </div>
      </SettingRow>

      <SettingRow label="Pitch">
        <div class="flex w-56 items-center gap-2">
          <span class="text-muted-foreground w-12 shrink-0 text-right text-xs tabular-nums">
            {active.pitch.toFixed(2)}x
          </span>
          <Slider
            value={active.pitch}
            min={0.5}
            max={2}
            step={0.05}
            onchange={(v) => (localConfig.tts.profiles[activeIndex].pitch = v)}
          />
        </div>
      </SettingRow>

      <SettingRow label="Effect">
        <Select
          options={effectOptions}
          value={active.effects.active_effect}
          onchange={(e) => {
            const v = (e.target as HTMLSelectElement).value as EffectId;
            localConfig.tts.profiles[activeIndex].effects.active_effect = v;
            localConfig.tts.profiles[activeIndex].effects.enabled = v !== "none";
          }}
          class="w-56"
        />
      </SettingRow>

      {#if activeCatalogEntry}
        <div class="border-border mt-3 space-y-3 border-t pt-3">
          <div class="flex items-start justify-between gap-4">
            <div>
              <p class="text-sm font-medium">{activeCatalogEntry.label}</p>
              <p class="text-muted-foreground text-xs">{activeCatalogEntry.description}</p>
            </div>
            <a
              href={activeCatalogEntry.docs_url}
              target="_blank"
              rel="noreferrer"
              class="text-muted-foreground hover:text-foreground inline-flex items-center gap-1 text-xs"
            >
              Docs <ExternalLink size={12} />
            </a>
          </div>
        </div>
      {/if}

      {#if activeVoiceOptions.length > 0}
        <SettingRow label="Catalog Voice" tooltip="Known voices from the engine catalog or provider API.">
          <div class="flex w-56 gap-1.5">
            <Select
              options={activeVoiceOptions}
              value={active.voice}
              onchange={(e) => setVoice(activeIndex, (e.target as HTMLSelectElement).value)}
              class="min-w-0 flex-1"
            />
            <Button
              variant="outline"
              size="sm"
              onclick={() => refreshVoices(active.engine)}
              disabled={voicesLoadingFor === active.engine}
              title="Refresh voices"
            >
              <RefreshCw size={14} />
            </Button>
          </div>
        </SettingRow>
      {:else if activeCatalogEntry?.supports_voice_refresh}
        <SettingRow label="Catalog Voice">
          <Button
            variant="outline"
            size="sm"
            onclick={() => refreshVoices(active.engine)}
            disabled={voicesLoadingFor === active.engine}
          >
            {voicesLoadingFor === active.engine ? "Loading voices…" : "Load voices"}
          </Button>
        </SettingRow>
      {/if}

      {#if activeCatalogEntry?.options.length}
        <div class="border-border mt-3 border-t pt-3">
          <p class="mb-2 text-sm font-medium">Engine Settings</p>
          {#each activeCatalogEntry.options as option (option.key)}
            <SettingRow label={option.label} tooltip={option.help}>
              {#if option.kind === "number"}
                <Input
                  type="number"
                  value={String(optionValue(active, option) ?? "")}
                  onchange={(e) => {
                    const raw = (e.target as HTMLInputElement).value;
                    setOptionValue(activeIndex, option.key, raw === "" ? null : Number(raw));
                  }}
                  class="w-56"
                />
              {:else if option.kind === "boolean"}
                <Select
                  options={[{ value: "true", label: "Enabled" }, { value: "false", label: "Disabled" }]}
                  value={String(Boolean(optionValue(active, option)))}
                  onchange={(e) =>
                    setOptionValue(activeIndex, option.key, (e.target as HTMLSelectElement).value === "true")}
                  class="w-56"
                />
              {:else if option.kind === "textarea"}
                <textarea
                  value={optionInputValue(active, option)}
                  onchange={(e) => {
                    const raw = (e.target as HTMLTextAreaElement).value;
                    const value = option.key === "args_template"
                      ? raw.split("\n").map((line) => line.trim()).filter(Boolean)
                      : raw;
                    setOptionValue(activeIndex, option.key, value);
                  }}
                  class="border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:ring-ring min-h-20 w-56 rounded-md border px-3 py-2 text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none"
                ></textarea>
              {:else}
                <Input
                  value={optionInputValue(active, option)}
                  onchange={(e) => setOptionValue(activeIndex, option.key, (e.target as HTMLInputElement).value)}
                  class="w-56"
                />
              {/if}
            </SettingRow>
          {/each}
        </div>
      {/if}
    {/if}

    {#if active && active.id === "default"}
      <p class="text-muted-foreground border-border mt-2 border-t pt-3 text-xs">
        The Default profile is now a real profile. Duplicate it to create a named profile with
        independent engine, catalog voice, engine settings, speed, pitch and effect.
      </p>
    {/if}
  </div>
</div>
