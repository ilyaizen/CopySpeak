<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { Slider } from "$lib/components/ui/slider/index.js";
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { invoke } from "@tauri-apps/api/core";
  import { Copy, Trash2, Download, Upload, RefreshCw, ExternalLink, AlertTriangle } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import ProfileExportDialog from "./profile-export-dialog.svelte";
  import { findSetupEntry } from "./engine-meta";
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

  let catalog = $state<EngineCatalogEntry[]>([]);
  let catalogLoading = $state(false);
  let voicesByEngine = $state<Partial<Record<TtsEngine, VoiceCatalogEntry[]>>>({});
  let voicesLoadingFor = $state<TtsEngine | null>(null);

  type DialogMode = "export" | "import" | "delete";
  let dialogMode = $state<DialogMode | null>(null);

  const engineOptions = $derived(
    catalog.length
      ? catalog.map((entry) => ({ value: entry.engine, label: entry.label }))
      : fallbackEngineOptions
  );

  const profiles = $derived(localConfig.tts.profiles);
  const activeId = $derived(localConfig.tts.active_profile_id);
  const activeIndex = $derived(profiles.findIndex((p: VoiceProfile) => p.id === activeId));
  const active = $derived(activeIndex >= 0 ? profiles[activeIndex] : null);
  const profileOptions = $derived(
    profiles.map((p: VoiceProfile) => ({ value: p.id, label: p.name }))
  );

  const activeCatalogEntry = $derived(
    active ? catalog.find((entry) => entry.engine === active.engine) : undefined
  );
  const activeVoiceCatalog = $derived<VoiceCatalogEntry[]>(
    active ? catalogVoicesFor(active.engine as TtsEngine) : []
  );
  const activeVoiceOptions = $derived(
    activeVoiceCatalog.map((voice: VoiceCatalogEntry) => ({
      value: voice.id,
      label: voice.label
    }))
  );

  // Passive hint: the active profile's engine needs a credential that isn't set.
  // Cheap local check (no IPC) — points the user to /engines rather than blocking.
  const credentialMissing = $derived(() => {
    if (!active) return false;
    const entry = findSetupEntry(active.engine);
    if (!entry || !entry.credentialTarget) return false;
    const fields = localConfig.tts as unknown as Record<string, { api_key?: string }>;
    return !fields[entry.credentialTarget]?.api_key;
  });

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
        next[entry.engine as TtsEngine] = entry.voices;
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
    const base =
      current && typeof current === "object" && !Array.isArray(current) ? current : {};
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
    const p = profiles.find((x: VoiceProfile) => x.id === id);
    if (p && p.id !== "default") {
      localConfig.tts.active_backend = p.engine;
    }
  }

  function onEngineChange(engine: TtsEngine) {
    if (activeIndex < 0) return;
    localConfig.tts.profiles[activeIndex].engine = engine;
    resetEngineOptions(activeIndex, engine);
    const firstVoice = catalogVoicesFor(engine)[0];
    if (firstVoice && !localConfig.tts.profiles[activeIndex].voice) {
      setVoice(activeIndex, firstVoice.id);
    }
    if (active && active.id !== "default") {
      localConfig.tts.active_backend = engine;
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
    dialogMode = "delete";
  }

  function confirmDelete() {
    if (!active) return;
    localConfig.tts.profiles = profiles.filter((p: VoiceProfile) => p.id !== active.id);
    selectProfile(localConfig.tts.profiles[0]?.id ?? "default");
    dialogMode = null;
    toast.success("Profile deleted");
  }

  function openExportDialog() {
    if (!active) return;
    dialogMode = "export";
  }

  function openImportDialog() {
    dialogMode = "import";
  }

  function handleImportProfile(imported: VoiceProfile) {
    if (profiles.some((p: VoiceProfile) => p.id === imported.id)) {
      imported.id = makeId();
    }
    localConfig.tts.profiles = [...profiles, imported];
    selectProfile(imported.id);
    dialogMode = null;
    toast.success(`Imported "${imported.name}"`);
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
      <Button variant="outline" size="sm" onclick={openExportDialog} title="Export">
        <Download size={14} />
      </Button>
      <Button variant="outline" size="sm" onclick={openImportDialog} title="Import">
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

  <div class="space-y-1 p-4">
    <SettingRow label="Active Profile">
      <Select
        options={profileOptions}
        value={activeId}
        onchange={(e) => selectProfile((e.target as HTMLSelectElement).value)}
        class="w-56"
      />
    </SettingRow>
  </div>

  {#if active}
    <div class="border-border space-y-4 border-t p-4">
      <!-- Identity -->
      <section class="border-border bg-muted/30 rounded-lg border p-3">
        <p class="text-muted-foreground mb-2 text-xs font-semibold tracking-wide uppercase">
          Identity
        </p>
        <SettingRow label="Name">
          <Input bind:value={localConfig.tts.profiles[activeIndex].name} class="w-56" />
        </SettingRow>
      </section>

      <!-- Engine & Voice -->
      <section class="border-border bg-muted/30 space-y-3 rounded-lg border p-3">
        <p class="text-muted-foreground text-xs font-semibold tracking-wide uppercase">
          Engine & Voice
        </p>
        <SettingRow label="Engine">
          <div class="w-56 space-y-1">
            <Select
              options={engineOptions}
              value={active.engine}
              onchange={(e) => onEngineChange((e.target as HTMLSelectElement).value as TtsEngine)}
              class="w-56"
            />
            {#if credentialMissing()}
              <a
                href="/engines"
                class="text-amber-600 dark:text-amber-400 inline-flex items-center gap-1 text-xs hover:underline"
              >
                <AlertTriangle size={12} />
                Set up engine credentials →
              </a>
            {/if}
          </div>
        </SettingRow>

        {#if activeVoiceOptions.length > 0}
          <SettingRow
            label="Voice"
            tooltip="Known voices from the engine catalog or provider API."
          >
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
          <SettingRow label="Voice">
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

        <SettingRow label="Manual Voice" tooltip="Override the catalog voice id. Blank = provider default.">
          <Input
            value={localConfig.tts.profiles[activeIndex].voice}
            placeholder="provider default"
            onchange={(e) => setVoice(activeIndex, (e.target as HTMLInputElement).value)}
            class="w-56"
          />
        </SettingRow>
      </section>

      <!-- Sound -->
      <section class="border-border bg-muted/30 space-y-3 rounded-lg border p-3">
        <p class="text-muted-foreground text-xs font-semibold tracking-wide uppercase">Sound</p>
        <SettingRow label="Speed">
          <div class="flex w-56 items-center gap-2">
            <span
              class="text-muted-foreground w-12 shrink-0 text-right text-xs tabular-nums"
            >
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
            <span
              class="text-muted-foreground w-12 shrink-0 text-right text-xs tabular-nums"
            >
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
      </section>

      <!-- Advanced: engine-specific options + docs -->
      {#if activeCatalogEntry}
        <section class="border-border bg-muted/30 space-y-3 rounded-lg border p-3">
          <div class="flex items-center justify-between gap-2">
            <p class="text-muted-foreground text-xs font-semibold tracking-wide uppercase">
              {activeCatalogEntry.label}
            </p>
            <a
              href={activeCatalogEntry.docs_url}
              target="_blank"
              rel="noreferrer"
              class="text-muted-foreground hover:text-foreground inline-flex items-center gap-1 text-xs"
            >
              Docs <ExternalLink size={12} />
            </a>
          </div>

          {#if activeCatalogEntry.options.length}
            {#each activeCatalogEntry.options as option (option.key)}
              <SettingRow label={option.label} tooltip={option.help}>
                {#if option.kind === "number"}
                  <Input
                    type="number"
                    value={String(optionValue(active, option) ?? "")}
                    onchange={(e) => {
                      const raw = (e.target as HTMLInputElement).value;
                      setOptionValue(
                        activeIndex,
                        option.key,
                        raw === "" ? null : Number(raw)
                      );
                    }}
                    class="w-56"
                  />
                {:else if option.kind === "boolean"}
                  <Select
                    options={[
                      { value: "true", label: "Enabled" },
                      { value: "false", label: "Disabled" }
                    ]}
                    value={String(Boolean(optionValue(active, option)))}
                    onchange={(e) =>
                      setOptionValue(
                        activeIndex,
                        option.key,
                        (e.target as HTMLSelectElement).value === "true"
                      )}
                    class="w-56"
                  />
                {:else if option.kind === "textarea"}
                  <textarea
                    value={optionInputValue(active, option)}
                    onchange={(e) => {
                      const raw = (e.target as HTMLTextAreaElement).value;
                      const value =
                        option.key === "args_template"
                          ? raw
                              .split("\n")
                              .map((line) => line.trim())
                              .filter(Boolean)
                          : raw;
                      setOptionValue(activeIndex, option.key, value);
                    }}
                    class="border-input bg-background ring-offset-background placeholder:text-muted-foreground focus-visible:ring-ring min-h-20 w-56 rounded-md border px-3 py-2 text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none"
                  ></textarea>
                {:else}
                  <Input
                    value={optionInputValue(active, option)}
                    onchange={(e) =>
                      setOptionValue(
                        activeIndex,
                        option.key,
                        (e.target as HTMLInputElement).value
                      )
                    }
                    class="w-56"
                  />
                {/if}
              </SettingRow>
            {/each}
          {/if}
        </section>
      {/if}

      {#if active.id === "default"}
        <p class="text-muted-foreground text-xs">
          The Default profile is now a real profile. Duplicate it to create a named profile
          with independent engine, catalog voice, engine settings, speed, pitch and effect.
        </p>
      {/if}
    </div>
  {/if}
</div>

<!-- Export / Import / Delete dialogs -->
{#if dialogMode}
  <ProfileExportDialog
    mode={dialogMode}
    profile={active}
    onImport={handleImportProfile}
    onDelete={confirmDelete}
    onClose={() => (dialogMode = null)}
  />
{/if}
