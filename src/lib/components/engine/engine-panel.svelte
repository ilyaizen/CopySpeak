<script lang="ts">
  // Presentational card for one engine's setup: credentials, install, test, docs.
  // Owns no IPC state — driven by callbacks from the parent. Per SRP this
  // component renders; the page/engine-setup orchestrates.

  import { _ } from "svelte-i18n";
  import { Key, Download, Loader2, ExternalLink, CheckCircle2, XCircle } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { openExternal } from "$lib/utils/external-link";
  import type { AppConfig } from "$lib/types";
  import type { EngineSetupEntry, TestState, InstallState } from "./engine-meta";

  let {
    entry,
    localConfig = $bindable(),
    testState = "idle",
    testMessage = "",
    onTest,
    installState = "idle",
    onInstall
  }: {
    entry: EngineSetupEntry;
    localConfig: AppConfig;
    testState?: TestState;
    testMessage?: string;
    onTest?: () => void;
    installState?: InstallState;
    onInstall?: () => void;
  } = $props();

  // ponytail: tts config carries per-engine structs indexed by provider name.
  // Index through a record; the typed structs are mirrored here just enough to
  // bind credentials without widening the public TtsConfig type.
  type TtsFields = Record<string, { api_key?: string; endpoint?: string }>;
  function tts(): TtsFields {
    return localConfig.tts as unknown as TtsFields;
  }

  function openDocs(e: Event) {
    e.preventDefault();
    openExternal(entry.docsUrl);
  }
</script>

<section class="border-border overflow-hidden rounded-lg border">
  <header class="bg-muted/50 border-border flex items-start justify-between gap-3 border-b p-4">
    <div class="min-w-0">
      <h2 class="text-lg font-semibold">{$_(`engine.${entry.id}.title`)}</h2>
      <p class="text-muted-foreground mt-1 text-sm">{$_(`engine.${entry.id}.description`)}</p>
    </div>
    <button
      onclick={openDocs}
      class="text-muted-foreground hover:text-foreground inline-flex shrink-0 cursor-pointer items-center gap-1 text-xs transition-colors"
    >
      <ExternalLink size={12} />
      {$_("engines.docs")}
    </button>
  </header>

  <div class="space-y-4 p-4">
    {#if entry.credential === "api_key" || entry.credential === "api_key_endpoint"}
      {#if entry.credentialTarget}
        <div class="space-y-2">
          <Label for="api-key">{$_("engine.apiSetup.apiKey")}</Label>
          <div class="flex items-center gap-2">
            <Key size={14} class="text-muted-foreground shrink-0" />
            <Input
              id="api-key"
              type="password"
              placeholder={entry.placeholderKey
                ? $_(`engine.apiSetup.${entry.placeholderKey}`)
                : ""}
              value={tts()[entry.credentialTarget]?.api_key ?? ""}
              oninput={(e) => {
                const t = tts();
                if (t[entry.credentialTarget!]) {
                  t[entry.credentialTarget!].api_key = e.currentTarget.value;
                }
              }}
            />
          </div>
        </div>
      {/if}

      {#if entry.credential === "api_key_endpoint" && entry.credentialTarget}
        <div class="space-y-2">
          <Label for="endpoint">{$_("engine.apiSetup.endpointLabel")}</Label>
          <Input
            id="endpoint"
            type="text"
            placeholder={$_("engine.apiSetup.endpointPlaceholder")}
            value={tts()[entry.credentialTarget]?.endpoint ?? ""}
            oninput={(e) => {
              const t = tts();
              if (t[entry.credentialTarget!]) {
                t[entry.credentialTarget!].endpoint = e.currentTarget.value;
              }
            }}
          />
        </div>
      {/if}
    {:else if entry.kind === "cloud"}
      <p class="text-muted-foreground text-sm">{$_("engines.noCredentialNeeded")}</p>
    {/if}

    {#if entry.installerId}
      <div class="flex flex-wrap items-center gap-3">
        <Button
          variant="outline"
          size="sm"
          disabled={installState === "installing"}
          onclick={() => onInstall?.()}
        >
          {#if installState === "installing"}
            <Loader2 size={14} class="mr-2 animate-spin" />
            {$_("engine.setup.installing")}
          {:else}
            <Download size={14} class="mr-2" />
            {$_("engine.setup.install")}
          {/if}
        </Button>
        <span class="text-muted-foreground text-xs">{$_("engines.installerSmokeTestHint")}</span>
      </div>
    {/if}

    {#if entry.kind === "cloud"}
      <div class="border-border flex flex-wrap items-center gap-3 border-t pt-3">
        <Button size="sm" disabled={testState === "testing"} onclick={() => onTest?.()}>
          {#if testState === "testing"}
            <Loader2 size={14} class="mr-2 animate-spin" />
            {$_("engine.testing")}
          {:else}
            {$_("engine.apiSetup.testButton")}
          {/if}
        </Button>

        {#if testState === "success"}
          <span class="inline-flex items-center gap-1 text-sm text-emerald-600 dark:text-emerald-400">
            <CheckCircle2 size={14} />
            {$_("engine.apiSetup.testPassed")}
          </span>
        {:else if testState === "fail"}
          <span class="inline-flex items-center gap-1 text-sm text-destructive">
            <XCircle size={14} />
            {testMessage || $_("engine.apiSetup.testFailed")}
          </span>
        {/if}
      </div>
    {/if}
  </div>
</section>
