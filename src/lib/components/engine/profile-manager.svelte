<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import {
    DropdownMenu,
    DropdownMenuTrigger,
    DropdownMenuContent,
    DropdownMenuItem
  } from "$lib/components/ui/dropdown-menu/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { Slider } from "$lib/components/ui/slider/index.js";
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { invoke } from "@tauri-apps/api/core";
  import {
    Copy,
    Trash2,
    Download,
    Upload,
    ExternalLink,
    AlertTriangle,
    Plus
  } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import ProfileExportDialog from "./profile-export-dialog.svelte";
  import VoicePicker from "./voice-picker.svelte";
  import { findSetupEntry } from "./engine-meta";
  import type {
    AppConfig,
    EngineCatalogEntry,
    EngineOptionDescriptor,
    EngineOptionValue,
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
    "edge",
    "kitten",
    "piper",
    "kokoro",
    "pocket"
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

  // §5: built-in profile templates — selecting one copies a fresh VoiceProfile
  // seed into profiles (new id), never a live reference. Mirrors the Rust
  // default_*_profile() pattern generalized to an on-demand list.
  const BUILTIN_PROFILE_PRESETS: {
    name: string;
    engine: TtsEngine;
    voice: string;
    voiceLabel: string;
    engineOptions: Record<string, EngineOptionValue>;
  }[] = [
    {
      name: "Kitten TTS — Rosie",
      engine: "kitten",
      voice: "Rosie",
      voiceLabel: "Rosie",
      engineOptions: { engine: "kitten" }
    },
    {
      name: "Piper — Amy",
      engine: "piper",
      voice: "en_US-amy-medium",
      voiceLabel: "Amy",
      engineOptions: { engine: "piper" }
    },
    {
      name: "Kokoro — Heart",
      engine: "kokoro",
      voice: "af_heart",
      voiceLabel: "Heart",
      engineOptions: { engine: "kokoro" }
    },
    {
      name: "Pocket — Alba",
      engine: "pocket",
      voice: "alba",
      voiceLabel: "Alba",
      engineOptions: { engine: "pocket" }
    }
  ];

  // §6: "Start from" prefill for the Local CLI escape hatch — copies
  // command/args/voice into the profile; editing never touches the template.
  const LOCAL_TEMPLATES: {
    id: string;
    label: string;
    command: string;
    args_template: string[];
    voice: string;
  }[] = [
    { id: "blank", label: "Custom (blank)", command: "", args_template: [], voice: "" },
    {
      id: "kitten",
      label: "Kitten-style",
      command: "uv",
      args_template: [
        "run",
        "--project",
        "{engine_dir}/kitten",
        "python",
        "{engine_dir}/kitten/scripts/copyspeak-kitten.py",
        "--text-file",
        "{input}",
        "--voice",
        "{voice}",
        "--output",
        "{output}",
        "--model",
        "{model}"
      ],
      voice: "Rosie"
    },
    {
      id: "piper",
      label: "Piper-style",
      command: "uv",
      args_template: [
        "run",
        "--project",
        "{engine_dir}/piper",
        "python",
        "{engine_dir}/piper/scripts/copyspeak-piper.py",
        "--text-file",
        "{input}",
        "--voice",
        "{voice}",
        "--output",
        "{output}"
      ],
      voice: "en_US-amy-medium"
    },
    {
      id: "kokoro",
      label: "Kokoro-style",
      command: "uv",
      args_template: [
        "run",
        "--project",
        "{engine_dir}/kokoro",
        "python",
        "{engine_dir}/kokoro/scripts/copyspeak-kokoro.py",
        "--text-file",
        "{input}",
        "--voice",
        "{voice}",
        "--output",
        "{output}"
      ],
      voice: "af_heart"
    },
    {
      id: "pocket",
      label: "Pocket-style",
      command: "uv",
      args_template: [
        "run",
        "--project",
        "{engine_dir}/pocket",
        "python",
        "{engine_dir}/pocket/scripts/copyspeak-pocket.py",
        "--text-file",
        "{input}",
        "--voice",
        "{voice}",
        "--output",
        "{output}"
      ],
      voice: "alba"
    }
  ];

  const engineOptions = $derived(
    catalog.length
      ? catalog.map((entry) => ({
          value: entry.engine,
          label: entry.label
        }))
      : fallbackEngineOptions
  );

  const activeEngineCatalogEntry = $derived(
    catalog.find((entry) => entry.engine === active?.engine)
  );

  const profiles = $derived(localConfig.tts.profiles);
  const activeId = $derived(localConfig.tts.active_profile_id);
  const activeIndex = $derived(profiles.findIndex((p: VoiceProfile) => p.id === activeId));
  const active = $derived(activeIndex >= 0 ? profiles[activeIndex] : null);

  // Live drag values: shown while a slider is dragged, cleared on commit so the
  // profile becomes the source of truth again (also handles profile switching).
  let speedLive = $state<number | null>(null);
  let pitchLive = $state<number | null>(null);
  const profileOptions = $derived(
    profiles.map((p: VoiceProfile) => ({ value: p.id, label: p.name }))
  );

  const activeCatalogEntry = $derived(
    active ? catalog.find((entry) => entry.engine === active.engine) : undefined
  );
  const activeVoiceCatalog = $derived.by<VoiceCatalogEntry[]>(() => {
    if (!active) return [];
    const rawVoices = catalogVoicesFor(active.engine as TtsEngine);
    if (active.engine === "local") {
      const preset = (active.engine_options as Record<string, unknown> | undefined)?.preset as
        string | undefined;
      if (preset === "piper") return rawVoices.filter((v) => v.language === "Piper");
      if (preset === "kokoro") return rawVoices.filter((v) => v.language === "Kokoro");
      if (preset === "kitten-tts") return rawVoices.filter((v) => v.language === "KittenTTS");
      if (preset === "custom") return rawVoices;
      return [];
    }
    return rawVoices;
  });

  const showPicker = $derived(
    activeVoiceCatalog.length > 0 || !!activeCatalogEntry?.supports_voice_refresh
  );

  // Picker is primary: the manual input stays locked while the current voice is
  // a catalog id. "Custom / manual id…" in the picker unlocks it for this
  // profile/engine only — ephemeral, never persisted to config.
  let manualOverride = $state(false);
  let manualRef = $state<HTMLInputElement | null>(null);

  $effect(() => {
    activeId;
    active?.engine;
    manualOverride = false;
  });

  const manualLocked = $derived(
    showPicker && !manualOverride && activeVoiceCatalog.some((v) => v.id === (active?.voice ?? ""))
  );

  // Passive hint: checks backend (config.json + .env) for credential presence.
  let credentialsResolved = $state<Record<string, boolean>>({});

  $effect(() => {
    if (!active) return;
    const entry = findSetupEntry(active.engine);
    if (!entry || !entry.credentialTarget) return;
    const engine = active.engine;
    // Skip if already checked
    if (engine in credentialsResolved) return;
    invoke<boolean>("has_engine_credentials", { engine }).then((ok) => {
      credentialsResolved[engine] = ok;
    });
  });

  const credentialMissing = $derived(() => {
    if (!active) return false;
    const entry = findSetupEntry(active.engine);
    if (!entry || !entry.credentialTarget) return false;
    return credentialsResolved[active.engine] === false;
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

  function applyPresetDefaults(index: number, preset: string) {
    const profile = localConfig.tts.profiles[index];
    if (profile.engine !== "local") return;
    if (preset === "piper") {
      profile.engine_options = {
        ...profile.engine_options,
        command: "uv",
        args_template: [
          "run",
          "--project",
          "{engine_dir}/piper",
          "python",
          "{engine_dir}/piper/scripts/copyspeak-piper.py",
          "--text-file",
          "{input}",
          "--voice",
          "{voice}",
          "--output",
          "{output}"
        ]
      } as VoiceProfile["engine_options"];
      profile.voice = "en_US-amy-medium";
      profile.voice_label = "Amy";
    } else if (preset === "kitten-tts") {
      profile.engine_options = {
        ...profile.engine_options,
        command: "uv",
        args_template: [
          "run",
          "--project",
          "{engine_dir}/kitten",
          "python",
          "{engine_dir}/kitten/scripts/copyspeak-kitten.py",
          "--text-file",
          "{input}",
          "--voice",
          "{voice}",
          "--output",
          "{output}",
          "--model",
          "{model}"
        ]
      } as VoiceProfile["engine_options"];
      profile.voice = "Rosie";
      profile.voice_label = "Rosie";
    } else if (preset === "kokoro") {
      profile.engine_options = {
        ...profile.engine_options,
        command: "uv",
        args_template: [
          "run",
          "--project",
          "{engine_dir}/kokoro",
          "python",
          "{engine_dir}/kokoro/scripts/copyspeak-kokoro.py",
          "--text-file",
          "{input}",
          "--voice",
          "{voice}",
          "--output",
          "{output}"
        ]
      } as VoiceProfile["engine_options"];
      profile.voice = "af_heart";
      profile.voice_label = "Heart";
    } else if (preset === "pocket") {
      profile.engine_options = {
        ...profile.engine_options,
        command: "uv",
        args_template: [
          "run",
          "--project",
          "{engine_dir}/pocket",
          "python",
          "{engine_dir}/pocket/scripts/copyspeak-pocket.py",
          "--text-file",
          "{input}",
          "--voice",
          "{voice}",
          "--output",
          "{output}"
        ]
      } as VoiceProfile["engine_options"];
      profile.voice = "alba";
      profile.voice_label = "Alba";
    }
  }

  function setOptionValue(index: number, key: string, value: unknown) {
    const profile = localConfig.tts.profiles[index];
    const current = profile.engine_options;
    const base = current && typeof current === "object" && !Array.isArray(current) ? current : {};

    let updatedOptions = {
      ...base,
      engine: profile.engine,
      [key]: value
    };

    profile.engine_options = updatedOptions as VoiceProfile["engine_options"];

    if (profile.engine === "local" && key === "preset") {
      applyPresetDefaults(index, value as string);
    }
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

  async function selectProfile(id: string) {
    localConfig.tts.active_profile_id = id;
    const p = profiles.find((x: VoiceProfile) => x.id === id);
    if (p && p.id !== "default") {
      localConfig.tts.active_backend = p.engine;
    }
    // Persist immediately — stateful like the footer profile selector
    try {
      await invoke("set_active_profile", { id });
    } catch (e) {
      console.error("Failed to set active profile:", e);
    }
  }

  function onEngineChange(engine: TtsEngine) {
    if (activeIndex < 0) return;
    localConfig.tts.profiles[activeIndex].engine = engine;
    resetEngineOptions(activeIndex, engine);
    // Auto-populate default preset's command/args/voice for local engine
    if (engine === "local") {
      const entry = catalog.find((item) => item.engine === engine);
      const defaultPreset = entry?.options.find((o) => o.key === "preset")?.default_value as
        string | undefined;
      if (defaultPreset && defaultPreset !== "custom") {
        applyPresetDefaults(activeIndex, defaultPreset);
      }
    }
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

  function newFromPreset(preset: (typeof BUILTIN_PROFILE_PRESETS)[number]) {
    const profile: VoiceProfile = {
      id: makeId(),
      name: preset.name,
      description: null,
      engine: preset.engine,
      voice: preset.voice,
      voice_label: preset.voiceLabel,
      speed: 1.0,
      pitch: 1.0,
      effects: { enabled: true, active_effect: "walkie_talkie" },
      engine_options: preset.engineOptions
    };
    localConfig.tts.profiles = [...profiles, profile];
    selectProfile(profile.id);
    toast.success(`Created "${profile.name}"`);
  }

  function applyLocalTemplate(id: string) {
    if (activeIndex < 0) return;
    const profile = localConfig.tts.profiles[activeIndex];
    if (profile.engine !== "local") return;
    const t = LOCAL_TEMPLATES.find((x) => x.id === id);
    if (!t) return;
    profile.engine_options = {
      ...profile.engine_options,
      engine: "local",
      command: t.command,
      args_template: t.args_template
    };
    profile.voice = t.voice;
    profile.voice_label = t.voice || null;
  }
</script>

<div>
  <!-- Header: title + actions -->
  <div class="border-border flex flex-wrap items-center justify-between gap-3 border-b pb-4">
    <div>
      <h2 class="text-lg font-semibold">Voice Profiles</h2>
      <p class="text-muted-foreground mt-1 text-sm">
        Named presets bundling engine, voice, speed, pitch and effect.
      </p>
    </div>
    <div class="flex gap-1.5">
      <DropdownMenu>
        <DropdownMenuTrigger
          class="border-input bg-background hover:bg-accent inline-flex h-8 items-center justify-center rounded-md border px-2"
          title="New profile from template"
          aria-label="New profile from template"
        >
          <Plus size={14} />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" class="min-w-48">
          {#each BUILTIN_PROFILE_PRESETS as preset (preset.name)}
            <DropdownMenuItem onclick={() => newFromPreset(preset)}>
              {preset.name}
            </DropdownMenuItem>
          {/each}
        </DropdownMenuContent>
      </DropdownMenu>
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

  <div class="space-y-1 border-b py-4">
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
    <div class="space-y-5 py-5">
      <!-- Identity -->
      <section class="border-border border-b pb-5">
        <p class="text-muted-foreground mb-2 text-xs font-semibold tracking-wide uppercase">
          Identity
        </p>
        <SettingRow label="Name">
          <Input bind:value={localConfig.tts.profiles[activeIndex].name} class="w-56" />
        </SettingRow>
      </section>

      <!-- Engine & Voice -->
      <section class="border-border space-y-3 border-b pb-5">
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
                class="inline-flex items-center gap-1 text-xs text-amber-600 hover:underline dark:text-amber-400"
              >
                <AlertTriangle size={12} />
                Set up engine credentials →
              </a>
            {/if}
            {#if activeEngineCatalogEntry && !activeEngineCatalogEntry.supports_captions}
              <p class="text-muted-foreground inline-flex items-center gap-1 text-xs">
                <AlertTriangle size={12} />
                Live word captions are unavailable for this engine.
              </p>
            {/if}
          </div>
        </SettingRow>

        {#if active.engine === "local"}
          <SettingRow
            label="Start from"
            tooltip="Prefill command/arguments/voice from a known engine. Editing afterward never changes the template."
          >
            <Select
              options={LOCAL_TEMPLATES.map((t) => ({ value: t.id, label: t.label }))}
              value="blank"
              onchange={(e) => applyLocalTemplate((e.target as HTMLSelectElement).value)}
              class="w-56"
            />
          </SettingRow>
        {/if}

        {#if showPicker}
          <SettingRow label="Voice" tooltip="Known voices from the engine catalog or provider API.">
            <VoicePicker
              voices={activeVoiceCatalog}
              value={active.voice}
              loading={voicesLoadingFor === active.engine}
              supportsRefresh={!!activeCatalogEntry?.supports_voice_refresh}
              onselect={(id) => {
                manualOverride = false;
                setVoice(activeIndex, id);
              }}
              onrefresh={() => refreshVoices(active.engine)}
              onmanual={() => {
                manualOverride = true;
                manualRef?.focus();
              }}
            />
          </SettingRow>
        {/if}

        <SettingRow
          label={showPicker ? "Manual Voice" : "Voice"}
          tooltip={showPicker
            ? "Override the catalog voice id. Blank = provider default."
            : "Voice id for this engine. Blank = provider default."}
        >
          <Input
            bind:ref={manualRef}
            value={localConfig.tts.profiles[activeIndex].voice}
            placeholder="provider default"
            disabled={manualLocked}
            onchange={(e) => setVoice(activeIndex, (e.target as HTMLInputElement).value)}
            class="w-56"
          />
        </SettingRow>
      </section>

      <!-- Sound -->
      <section class="border-border space-y-3 border-b pb-5">
        <p class="text-muted-foreground text-xs font-semibold tracking-wide uppercase">Sound</p>
        <SettingRow label="Speed">
          <div class="flex w-56 items-center gap-2">
            <span class="text-muted-foreground w-12 shrink-0 text-right text-xs tabular-nums">
              {(speedLive ?? active.speed).toFixed(2)}x
            </span>
            <Slider
              value={active.speed}
              min={0.5}
              max={2}
              step={0.05}
              oninput={(v) => (speedLive = v)}
              onchange={(v) => {
                localConfig.tts.profiles[activeIndex].speed = v;
                speedLive = null;
              }}
            />
          </div>
        </SettingRow>
        <SettingRow label="Pitch">
          <div class="flex w-56 items-center gap-2">
            <span class="text-muted-foreground w-12 shrink-0 text-right text-xs tabular-nums">
              {(pitchLive ?? active.pitch).toFixed(2)}x
            </span>
            <Slider
              value={active.pitch}
              min={0.75}
              max={1.35}
              step={0.01}
              oninput={(v) => (pitchLive = v)}
              onchange={(v) => {
                localConfig.tts.profiles[activeIndex].pitch = v;
                pitchLive = null;
              }}
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
        <section class="border-border space-y-3 border-b pb-5">
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
                      setOptionValue(activeIndex, option.key, raw === "" ? null : Number(raw));
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
                {:else if option.kind === "select"}
                  <Select
                    options={(option.choices ?? []).map((c) => ({ value: c, label: c }))}
                    value={String(optionValue(active, option) ?? "")}
                    onchange={(e) => {
                      const val = (e.target as HTMLSelectElement).value;
                      setOptionValue(activeIndex, option.key, val);
                    }}
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
                      setOptionValue(activeIndex, option.key, (e.target as HTMLInputElement).value)}
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
          The Default profile is now a real profile. Duplicate it to create a named profile with
          independent engine, catalog voice, engine settings, speed, pitch and effect.
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
