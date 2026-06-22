<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { RefreshCw, Trash2 } from "@lucide/svelte";

  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Textarea } from "$lib/components/ui/textarea/index.js";

  import type { AppConfig, LlmProviderConfig, PostProcessingProvider } from "$lib/types";

  let { localConfig = $bindable() } = $props<{ localConfig: AppConfig }>();

  const providerOptions = [
    { value: "groq", label: "Groq" },
    { value: "openai", label: "OpenAI" },
    { value: "anthropic", label: "Anthropic" },
    { value: "gemini", label: "Gemini" },
    { value: "openrouter", label: "OpenRouter" },
    { value: "ollama", label: "Ollama" },
    { value: "xai", label: "xAI" },
    { value: "aws", label: "AWS Bedrock" },
    { value: "cerebras", label: "Cerebras" },
    { value: "custom", label: "Custom" }
  ];

  let modelOptions = $state<{ value: string; label: string }[]>([]);
  let isRefreshingModels = $state(false);
  let modelRefreshError = $state("");

  let providerConfig = $derived(
    localConfig.post_processing[localConfig.post_processing.provider] as LlmProviderConfig
  );

  let promptOptions = $derived(
    localConfig.post_processing.prompt_presets.map((preset) => ({
      value: preset.label,
      label: preset.label
    }))
  );

  function handleProviderChange(e: Event) {
    localConfig.post_processing.provider = (e.target as HTMLSelectElement).value as PostProcessingProvider;
    modelOptions = [];
    modelRefreshError = "";
  }

  function handlePromptPresetChange(e: Event) {
    const label = (e.target as HTMLSelectElement).value;
    const preset = localConfig.post_processing.prompt_presets.find((item) => item.label === label);
    if (!preset) return;

    localConfig.post_processing.selected_prompt_label = preset.label;
    localConfig.post_processing.prompt = preset.prompt;
  }

  function savePromptPreset() {
    const label = localConfig.post_processing.selected_prompt_label.trim();
    if (!label) return;

    const existing = localConfig.post_processing.prompt_presets.find((item) => item.label === label);
    if (existing) {
      existing.prompt = localConfig.post_processing.prompt;
      return;
    }

    localConfig.post_processing.prompt_presets = [
      ...localConfig.post_processing.prompt_presets,
      { label, prompt: localConfig.post_processing.prompt }
    ];
  }

  function deletePromptPreset() {
    const label = localConfig.post_processing.selected_prompt_label;
    localConfig.post_processing.prompt_presets = localConfig.post_processing.prompt_presets.filter(
      (item) => item.label !== label
    );

    const next = localConfig.post_processing.prompt_presets[0];
    if (next) {
      localConfig.post_processing.selected_prompt_label = next.label;
      localConfig.post_processing.prompt = next.prompt;
    }
  }

  async function refreshModels() {
    isRefreshingModels = true;
    modelRefreshError = "";
    try {
      const models = await invoke<string[]>("list_post_processing_models", {
        provider: localConfig.post_processing.provider,
        config: providerConfig
      });
      modelOptions = models.map((model) => ({ value: model, label: model }));
    } catch (error) {
      modelRefreshError = error instanceof Error ? error.message : String(error);
    } finally {
      isRefreshingModels = false;
    }
  }
</script>

<div class="space-y-4">
  <SettingRow label="LLM Post-Processing" tooltip="Optimize copied text with an LLM before TTS generation.">
    <Switch bind:checked={localConfig.post_processing.enabled} />
  </SettingRow>

  {#if localConfig.post_processing.enabled}
    <SettingRow label="Provider" tooltip="Choose the LLM provider used for Post-Processing.">
      <Select
        options={providerOptions}
        value={localConfig.post_processing.provider}
        onchange={handleProviderChange}
        class="w-44"
      />
    </SettingRow>

    <SettingRow label="API key" tooltip="Ollama/local endpoints can leave this empty.">
      <Input bind:value={providerConfig.api_key} type="password" class="w-72" />
    </SettingRow>

    <SettingRow label="Model" tooltip="Provider model used for Post-Processing.">
      <div class="flex items-center gap-2">
        {#if modelOptions.length > 0}
          <Select options={modelOptions} bind:value={providerConfig.model} class="w-72" />
        {:else}
          <Input bind:value={providerConfig.model} class="w-72" />
        {/if}
        <Button
          variant="outline"
          size="icon-sm"
          aria-label="Refresh models"
          disabled={isRefreshingModels}
          onclick={refreshModels}
        >
          <RefreshCw class="size-4" />
        </Button>
      </div>
    </SettingRow>

    {#if modelRefreshError}
      <div class="text-destructive text-sm">{modelRefreshError}</div>
    {/if}

    <SettingRow label="Endpoint" tooltip="API endpoint for the selected provider.">
      <Input bind:value={providerConfig.endpoint} class="w-72" />
    </SettingRow>

    <SettingRow label="Prompt label" tooltip="Select, rename, add, update, or delete saved prompt labels.">
      <div class="flex items-center gap-2">
        <Select
          options={promptOptions}
          value={localConfig.post_processing.selected_prompt_label}
          onchange={handlePromptPresetChange}
          class="w-44"
        />
        <Input bind:value={localConfig.post_processing.selected_prompt_label} class="w-44" />
        <Button variant="outline" size="sm" onclick={savePromptPreset}>Save</Button>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Delete prompt label"
          disabled={localConfig.post_processing.prompt_presets.length <= 1}
          onclick={deletePromptPreset}
        >
          <Trash2 class="size-4" />
        </Button>
      </div>
    </SettingRow>

    <div class="space-y-2">
      <div class="text-sm font-medium">Prompt</div>
      <Textarea bind:value={localConfig.post_processing.prompt} class="min-h-28" />
    </div>
  {/if}
</div>
