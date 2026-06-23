<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { Slider } from "$lib/components/ui/slider/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import type { AppConfig } from "$lib/types";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import { untrack } from "svelte";
  import { _ } from "svelte-i18n";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

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
