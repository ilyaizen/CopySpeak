<script lang="ts">
  // Presentational card for one engine's setup: credentials, install, test, docs.
  // Owns no IPC state — driven by callbacks from the parent. Per SRP this
  // component renders; the page/engine-setup orchestrates.

  import { _ } from "svelte-i18n";
  import {
    Key,
    Download,
    Loader2,
    ExternalLink,
    CheckCircle2,
    XCircle,
    Eye,
    EyeOff
  } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { openExternal } from "$lib/utils/external-link";
  import type { AppConfig } from "$lib/types";
  import type { CredentialTarget, EngineSetupEntry, TestState } from "./engine-meta";

  // Track which fields are revealed (per credential target, keyed by field name)
  let revealedFields = $state<Record<string, boolean>>({});

  let {
    entry,
    localConfig = $bindable(),
    testState = "idle",
    testMessage = "",
    onTest,
    onInstall
  }: {
    entry: EngineSetupEntry;
    localConfig: AppConfig;
    testState?: TestState;
    testMessage?: string;
    onTest?: () => void;
    onInstall?: () => void;
  } = $props();

  // ponytail: tts config carries per-engine structs indexed by provider name.
  // Index through a record; the typed structs are mirrored here just enough to
  // bind credentials without widening the public TtsConfig type.
  interface CredentialFields {
    api_key?: string;
    endpoint?: string;
  }
  interface TtsFields extends Record<CredentialTarget, CredentialFields> {}
  function tts(): TtsFields {
    const c = localConfig.tts;
    return {
      openai: c.openai,
      elevenlabs: c.elevenlabs,
      cartesia: c.cartesia,
      google: c.google,
      microsoft: c.microsoft
    };
  }

  function openDocs(e: Event) {
    e.preventDefault();
    openExternal(entry.docsUrl);
  }
</script>

<section>
  <header class="border-border flex items-start justify-between gap-3 border-b pb-4">
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

  <div class="space-y-4 py-4">
    {#if entry.credential === "api_key" || entry.credential === "api_key_endpoint"}
      {#if entry.credentialTarget}
        <div class="space-y-2">
          <Label for="api-key">{$_("engine.apiSetup.apiKey")}</Label>
          <div class="flex items-center gap-2">
            <Key size={14} class="text-muted-foreground shrink-0" />
            <Input
              id="api-key"
              type={revealedFields[entry.credentialTarget! + "-api_key"] ? "text" : "password"}
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
            <button
              type="button"
              class="text-muted-foreground hover:text-foreground shrink-0 cursor-pointer transition-colors"
              onclick={() => {
                const key = entry.credentialTarget! + "-api_key";
                revealedFields = { ...revealedFields, [key]: !revealedFields[key] };
              }}
              aria-label={revealedFields[entry.credentialTarget! + "-api_key"]
                ? "Hide API key"
                : "Show API key"}
            >
              {#if revealedFields[entry.credentialTarget! + "-api_key"]}
                <EyeOff size={14} />
              {:else}
                <Eye size={14} />
              {/if}
            </button>
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
        <Button variant="outline" size="sm" onclick={() => onInstall?.()}>
          <Download size={14} class="mr-2" />
          {$_("engine.setup.installManage")}
        </Button>
        <span class="text-muted-foreground text-xs">{$_("engines.installerSmokeTestHint")}</span>
      </div>
    {/if}

    {#if entry.kind === "cloud" || (entry.kind === "local" && entry.id !== "uv")}
      <div class="border-border flex flex-wrap items-center gap-3 border-t pt-3">
        <Button size="sm" disabled={testState === "testing"} onclick={() => onTest?.()}>
          {#if testState === "testing"}
            <Loader2 size={14} class="mr-2 animate-spin" />
            {$_("engine.testing")}
          {:else if entry.kind === "local"}
            {$_("engine.localEngine.testEngine")}
          {:else}
            {$_("engine.apiSetup.testButton")}
          {/if}
        </Button>

        {#if testState === "success"}
          <span
            class="inline-flex items-center gap-1 text-sm text-emerald-600 dark:text-emerald-400"
          >
            <CheckCircle2 size={14} />
            {$_("engine.apiSetup.testPassed")}
          </span>
        {:else if testState === "fail"}
          <span class="text-destructive inline-flex items-center gap-1 text-sm">
            <XCircle size={14} />
            {testMessage || $_("engine.apiSetup.testFailed")}
          </span>
        {/if}
      </div>
    {/if}
  </div>
</section>
