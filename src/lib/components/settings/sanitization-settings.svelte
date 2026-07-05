<script lang="ts">
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { _ } from "svelte-i18n";

  let {
    localConfig = $bindable()
  }: {
    localConfig: import("$lib/types").AppConfig;
  } = $props();

  function updateSanitization(updates: Partial<any>) {
    Object.assign(localConfig.sanitization, updates);
  }

  function updateMarkdown(
    field: keyof import("$lib/types").MarkdownSanitizationConfig,
    value: boolean
  ) {
    localConfig.sanitization.markdown[field] = value;
  }

  const markdownToggles: {
    key: keyof import("$lib/types").MarkdownSanitizationConfig;
    labelKey: string;
    tooltipKey: string;
  }[] = [
    {
      key: "strip_code_blocks",
      labelKey: "stripCodeBlocks",
      tooltipKey: "stripCodeBlocksDescription"
    },
    {
      key: "strip_inline_code",
      labelKey: "stripInlineCode",
      tooltipKey: "stripInlineCodeDescription"
    },
    { key: "strip_headers", labelKey: "stripHeaders", tooltipKey: "stripHeadersDescription" },
    { key: "strip_links", labelKey: "stripLinks", tooltipKey: "stripLinksDescription" },
    {
      key: "strip_bold_italic",
      labelKey: "stripBoldItalic",
      tooltipKey: "stripBoldItalicDescription"
    },
    { key: "strip_lists", labelKey: "stripLists", tooltipKey: "stripListsDescription" },
    {
      key: "strip_blockquotes",
      labelKey: "stripBlockquotes",
      tooltipKey: "stripBlockquotesDescription"
    }
  ];
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
            updateSanitization({
              markdown: { ...localConfig.sanitization.markdown, enabled: v }
            })}
        />
      </SettingRow>

      {#if localConfig.sanitization.markdown.enabled}
        <div class="border-border ml-2 space-y-3 border-l-2 pl-4">
          {#each markdownToggles as toggle}
            <SettingRow
              label={$_(`settings.sanitization.${toggle.labelKey}`)}
              tooltip={$_(`settings.sanitization.${toggle.tooltipKey}`)}
            >
              <Switch
                id={`markdown-${toggle.key}`}
                checked={localConfig.sanitization.markdown[toggle.key]}
                onchange={(v) => updateMarkdown(toggle.key, v)}
              />
            </SettingRow>
          {/each}
        </div>
      {/if}

      <SettingRow
        label={$_("settings.sanitization.ttsNormalization")}
        tooltip={$_("settings.sanitization.ttsNormalizationDescription")}
      >
        <Switch
          id="tts-normalization-enabled"
          checked={localConfig.sanitization.tts_normalization.enabled}
          onchange={(v) =>
            updateSanitization({
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
