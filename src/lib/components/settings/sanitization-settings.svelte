<script lang="ts">
  import SettingRow from "$lib/components/ui/setting-row/index.js";
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

<div class="space-y-4">
  <SettingRow
    label={$_("settings.sanitization.enabled")}
    tooltip={$_("settings.sanitization.enabledDescription")}
  >
    <Switch id="sanitization-enabled" bind:checked={localConfig.sanitization.enabled} />
  </SettingRow>

  {#if localConfig.sanitization.enabled}
    <div class="border-border mt-4 space-y-4 border-t pt-4">
      <SettingRow
        label={$_("settings.sanitization.markdown")}
        tooltip={$_("settings.sanitization.markdownDescription")}
      >
        <Switch
          id="markdown-enabled"
          checked={localConfig.sanitization.markdown.enabled}
          onchange={(v) =>
            update({
              markdown: { ...localConfig.sanitization.markdown, enabled: v }
            })}
        />
      </SettingRow>

      <SettingRow
        label={$_("settings.sanitization.ttsNormalization")}
        tooltip={$_("settings.sanitization.ttsNormalizationDescription")}
      >
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
      </SettingRow>
    </div>
  {/if}
</div>
