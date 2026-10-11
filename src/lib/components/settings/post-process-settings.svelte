<script lang="ts">
  import { RefreshCw, Trash2 } from "@lucide/svelte";

  import { Button } from "#lib/components/ui/button/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { Select } from "#lib/components/ui/select/index.js";
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { Switch } from "#lib/components/ui/switch/index.js";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { _ } from "svelte-i18n";
  import type { AppConfig, PostProcessConfig } from "#lib/types";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

  const DEFAULT_PROMPT = `Rewrite text terse like smart caveman for software developer listening. All technical substance stay. Only fluff die.

Rules:
- Drop articles, filler words, pleasantries, hedging, repetition, and boilerplate.
- Keep technical facts, names, numbers, code identifiers, commands, and original language exact.
- Max 3 bullets/points. No framing or commentary. Output only rewritten text.

Pattern: [thing] [action] [reason]. [next step].

Text:
\${output}`;

  interface CredentialCheckResult {
    success: boolean;
    message: string;
    error_type?: string | null;
  }

  let pp = $derived.by(() => {
    // SAFETY: AppConfig.post_process is always a full PostProcessConfig — the
    // backend normalizes it at load and the mock in play-page.svelte matches.
    return localConfig.post_process as PostProcessConfig;
  });
  let activeProvider = $derived.by(() => {
    // SAFETY: provider_id is normalized by the backend at config load; when
    // it dangles (mid-edit), fall back to the first entry like the backend does.
    return pp.providers.find((p) => p.id === pp.provider_id) ?? pp.providers[0];
  });

  let providerOptions = $derived(pp.providers.map((p) => ({ value: p.id, label: p.label })));

  let modelOptions = $state<{ value: string; label: string }[]>([]);
  let isRefreshingModels = $state(false);
  let modelRefreshError = $state("");
  let isTesting = $state(false);

  function handleProviderChange(e: Event) {
    // SAFETY: this handler is only bound to the provider <select>; its target
    // is that element and its options enumerate exactly the registry ids.
    pp.provider_id = (e.target as HTMLSelectElement).value;
    modelOptions = [];
    modelRefreshError = "";
  }

  function addProvider() {
    const id = `custom-${Date.now().toString(36)}`;
    pp.providers = [
      ...pp.providers,
      {
        id,
        label: "New provider",
        base_url: "",
        api_key: "",
        model: ""
      }
    ];
    pp.provider_id = id;
    modelOptions = [];
    modelRefreshError = "";
  }

  function deleteProvider() {
    if (!activeProvider) return;
    pp.providers = pp.providers.filter((p) => p.id !== activeProvider.id);
    // Dangling provider_id is repaired on save by the backend; mirror that
    // here so the UI stays consistent without a save round-trip.
    pp.provider_id = pp.providers[0]?.id ?? "";
    modelOptions = [];
    modelRefreshError = "";
  }

  async function refreshModels() {
    if (!activeProvider) return;
    isRefreshingModels = true;
    modelRefreshError = "";
    try {
      const models = await invoke<string[]>("list_post_process_models", {
        baseUrl: activeProvider.base_url,
        apiKey: activeProvider.api_key
      });
      modelOptions = models.map((model) => ({ value: model, label: model }));
      if (models.length === 0) {
        // SAFETY: svelte-i18n's `$_` returns string for keys present in en.json.
        modelRefreshError = $_("settings.postProcess.modelsEmpty") as string;
      }
    } catch (error) {
      modelRefreshError = error instanceof Error ? error.message : String(error);
    } finally {
      isRefreshingModels = false;
    }
  }

  async function testKey() {
    isTesting = true;
    try {
      const result = await invoke<CredentialCheckResult>("check_post_process_credentials");
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
      <SettingRow
        label={$_("settings.postProcess.provider")}
        tooltip={$_("settings.postProcess.providerTooltip")}
      >
        <div class="flex items-center gap-2">
          <Select
            options={providerOptions}
            value={activeProvider?.id ?? ""}
            onchange={handleProviderChange}
            class="w-44"
          />
          <Button
            variant="outline"
            size="icon-sm"
            aria-label={$_("settings.postProcess.addProvider")}
            onclick={addProvider}
          >
            +
          </Button>
        </div>
      </SettingRow>

      {#if activeProvider}
        <SettingRow
          label={$_("settings.postProcess.name")}
          tooltip={$_("settings.postProcess.nameTooltip")}
        >
          <Input bind:value={activeProvider.label} class="w-56" />
        </SettingRow>

        <SettingRow
          label={$_("settings.postProcess.baseUrl")}
          tooltip={$_("settings.postProcess.baseUrlTooltip")}
        >
          <Input
            bind:value={activeProvider.base_url}
            placeholder="https://api.example.com/v1"
            class="w-56"
          />
        </SettingRow>

        <SettingRow
          label={$_("settings.postProcess.apiKey")}
          tooltip={$_("settings.postProcess.apiKeyHelp")}
        >
          <div class="flex items-center gap-2">
            <Input
              bind:value={activeProvider.api_key}
              type="password"
              autocomplete="off"
              placeholder={$_("settings.postProcess.apiKeyPlaceholder")}
              class="w-56"
            />
            <Button
              variant="outline"
              size="sm"
              onclick={testKey}
              disabled={isTesting || !activeProvider.api_key.trim()}
            >
              {isTesting ? "..." : $_("settings.postProcess.testKey")}
            </Button>
          </div>
        </SettingRow>

        <SettingRow
          label={$_("settings.postProcess.model")}
          tooltip={$_("settings.postProcess.modelTooltip")}
        >
          <div class="flex items-center gap-2">
            {#if modelOptions.length > 0}
              <Select options={modelOptions} bind:value={activeProvider.model} class="w-56" />
            {:else}
              <Input bind:value={activeProvider.model} class="w-56" />
            {/if}
            <Button
              variant="outline"
              size="icon-sm"
              aria-label={$_("settings.postProcess.refreshModels")}
              disabled={isRefreshingModels || !activeProvider.base_url.trim()}
              onclick={refreshModels}
            >
              <RefreshCw class="size-4" />
            </Button>
          </div>
        </SettingRow>

        {#if modelRefreshError}
          <div class="text-destructive text-sm">{modelRefreshError}</div>
        {/if}

        {#if pp.providers.length > 1}
          <SettingRow
            label={$_("settings.postProcess.deleteProvider")}
            tooltip={$_("settings.postProcess.deleteProviderTooltip")}
          >
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={$_("settings.postProcess.deleteProvider")}
              onclick={deleteProvider}
            >
              <Trash2 class="size-4" />
            </Button>
          </SettingRow>
        {/if}
      {/if}

      <div class="space-y-2">
        <div class="flex items-center justify-between">
          <label class="text-sm font-medium" for="post-process-prompt"
            >{$_("settings.postProcess.prompt")}</label
          >
          <Button variant="ghost" size="sm" onclick={restoreDefaultPrompt}>
            {$_("settings.postProcess.restoreDefault")}
          </Button>
        </div>
        <textarea
          id="post-process-prompt"
          rows="10"
          class="border-input bg-background placeholder:text-muted-foreground focus-visible:ring-ring w-full rounded-md border px-3 py-2 font-mono text-xs focus-visible:ring-1 focus-visible:outline-none"
          bind:value={localConfig.post_process.prompt}></textarea>
        <p class="text-muted-foreground text-xs">{$_("settings.postProcess.promptHelp")}</p>
      </div>
    </div>
  {/if}
</div>
