<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { Slider } from "$lib/components/ui/slider/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { invoke } from "@tauri-apps/api/core";
  import type { AppConfig } from "$lib/types";
  import { openExternal } from "$lib/utils/external-link";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import { untrack } from "svelte";
  import { _ } from "svelte-i18n";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

  interface ElevenLabsVoice {
    voice_id: string;
    name?: string;
    category?: string;
    labels?: Record<string, string>;
    description?: string;
    preview_url?: string;
  }

  // Module-level voice cache (persists across component instances)
  let voiceCache: ElevenLabsVoice[] = [];

  let voices = $state<ElevenLabsVoice[]>([]);
  let isLoading = $state(false);
  let loadError = $state<string | null>(null);
  let isVerifying = $state(false);
  let verifiedVoice = $state<ElevenLabsVoice | null>(null);

  // Initialize voices from cache if available
  if (voiceCache.length > 0) {
    voices = voiceCache;
  }

  // Style slider: syncs FROM config when parent cancels
  // Sync TO config happens via Slider's onchange callback (avoids race condition)
  // untrack() prevents tracking styleValue - effect only runs when localConfig changes
  let styleValue = $state(localConfig.tts.elevenlabs.voice_style ?? 0);

  $effect(() => {
    const configValue = localConfig.tts.elevenlabs.voice_style ?? 0;
    if (untrack(() => styleValue) !== configValue) {
      styleValue = configValue;
    }
  });

  async function loadVoices() {
    if (!localConfig.tts.elevenlabs.api_key) {
      loadError = $_("engine.elevenlabsEngine.setupApiKeyFirst");
      return;
    }

    // Use cache if available
    if (voiceCache.length > 0) {
      voices = voiceCache;
      return;
    }

    isLoading = true;
    loadError = null;
    try {
      const result = await invoke<ElevenLabsVoice[]>("list_elevenlabs_voices");
      voices = result ?? [];
      voiceCache = voices;
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      isLoading = false;
    }
  }

  async function verifyVoiceId() {
    if (!localConfig.tts.elevenlabs.voice_id.trim() || !localConfig.tts.elevenlabs.api_key) return;
    isVerifying = true;
    verifiedVoice = null;
    try {
      const voice = await invoke<ElevenLabsVoice>("get_elevenlabs_voice_by_id", {
        voiceId: localConfig.tts.elevenlabs.voice_id
      });
      verifiedVoice = voice;
      if (voice.name) {
        localConfig.tts.elevenlabs.voice_name = voice.name.split(" -")[0].trim();
      }
    } catch {
      verifiedVoice = null;
      localConfig.tts.elevenlabs.voice_name = undefined;
    } finally {
      isVerifying = false;
    }
  }

  const modelOptions = [
    { value: "eleven_turbo_v2_5", label: "Turbo v2.5 (Fastest)" },
    { value: "eleven_turbo_v2", label: "Turbo v2 (Fast)" },
    { value: "eleven_multilingual_v2", label: "Multilingual v2 (Latest)" },
    { value: "eleven_multilingual_v1", label: "Multilingual v1" },
    { value: "eleven_monolingual_v1", label: "Monolingual v1" }
  ];

  const formatOptions = [
    { value: "mp3_44100_128", label: "MP3 44.1kHz 128kbps" },
    { value: "mp3_44100_192", label: "MP3 44.1kHz 192kbps" },
    { value: "mp3_44100_32", label: "MP3 44.1kHz 32kbps" },
    { value: "pcm_44100", label: "PCM 44.1kHz 16-bit" },
    { value: "flac_44100", label: "FLAC 44.1kHz (Lossless)" }
  ];
</script>

