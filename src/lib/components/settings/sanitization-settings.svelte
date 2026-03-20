<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import { _ } from "svelte-i18n";

  let {
    localConfig = $bindable()
  }: {
    localConfig: import("$lib/types").AppConfig;
  } = $props();

  function update(updates: Partial<any>) {
    Object.assign(localConfig.sanitization, updates);
  }
</script>

<div class="border-border bg-card rounded-lg border p-4 shadow-sm">
  <h3 class="text-card-foreground mb-4 text-lg font-medium">{$_("settings.sanitization.title")}</h3>

  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <div class="flex items-center gap-1.5">
        <Label for="localConfig.sanitization-enabled">{$_("settings.sanitization.enabled")}</Label>
        <InfoTooltip text={$_("settings.sanitization.enabledDescription")} />
      </div>
      <Switch
        id="localConfig.sanitization-enabled"
        bind:checked={localConfig.sanitization.enabled}
      />
    </div>

    {#if localConfig.sanitization.enabled}
      <div class="border-border mt-4 space-y-4 border-t pt-4">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-1.5">
            <Label for="markdown-enabled">{$_("settings.sanitization.markdown")}</Label>
            <InfoTooltip text={$_("settings.sanitization.markdownDescription")} />
          </div>
          <Switch
            id="markdown-enabled"
            checked={localConfig.sanitization.markdown.enabled}
            onchange={(v) =>
              update({
                markdown: { ...localConfig.sanitization.markdown, enabled: v }
              })}
          />
        </div>

        <div class="flex items-center justify-between">
          <div class="flex items-center gap-1.5">
            <Label for="tts-normalization-enabled"
              >{$_("settings.sanitization.ttsNormalization")}</Label
            >
            <InfoTooltip text={$_("settings.sanitization.ttsNormalizationDescription")} />
          </div>
          <Switch
            id="tts-normalization-enabled"
            checked={localConfig.sanitization.tts_normalization.enabled}
            onchange={(v) =>
              update({
                tts_normalization: {
                  ...localConfig.sanitization.tts_normalization,
                  enabled: v
                }
              })}
          />
        </div>
      </div>
    {/if}
  </div>
</div>
