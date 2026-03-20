<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import type { AppConfig } from "$lib/types";
  import { _ } from "svelte-i18n";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

  function createOptions(values: string[]): Array<{ value: string; label: string }> {
    return values.map((value) => ({ value, label: value }));
  }

  const modelOptions = createOptions([
    "gpt-4o-mini-tts",
    "gpt-4o-mini-tts-2025-03-20",
    "gpt-4o-mini-tts-2025-12-15",
    "tts-1",
    "tts-1-1106",
    "tts-1-hd",
    "tts-1-hd-1106"
  ]);

  const voiceOptions = createOptions([
    "Alloy",
    "Ash",
    "Coral",
    "Echo",
    "Fable",
    "Nova",
    "Onyx",
    "Shimmer",
    "Verse"
  ]);
</script>

<div class="bg-card border-border rounded-lg border p-3">
  <div class="space-y-4">
    <div class="flex items-center justify-between gap-4">
      <div class="flex w-40 items-center gap-1.5">
        <Label for="openai-model" class="whitespace-nowrap">{$_("engine.openaiEngine.model")}</Label
        >
        <InfoTooltip text={$_("engine.openaiEngine.modelTooltip")} />
      </div>
      <Select
        id="openai-model"
        options={modelOptions}
        value={localConfig.tts.openai.model}
        onchange={(e) => {
          localConfig.tts.openai.model = (e.target as HTMLSelectElement).value;
        }}
      />
    </div>
    <div class="flex items-center justify-between gap-4">
      <div class="flex w-40 items-center gap-1.5">
        <Label for="openai-voice" class="whitespace-nowrap">{$_("engine.openaiEngine.voice")}</Label
        >
        <InfoTooltip text={$_("engine.openaiEngine.voiceTooltip")} />
      </div>
      <Select
        id="openai-voice"
        options={voiceOptions}
        value={localConfig.tts.openai.voice}
        onchange={(e) => {
          localConfig.tts.openai.voice = (e.target as HTMLSelectElement).value;
        }}
      />
    </div>
  </div>
</div>