<div class="space-y-3">
  <div class="flex items-center justify-between gap-4">
    <div class="flex items-center gap-1.5">
      <Label for="voice-mode" class="whitespace-nowrap"
        >{$_("engine.elevenlabsEngine.manualVoiceId")}</Label
      >
      <InfoTooltip text={$_("engine.elevenlabsEngine.manualVoiceIdTooltip")} />
    </div>
    <Switch
      id="voice-mode"
      checked={!!localConfig.tts.elevenlabs.use_manual_voice_id}
      onchange={(v) => (localConfig.tts.elevenlabs.use_manual_voice_id = v)}
    />
  </div>

  {#if localConfig.tts.elevenlabs.use_manual_voice_id}
    <div class="space-y-2">
      <div class="flex items-center justify-between gap-4">
        <Label for="voice-id" class="w-24">{$_("engine.elevenlabsEngine.voiceId")}</Label>
        <div class="flex flex-1 items-center gap-2">
          <Input
            id="voice-id"
            type="text"
            placeholder={$_("engine.elevenlabsEngine.voiceIdPlaceholder")}
            value={localConfig.tts.elevenlabs.voice_id}
            onchange={(e) => {
              localConfig.tts.elevenlabs.voice_id = (e.target as HTMLInputElement).value;
              verifiedVoice = null;
            }}
          />
          <Button
            variant="outline"
            size="sm"
            onclick={verifyVoiceId}
            disabled={isVerifying || !localConfig.tts.elevenlabs.voice_id.trim()}
          >
            {isVerifying ? "..." : $_("engine.elevenlabsEngine.verify")}
          </Button>
        </div>
      </div>
      {#if verifiedVoice}
        <p class="text-xs text-emerald-600">
          {$_("engine.elevenlabsEngine.verified")}: {verifiedVoice.name || verifiedVoice.voice_id}
        </p>
      {:else if localConfig.tts.elevenlabs.voice_name}
        <p class="text-muted-foreground text-xs">
          {$_("engine.elevenlabsEngine.cached")}: {localConfig.tts.elevenlabs.voice_name}
        </p>
      {:else}
        <p class="text-muted-foreground text-xs">
          {$_("engine.elevenlabsEngine.findVoices")}
          <button
            onclick={() => openExternal("https://elevenlabs.io/app/voices")}
            class="underline"
          >
            elevenlabs.io/app/voices
          </button>
        </p>
      {/if}
    </div>
  {:else}
    <div class="flex items-center justify-between gap-4">
      <div class="flex items-center gap-1.5">
        <Label for="voice-select" class="whitespace-nowrap"
          >{$_("engine.elevenlabsEngine.voice")}</Label
        >
        <InfoTooltip text={$_("engine.elevenlabsEngine.voiceTooltip")} />
      </div>
      <div class="flex items-center gap-2">
        <Select
          id="voice-select"
          options={voices.map((v) => ({
            value: v.voice_id,
            label: (v.name || v.voice_id) + (v.category ? ` (${v.category})` : "")
          }))}
          value={localConfig.tts.elevenlabs.voice_id}
          onchange={(e) => {
            const id = (e.target as HTMLSelectElement).value;
            localConfig.tts.elevenlabs.voice_id = id;
            const found = voices.find((v) => v.voice_id === id);
            if (found?.name) {
              localConfig.tts.elevenlabs.voice_name = found.name.split(" -")[0].trim();
            }
          }}
        />
        <Button
          variant="ghost"
          size="sm"
          onclick={loadVoices}
          disabled={isLoading || !localConfig.tts.elevenlabs.api_key}
        >
          {isLoading ? "..." : $_("engine.elevenlabsEngine.load")}
        </Button>
      </div>
    </div>
    {#if loadError}
      <p class="text-destructive text-xs">{loadError}</p>
    {:else if voices.length === 0 && localConfig.tts.elevenlabs.api_key}
      <p class="text-muted-foreground text-xs">{$_("engine.elevenlabsEngine.clickLoadToFetch")}</p>
    {:else if !localConfig.tts.elevenlabs.api_key}
      <p class="text-muted-foreground text-xs">
        {$_("engine.elevenlabsEngine.setupApiKeyInSettings")}
      </p>
    {/if}
  {/if}

  <div class="flex items-center justify-between gap-4">
    <div class="flex items-center gap-1.5">
      <Label for="model" class="whitespace-nowrap">{$_("engine.elevenlabsEngine.model")}</Label>
      <InfoTooltip text={$_("engine.elevenlabsEngine.modelTooltip")} />
    </div>
    <Select
      id="model"
      options={modelOptions}
      value={localConfig.tts.elevenlabs.model_id}
      onchange={(e) =>
        (localConfig.tts.elevenlabs.model_id = (e.target as HTMLSelectElement).value)}
    />
  </div>

  <div class="flex items-center justify-between gap-4">
    <div class="flex items-center gap-1.5">
      <Label for="format" class="whitespace-nowrap">{$_("engine.elevenlabsEngine.format")}</Label>
      <InfoTooltip text={$_("engine.elevenlabsEngine.formatTooltip")} />
    </div>
    <Select
      id="format"
      options={formatOptions}
      value={localConfig.tts.elevenlabs.output_format}
      onchange={(e) =>
        (localConfig.tts.elevenlabs.output_format = (e.target as HTMLSelectElement).value as any)}
    />
  </div>

  <div class="border-border space-y-3 rounded-lg border p-3">
    <h4 class="text-xs font-medium">{$_("engine.elevenlabsEngine.voiceSettings")}</h4>

    <div class="space-y-1.5">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1.5">
          <Label for="stability" class="text-xs">{$_("engine.elevenlabsEngine.stability")}</Label>
          <InfoTooltip text={$_("engine.elevenlabsEngine.stabilityTooltip")} />
        </div>
        <span class="text-muted-foreground font-mono text-xs">
          {localConfig.tts.elevenlabs.voice_stability.toFixed(2)}
        </span>
      </div>
      <Slider
        id="stability"
        min={0}
        max={1}
        step={0.01}
        bind:value={localConfig.tts.elevenlabs.voice_stability}
      />
    </div>

    <div class="space-y-1.5">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1.5">
          <Label for="similarity" class="text-xs">{$_("engine.elevenlabsEngine.similarity")}</Label>
          <InfoTooltip text={$_("engine.elevenlabsEngine.similarityTooltip")} />
        </div>
        <span class="text-muted-foreground font-mono text-xs">
          {localConfig.tts.elevenlabs.voice_similarity_boost.toFixed(2)}
        </span>
      </div>
      <Slider
        id="similarity"
        min={0}
        max={1}
        step={0.01}
        bind:value={localConfig.tts.elevenlabs.voice_similarity_boost}
      />
    </div>

    <div class="space-y-1.5">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1.5">
          <Label for="style" class="text-xs">{$_("engine.elevenlabsEngine.style")}</Label>
          <InfoTooltip text={$_("engine.elevenlabsEngine.styleTooltip")} />
        </div>
        <span class="text-muted-foreground font-mono text-xs">
          {styleValue.toFixed(2)}
        </span>
      </div>
      <Slider
        id="style"
        min={0}
        max={1}
        step={0.01}
        bind:value={styleValue}
        onchange={(v) => {
          localConfig.tts.elevenlabs.voice_style = v;
        }}
      />
    </div>

    <div class="flex items-center justify-between">
      <div class="flex items-center gap-1.5">
        <Label for="boost" class="text-xs">{$_("engine.elevenlabsEngine.speakerBoost")}</Label>
        <InfoTooltip text={$_("engine.elevenlabsEngine.speakerBoostTooltip")} />
      </div>
      <Switch
        id="boost"
        checked={!!localConfig.tts.elevenlabs.use_speaker_boost}
        onchange={(v) => (localConfig.tts.elevenlabs.use_speaker_boost = v)}
      />
    </div>
  </div>
</div>
