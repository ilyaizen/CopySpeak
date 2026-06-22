<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { Slider } from "$lib/components/ui/slider/index.js";
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { Copy, Trash2, Download, Upload } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import type { AppConfig, TtsEngine, EffectId, VoiceProfile } from "$lib/types";

  let { localConfig = $bindable() } = $props<{ localConfig: AppConfig }>();

  const ENGINES: TtsEngine[] = [
    "local",
    "http",
    "openai",
    "elevenlabs",
    "cartesia",
    "google",
    "microsoft"
  ];
  const EFFECTS: EffectId[] = ["none", "walkie_talkie", "game_boy"];

  const engineOptions = ENGINES.map((e) => ({ value: e, label: e }));
  const effectOptions = EFFECTS.map((e) => ({ value: e, label: e }));

  let fileInput: HTMLInputElement | null = $state(null);

  const profiles = $derived(localConfig.tts.profiles);
  const activeId = $derived(localConfig.tts.active_profile_id);
  const activeIndex = $derived(profiles.findIndex((p: VoiceProfile) => p.id === activeId));
  const active = $derived(activeIndex >= 0 ? profiles[activeIndex] : null);
  const profileOptions = $derived(profiles.map((p: VoiceProfile) => ({ value: p.id, label: p.name })));

  function selectProfile(id: string) {
    localConfig.tts.active_profile_id = id;
    // Mirror a named profile's engine to the legacy field so the rest of the app
    // (engine tabs, HUD) stays coherent. The "default" profile is legacy passthrough.
    const p = profiles.find((x: VoiceProfile) => x.id === id);
    if (p && p.id !== "default") {
      localConfig.tts.active_backend = p.engine;
    }
  }

  function onEngineChange(engine: TtsEngine) {
    if (activeIndex < 0) return;
    localConfig.tts.profiles[activeIndex].engine = engine;
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
    const json = JSON.stringify({ schema_version: 1, ...active }, null, 2);
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

      <SettingRow label="Voice" tooltip="Voice id / name. Blank uses the provider default.">
        <Input
          bind:value={localConfig.tts.profiles[activeIndex].voice}
          placeholder="provider default"
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

      <!-- Provider-specific fields -->
      {#if active.engine === "google"}
        <SettingRow label="Google API Key">
          <Input type="password" bind:value={localConfig.tts.google.api_key} class="w-56" />
        </SettingRow>
        <SettingRow label="Model">
          <Input bind:value={localConfig.tts.google.model} class="w-56" />
        </SettingRow>
      {:else if active.engine === "microsoft"}
        <SettingRow label="Microsoft API Key">
          <Input type="password" bind:value={localConfig.tts.microsoft.api_key} class="w-56" />
        </SettingRow>
        <SettingRow label="Endpoint">
          <Input bind:value={localConfig.tts.microsoft.endpoint} placeholder="https://..." class="w-56" />
        </SettingRow>
        <SettingRow label="Model / Deployment">
          <Input bind:value={localConfig.tts.microsoft.model} class="w-56" />
        </SettingRow>
      {:else if active.engine === "http"}
        <SettingRow label="URL Template">
          <Input
            bind:value={localConfig.tts.http.url_template}
            placeholder="http://127.0.0.1/v1/audio/speech"
            class="w-56"
          />
        </SettingRow>
        <SettingRow label="Body Template (JSON)">
          <Input bind:value={localConfig.tts.http.body_template} class="w-56" />
        </SettingRow>
        <SettingRow label="Response Format">
          <Input bind:value={localConfig.tts.http.response_format} class="w-56" />
        </SettingRow>
        <SettingRow label="Timeout (s)">
          <Input type="number" bind:value={localConfig.tts.http.timeout_secs} class="w-56" />
        </SettingRow>
      {/if}
    {/if}

    {#if active && active.id === "default"}
      <p class="text-muted-foreground border-border mt-2 border-t pt-3 text-xs">
        The Default profile mirrors the engine tabs. Duplicate it to create a named, fully
        independent profile (engine + voice + speed + pitch + effect).
      </p>
    {/if}
  </div>
</div>
