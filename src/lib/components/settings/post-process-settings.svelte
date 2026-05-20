<script lang="ts">
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { _ } from "svelte-i18n";
  import type { AppConfig } from "$lib/types";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

  const DEFAULT_PROMPT = `Rewrite text terse like smart caveman for software developer listening. All technical substance stay. Only fluff die.

Rules:
- Drop articles, filler words, pleasantries, hedging, repetition, and boilerplate.
- Keep technical facts, names, numbers, code identifiers, commands, and original language exact.
- Max 3 bullets/points. No framing or commentary. Output only rewritten text.

Pattern: [thing] [action] [reason]. [next step].

Text:
\${output}`;

  const MODEL_OPTIONS = [
    { value: "openai/gpt-oss-20b", label: "gpt-oss-20b (fast)" },
    { value: "llama-3.3-70b-versatile", label: "llama-3.3-70b (capable)" },
    { value: "llama-3.1-8b-instant", label: "llama-3.1-8b (instant)" }
  ];

  interface CredentialCheckResult {
    success: boolean;
    message: string;
    error_type?: string | null;
  }

  let isTesting = $state(false);

  async function testKey() {
    isTesting = true;
    try {
      const result = await invoke<CredentialCheckResult>("check_groq_credentials");
      if (result.success) {
        toast.success(result.message);
      } else {
        toast.error(result.message);
      }
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      isTesting = false;
    }
  }

  function restoreDefaultPrompt() {
    localConfig.post_process.prompt = DEFAULT_PROMPT;
  }
</script>

<div class="space-y-4">
  <SettingRow
    label={$_("settings.postProcess.enabled")}
    tooltip={$_("settings.postProcess.enabledDescription")}
  >
    <Switch id="post-process-enabled" bind:checked={localConfig.post_process.enabled} />
  </SettingRow>

  {#if localConfig.post_process.enabled}
    <div class="border-border mt-4 space-y-4 border-t pt-4">
      <div class="space-y-2">
        <div class="flex items-center justify-between gap-4">
          <Label for="groq-api-key" class="whitespace-nowrap"
            >{$_("settings.postProcess.apiKey")}</Label
          >
          <div class="flex flex-1 items-center gap-2">
            <Input
              id="groq-api-key"
              type="password"
              autocomplete="off"
              placeholder={$_("settings.postProcess.apiKeyPlaceholder")}
              bind:value={localConfig.post_process.api_key}
            />
            <Button
              variant="outline"
              size="sm"
              onclick={testKey}
              disabled={isTesting || !localConfig.post_process.api_key.trim()}
            >
              {isTesting ? "..." : $_("settings.postProcess.testKey")}
            </Button>
          </div>
        </div>
        <p class="text-muted-foreground text-xs">{$_("settings.postProcess.apiKeyHelp")}</p>
      </div>

      <div class="flex items-center justify-between gap-4">
        <div class="flex items-center gap-1.5">
          <Label for="groq-model" class="whitespace-nowrap"
            >{$_("settings.postProcess.model")}</Label
          >
          <InfoTooltip text={$_("settings.postProcess.modelTooltip")} />
        </div>
        <Select
          id="groq-model"
          options={MODEL_OPTIONS}
          value={localConfig.post_process.model}
          onchange={(e) =>
            (localConfig.post_process.model = (e.target as HTMLSelectElement).value)}
        />
      </div>

      <div class="space-y-2">
        <div class="flex items-center justify-between">
          <Label for="groq-prompt" class="text-sm"
            >{$_("settings.postProcess.prompt")}</Label
          >
          <Button variant="ghost" size="sm" onclick={restoreDefaultPrompt}>
            {$_("settings.postProcess.restoreDefault")}
          </Button>
        </div>
        <textarea
          id="groq-prompt"
          rows="10"
          class="border-input bg-background placeholder:text-muted-foreground focus-visible:ring-ring w-full rounded-md border px-3 py-2 font-mono text-xs focus-visible:ring-1 focus-visible:outline-none"
          bind:value={localConfig.post_process.prompt}
        ></textarea>
        <p class="text-muted-foreground text-xs">{$_("settings.postProcess.promptHelp")}</p>
      </div>
    </div>
  {/if}
</div>
