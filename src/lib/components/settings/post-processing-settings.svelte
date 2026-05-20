<script lang="ts">
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
    { value: "custom", label: "Custom" }
  ];

  const promptOptions = [
    {
      value:
        "Rewrite the text to be shorter, clearer, and easier for a developer to understand. Preserve meaning. Return only the rewritten text, not bullets unless the input is already a list.",
      label: "Concise developer"
    },
    {
      value:
        "Clean up grammar, punctuation, spacing, and obvious transcription/copy artifacts. Preserve meaning and technical terms. Return only the cleaned text.",
      label: "Cleanup"
    },
    {
      value:
        "Rewrite in a concise professional tone for a technical audience. Preserve meaning and code identifiers. Return only the rewritten text.",
      label: "Professional"
    },
    {
      value:
        "Summarize the text for a developer in 1-3 concise sentences. Preserve key decisions, requirements, and action items. Return only the summary.",
      label: "Summarize"
    },
    {
      value:
        "Optimize this text for text-to-speech. Remove markdown noise, make punctuation natural for speech, and keep technical meaning. Return only the optimized text.",
      label: "TTS optimized"
    },
    {
      value:
        "Compress aggressively into terse caveman-style developer notes. Keep technical facts, names, paths, and commands exact. Prefer short sentences over lists unless the input is a list. Return only the compressed text.",
      label: "Caveman"
    }
  ];

  let providerConfig = $derived(
    localConfig.post_processing[localConfig.post_processing.provider] as LlmProviderConfig
  );

  function handleProviderChange(e: Event) {
    localConfig.post_processing.provider = (e.target as HTMLSelectElement).value as PostProcessingProvider;
  }

  function handlePromptPresetChange(e: Event) {
    localConfig.post_processing.prompt = (e.target as HTMLSelectElement).value;
  }
</script>

<div class="space-y-4">
  <SettingRow label="AI post-processing" tooltip="Optimize copied text with an LLM before TTS generation.">
    <Switch bind:checked={localConfig.post_processing.enabled} />
  </SettingRow>

  {#if localConfig.post_processing.enabled}
    <SettingRow label="Provider" tooltip="Groq remains the default, with other Handy-style APIs available.">
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

    <SettingRow label="Model" tooltip="Provider model used for post-processing.">
      <Input bind:value={providerConfig.model} class="w-72" />
    </SettingRow>

    <SettingRow label="Endpoint" tooltip="API endpoint for the selected provider.">
      <Input bind:value={providerConfig.endpoint} class="w-72" />
    </SettingRow>

    <SettingRow label="Prompt preset" tooltip="Choose a starting prompt; edit the prompt below for custom behavior.">
      <Select options={promptOptions} value={localConfig.post_processing.prompt} onchange={handlePromptPresetChange} class="w-44" />
    </SettingRow>

    <div class="space-y-2">
      <div class="text-sm font-medium">Prompt</div>
      <Textarea bind:value={localConfig.post_processing.prompt} class="min-h-28" />
    </div>
  {/if}
</div>
