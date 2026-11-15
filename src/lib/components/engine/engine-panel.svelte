<script lang="ts">
  // One engine row. Voice and model settings stay in Profiles.

  import { _ } from "svelte-i18n";
  import {
    Download,
    Loader2,
    ExternalLink,
    CheckCircle2,
    XCircle,
    Eye,
    EyeOff
  } from "@lucide/svelte";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { Label } from "#lib/components/ui/label/index.js";
  import { installStore } from "#lib/stores/install-store.svelte";
  import { openExternal } from "#lib/utils/external-link";
  import type { AppConfig } from "#lib/types";
  import type { EngineSetupEntry, TestState } from "./engine-meta";
  import { invoke } from "@tauri-apps/api/core";

  // Runtime verdict for local engines: installed + (ONNX engines) whether the
  // last daemon run used CPU or CUDA. Cheap probe + app-log parse on the Rust
  // side; re-probed when the parent bumps `runtimeProbeKey` (post-install).
  interface RuntimeStatus {
    installed: boolean;
    provider: string | null;
  }

  // Keep each key masked until its own reveal button is used.
  let keyRevealed = $state(false);

  let runtimeStatus = $state<RuntimeStatus | null>(null);

  let {
    entry,
    localConfig = $bindable(),
    testState = "idle",
    testMessage = "",
    onTest,
    onInstall,
    onCredentialChange,
    testDisabled = false,
    runtimeProbeKey = 0,
    envKeySources = {}
  }: {
    entry: EngineSetupEntry;
    localConfig: AppConfig;
    testState?: TestState;
    testMessage?: string;
    onTest?: () => void;
    onInstall?: () => void;
    onCredentialChange?: () => void;
    testDisabled?: boolean;
    /** Bump to re-probe runtime status (e.g. after an install settles). */
    runtimeProbeKey?: number;
    /** engine → winning env var for keys currently provided by .env. */
    envKeySources?: Record<string, string>;
  } = $props();

  const installRunning = $derived(installStore.isRunning(entry.installerId ?? entry.id));
  let runtimeError = $state(false);

  // Re-probe after a run ends, even if its dialog was dismissed.
  $effect(() => {
    void runtimeProbeKey;
    void installRunning;
    let cancelled = false;
    runtimeStatus = null;
    runtimeError = false;
    if (entry.kind !== "local" || !entry.installerId) {
      runtimeStatus = null;
      return;
    }
    void invoke<RuntimeStatus>("engine_runtime_status", { engine: entry.id })
      .then((status) => {
        if (!cancelled) runtimeStatus = status;
      })
      .catch(() => {
        if (!cancelled) runtimeError = true;
      });
    return () => {
      cancelled = true;
    };
  });

  interface CredentialFields {
    api_key?: string;
    endpoint?: string;
  }

  function openDocs() {
    openExternal(entry.docsUrl);
  }

  // .env-supplied keys: the backend never sends the raw secret — this map
  // (from get_env_key_sources) says which engine's key currently comes from
  // .env. While the field is untouched we show a sentinel placeholder and no
  // eye toggle (nothing to reveal); a typed key stays editable and the parent
  // warns at save when env precedence would keep shadowing it.
  const envVarName = $derived(
    entry.credentialTarget ? envKeySources[entry.credentialTarget] : undefined
  );
  const showingEnvSentinel = $derived.by(() => {
    if (!envVarName || !entry.credentialTarget) return false;
    return (localConfig.tts[entry.credentialTarget].api_key ?? "") === "";
  });
  const credentialFields: CredentialFields | undefined = $derived(
    entry.credentialTarget ? localConfig.tts[entry.credentialTarget] : undefined
  );
  const configured = $derived(
    entry.credential === "none" ||
      Boolean(
        (envVarName || credentialFields?.api_key?.trim()) &&
        (entry.credential !== "api_key_endpoint" || credentialFields?.endpoint?.trim())
      )
  );
  const canTest = $derived(entry.kind === "local" ? runtimeStatus?.installed === true : configured);
</script>

