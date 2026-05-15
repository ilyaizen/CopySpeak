<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import type { AppConfig } from "$lib/types";
  import { _ } from "svelte-i18n";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

  function createOptions(values: string[]): Array<{ value: string; label: string }> {
    return values.map((value) => ({ value, label: value }));
  }

  const modelOptions = createOptions(["sonic-3.5", "sonic-3.5-2026-05-04", "sonic-latest"]);
  const formatOptions = createOptions(["wav"]);
</script>

<div class="bg-card border-border rounded-lg border p-3">
  <div class="space-y-4">
    <div class="flex items-center justify-between gap-4">
      <div class="flex w-40 items-center gap-1.5">
        <Label for="cartesia-model" class="whitespace-nowrap">{$_("engine.cartesiaEngine.model")}</Label>
        <InfoTooltip text={$_("engine.cartesiaEngine.modelTooltip")} />
      </div>
      <Select
        id="cartesia-model"
        options={modelOptions}
        value={localConfig.tts.cartesia.model_id}
        onchange={(e) => {
          localConfig.tts.cartesia.model_id = (e.target as HTMLSelectElement).value;
        }}
      />
    </div>

    <div class="flex items-center justify-between gap-4">
      <div class="flex w-40 items-center gap-1.5">
        <Label for="cartesia-voice" class="whitespace-nowrap">{$_("engine.cartesiaEngine.voiceId")}</Label>
        <InfoTooltip text={$_("engine.cartesiaEngine.voiceIdTooltip")} />
      </div>
      <Input
        id="cartesia-voice"
        value={localConfig.tts.cartesia.voice_id}
        placeholder="f786b574-daa5-4673-aa0c-cbe3e8534c02"
        oninput={(e) => {
          localConfig.tts.cartesia.voice_id = (e.target as HTMLInputElement).value;
        }}
      />
    </div>

    <div class="flex items-center justify-between gap-4">
      <div class="flex w-40 items-center gap-1.5">
        <Label for="cartesia-format" class="whitespace-nowrap">{$_("engine.cartesiaEngine.format")}</Label>
        <InfoTooltip text={$_("engine.cartesiaEngine.formatTooltip")} />
      </div>
      <Select
        id="cartesia-format"
        options={formatOptions}
        value={localConfig.tts.cartesia.output_format}
        onchange={(e) => {
          localConfig.tts.cartesia.output_format = (e.target as HTMLSelectElement).value;
        }}
      />
    </div>
  </div>
</div>
