<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
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

  let fileInput: HTMLInputElement | null = $state(null);

  const profiles = $derived(localConfig.tts.profiles);
  const activeId = $derived(localConfig.tts.active_profile_id);
  const activeIndex = $derived(profiles.findIndex((p: VoiceProfile) => p.id === activeId));
  const active = $derived(activeIndex >= 0 ? profiles[activeIndex] : null);

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

<div class="border-border bg-muted/30 rounded-lg border p-4">
  <div class="mb-3 flex items-center justify-between">
    <h3 class="text-sm font-semibold">Voice Profiles</h3>
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

  <div class="grid grid-cols-1 gap-3 sm:grid-cols-2">
    <div class="space-y-1.5">
      <Label class="text-xs">Active Profile</Label>
      <select
        class="border-input bg-background h-9 w-full rounded-md border px-2 text-sm"
        value={activeId}
        onchange={(e) => selectProfile((e.target as HTMLSelectElement).value)}
      >
        {#each profiles as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
    </div>

    {#if active}
      <div class="space-y-1.5">
        <Label class="text-xs">Name</Label>
        <Input bind:value={localConfig.tts.profiles[activeIndex].name} />
      </div>

      <div class="space-y-1.5">
        <Label class="text-xs">Engine</Label>
        <select
          class="border-input bg-background h-9 w-full rounded-md border px-2 text-sm"
          value={active.engine}
          onchange={(e) => onEngineChange((e.target as HTMLSelectElement).value as TtsEngine)}
        >
          {#each ENGINES as eng}
            <option value={eng}>{eng}</option>
          {/each}
        </select>
      </div>

      <div class="space-y-1.5">
        <Label class="text-xs">Voice</Label>
        <Input
          bind:value={localConfig.tts.profiles[activeIndex].voice}
          placeholder="voice id / name (blank = provider default)"
        />
      </div>

      <div class="space-y-1.5">
        <Label class="text-xs">Speed: {active.speed.toFixed(2)}x</Label>
        <input
          type="range"
          min="0.5"
          max="2"
          step="0.05"
          class="w-full"
          bind:value={localConfig.tts.profiles[activeIndex].speed}
        />
      </div>

      <div class="space-y-1.5">
        <Label class="text-xs">Pitch: {active.pitch.toFixed(2)}x</Label>
        <input
          type="range"
          min="0.5"
          max="2"
          step="0.05"
          class="w-full"
          bind:value={localConfig.tts.profiles[activeIndex].pitch}
        />
      </div>

      <div class="space-y-1.5">
        <Label class="text-xs">Effect</Label>
        <select
          class="border-input bg-background h-9 w-full rounded-md border px-2 text-sm"
          value={active.effects.active_effect}
          onchange={(e) => {
            const v = (e.target as HTMLSelectElement).value as EffectId;
            localConfig.tts.profiles[activeIndex].effects.active_effect = v;
            localConfig.tts.profiles[activeIndex].effects.enabled = v !== "none";
          }}
        >
          {#each EFFECTS as fx}
            <option value={fx}>{fx}</option>
          {/each}
        </select>
      </div>
    {/if}
  </div>

  {#if active && active.engine === "google"}
    <div class="border-border mt-3 grid grid-cols-1 gap-3 border-t pt-3 sm:grid-cols-2">
      <div class="space-y-1.5">
        <Label class="text-xs">Google API Key</Label>
        <Input type="password" bind:value={localConfig.tts.google.api_key} />
      </div>
      <div class="space-y-1.5">
        <Label class="text-xs">Model</Label>
        <Input bind:value={localConfig.tts.google.model} />
      </div>
    </div>
  {:else if active && active.engine === "microsoft"}
    <div class="border-border mt-3 grid grid-cols-1 gap-3 border-t pt-3 sm:grid-cols-2">
      <div class="space-y-1.5">
        <Label class="text-xs">Microsoft API Key</Label>
        <Input type="password" bind:value={localConfig.tts.microsoft.api_key} />
      </div>
      <div class="space-y-1.5">
        <Label class="text-xs">Endpoint</Label>
        <Input bind:value={localConfig.tts.microsoft.endpoint} placeholder="https://..." />
      </div>
      <div class="space-y-1.5">
        <Label class="text-xs">Model / Deployment</Label>
        <Input bind:value={localConfig.tts.microsoft.model} />
      </div>
    </div>
  {:else if active && active.engine === "http"}
    <div class="border-border mt-3 grid grid-cols-1 gap-3 border-t pt-3 sm:grid-cols-2">
      <div class="space-y-1.5 sm:col-span-2">
        <Label class="text-xs">URL Template</Label>
        <Input bind:value={localConfig.tts.http.url_template} placeholder="http://127.0.0.1:.../v1/audio/speech" />
      </div>
      <div class="space-y-1.5 sm:col-span-2">
        <Label class="text-xs">Body Template (JSON)</Label>
        <Input bind:value={localConfig.tts.http.body_template} />
      </div>
      <div class="space-y-1.5">
        <Label class="text-xs">Response Format</Label>
        <Input bind:value={localConfig.tts.http.response_format} />
      </div>
      <div class="space-y-1.5">
        <Label class="text-xs">Timeout (s)</Label>
        <Input type="number" bind:value={localConfig.tts.http.timeout_secs} />
      </div>
    </div>
  {/if}

  {#if active && active.id === "default"}
    <p class="text-muted-foreground mt-3 text-xs">
      The Default profile mirrors the engine tabs below. Duplicate it to create a named, fully
      independent profile (engine + voice + speed + pitch + effect).
    </p>
  {/if}
</div>