<tr class="border-border border-t">
  <th role="rowheader" scope="row" class="px-4 py-4 text-start align-top font-normal break-words">
    <div class="flex flex-wrap items-center gap-2">
      <span class="font-medium">{entry.menuLabel ?? $_(`engine.${entry.id}.title`)}</span>
      <span class="text-muted-foreground text-xs">{$_(`engine.kind.${entry.kind}`)}</span>
    </div>
    <p class="text-muted-foreground mt-1.5 max-w-xs text-xs leading-relaxed">
      {$_(`engine.${entry.id}.description`)}
    </p>
  </th>

  <td role="cell" class="px-4 py-4 align-top">
    {#if entry.credentialTarget && credentialFields}
      <div class="space-y-2">
        <Label for={`api-key-${entry.id}`} class="text-xs">{$_("engine.apiSetup.apiKey")}</Label>
        <div class="relative">
          <Input
            id={`api-key-${entry.id}`}
            aria-label={`${$_(`engine.${entry.id}.title`)} ${$_("engine.apiSetup.apiKey")}`}
            type={keyRevealed ? "text" : "password"}
            autocomplete="off"
            spellcheck={false}
            disabled={testDisabled}
            class="min-w-0 pe-10 text-sm"
            placeholder={showingEnvSentinel
              ? $_("engine.apiSetup.envSentinel")
              : entry.placeholderKey
                ? $_(`engine.apiSetup.${entry.placeholderKey}`)
                : ""}
            value={credentialFields.api_key ?? ""}
            oninput={(e) => {
              if (credentialFields) credentialFields.api_key = e.currentTarget.value;
              onCredentialChange?.();
            }}
          />
          {#if !showingEnvSentinel}
            <button
              type="button"
              class="text-muted-foreground hover:text-foreground focus-visible:ring-ring absolute inset-y-0 end-0 flex w-10 cursor-pointer items-center justify-center rounded focus-visible:ring-2 focus-visible:outline-none"
              onclick={() => (keyRevealed = !keyRevealed)}
              aria-label={`${$_(`engine.${entry.id}.title`)}: ${keyRevealed ? $_("engines.dashboard.hideKey") : $_("engines.dashboard.showKey")}`}
              aria-pressed={keyRevealed}
            >
              {#if keyRevealed}<EyeOff size={14} />{:else}<Eye size={14} />{/if}
            </button>
          {/if}
        </div>
        {#if envVarName}
          <p class="text-muted-foreground text-xs break-all">
            {envVarName} <span class="font-mono">(.env)</span>
          </p>
        {/if}
        {#if entry.credential === "api_key_endpoint"}
          <Label for={`endpoint-${entry.id}`} class="text-xs"
            >{$_("engine.apiSetup.endpointLabel")}</Label
          >
          <Input
            id={`endpoint-${entry.id}`}
            type="url"
            disabled={testDisabled}
            aria-label={`${$_(`engine.${entry.id}.title`)} ${$_("engine.apiSetup.endpointLabel")}`}
            placeholder={$_("engine.apiSetup.endpointPlaceholder")}
            value={credentialFields.endpoint ?? ""}
            oninput={(e) => {
              if (credentialFields) credentialFields.endpoint = e.currentTarget.value;
              onCredentialChange?.();
            }}
          />
        {/if}
      </div>
    {:else if entry.installerId}
      <div class="space-y-1.5">
        <Button
          variant="outline"
          size="sm"
          disabled={testDisabled && !installRunning}
          onclick={() => onInstall?.()}
        >
          {#if installRunning}<Loader2 size={14} class="me-2 animate-spin" />{:else}<Download
              size={14}
              class="me-2"
            />{/if}
          {runtimeStatus?.installed === false && !installRunning
            ? $_("engine.setup.install")
            : $_("engine.setup.installManage")}
        </Button>
        <p class="text-muted-foreground text-xs">{entry.downloadSize}</p>
      </div>
    {:else}
      <p class="text-muted-foreground text-xs">{$_("engines.noCredentialNeeded")}</p>
    {/if}
  </td>

  <td role="cell" class="px-4 py-4 align-top break-words">
    <div class="space-y-1.5 text-xs" role="status">
      {#if installRunning || testState === "testing"}
        <span class="text-muted-foreground inline-flex items-center gap-1.5"
          ><Loader2 size={12} class="animate-spin" />{installRunning
            ? $_("engines.dashboard.installing")
            : $_("engine.testing")}</span
        >
      {:else if testState === "success"}
        <span class="text-foreground inline-flex items-center gap-1.5"
          ><CheckCircle2 size={12} />{$_("engine.apiSetup.testPassed")}</span
        >
      {:else if testState === "fail"}
        <span class="text-destructive inline-flex items-center gap-1.5"
          ><XCircle size={12} />{$_("engine.apiSetup.testFailed")}</span
        >
      {:else if entry.kind === "local"}
        <span class="text-muted-foreground"
          >{runtimeError
            ? $_("engines.dashboard.statusUnknown")
            : !runtimeStatus
              ? $_("common.loading")
              : runtimeStatus.installed
                ? $_("engines.voiceInstalled")
                : $_("engines.dashboard.notInstalled")}</span
        >
      {:else}
        <span class={configured ? "text-foreground" : "text-muted-foreground"}
          >{entry.credential === "none"
            ? $_("engines.dashboard.noKey")
            : configured
              ? $_("engines.dashboard.keySet")
              : $_("engines.dashboard.keyRequired")}</span
        >
      {/if}
      {#if testState === "fail" && testMessage}
        <p class="text-destructive [overflow-wrap:anywhere] whitespace-pre-wrap">{testMessage}</p>
      {/if}
      {#if runtimeStatus?.installed && runtimeStatus.provider}
        <p class="text-muted-foreground uppercase">
          {runtimeStatus.provider === "cuda" ? "GPU (CUDA)" : runtimeStatus.provider}
        </p>
      {/if}
    </div>
  </td>

  <td role="cell" class="px-4 py-4 text-end align-top">
    <div class="engine-actions flex flex-wrap items-center justify-end gap-2">
      <Button
        variant="outline"
        size="sm"
        disabled={testDisabled || !canTest}
        onclick={() => onTest?.()}
        aria-label={`${$_("common.test")} ${$_(`engine.${entry.id}.title`)}`}
      >
        {$_("common.test")}
      </Button>
      <Button
        variant="ghost"
        size="icon"
        onclick={openDocs}
        aria-label={`${$_(`engine.${entry.id}.title`)}: ${$_("engines.docs")}`}
      >
        <ExternalLink size={14} />
      </Button>
    </div>
  </td>
</tr>
