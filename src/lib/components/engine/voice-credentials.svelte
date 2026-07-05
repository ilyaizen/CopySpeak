<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { Key, Download, Loader2, ExternalLink } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { openExternal } from "$lib/utils/external-link";
  import type { AppConfig, TtsEngine } from "$lib/types";

  type CredentialKind = "none" | "api_key" | "api_key_endpoint";
  type CredentialTarget = "openai" | "elevenlabs" | "cartesia" | "google" | "microsoft";

  interface EngineMeta {
    id: TtsEngine;
    credential: CredentialKind;
    credentialTarget?: CredentialTarget;
    docsUrl: string;
    description: string;
    installer?: string;
    placeholderKey?: string;
  }

  const ENGINE_META: Record<string, EngineMeta> = {
    edge: {
      id: "edge",
      credential: "none",
      docsUrl: "https://github.com/rany2/edge-tts",
      description: "Free Microsoft Read Aloud via edge-tts. No API key needed."
    },
    cartesia: {
      id: "cartesia",
      credential: "api_key",
      credentialTarget: "cartesia",
      docsUrl: "https://docs.cartesia.ai/api-reference/tts/bytes",
      description: "Ultra-low latency TTS with emotion control.",
      placeholderKey: "placeholderCartesia"
    },
    elevenlabs: {
      id: "elevenlabs",
      credential: "api_key",
      credentialTarget: "elevenlabs",
      docsUrl: "https://elevenlabs.io/docs/api-reference/text-to-speech/convert",
      description: "High-quality multilingual TTS with voice cloning.",
      placeholderKey: "placeholderElevenlabs"
    },
    openai: {
      id: "openai",
      credential: "api_key",
      credentialTarget: "openai",
      docsUrl: "https://platform.openai.com/docs/guides/text-to-speech",
      description: "OpenAI text-to-speech with multiple voices.",
      placeholderKey: "placeholderOpenai"
    },
    google: {
      id: "google",
      credential: "api_key",
      credentialTarget: "google",
      docsUrl: "https://ai.google.dev/gemini-api/docs/speech-generation",
      description: "Google Gemini TTS — experimental speech generation.",
      placeholderKey: "placeholderGoogle"
    },
    microsoft: {
      id: "microsoft",
      credential: "api_key_endpoint",
      credentialTarget: "microsoft",
      docsUrl: "https://learn.microsoft.com/en-us/azure/ai-services/speech-service/text-to-speech",
      description: "Azure Microsoft AI Speech service (MAI-Voice-2).",
      placeholderKey: "placeholderMicrosoft"
    },
    http: {
      id: "http",
      credential: "none",
      docsUrl: "https://github.com/CopySpeak/CopySpeak-TTS/blob/main/docs/profile-engine-settings.md",
      description: "Generic HTTP TTS — connect to any TTS API."
    },
    local: {
      id: "local",
      credential: "none",
      docsUrl: "",
      description: "Local TTS engines installed on your machine."
    }
  };

  let { engine, localConfig } = $props<{
    engine: TtsEngine;
    localConfig: AppConfig;
  }>();

  let uvAvailable = $state<boolean | null>(null);
  let installingFor = $state<string | null>(null);

  const meta = $derived(ENGINE_META[engine] ?? ENGINE_META.edge);

  type TtsFields = Record<string, { api_key?: string; endpoint?: string }>;
  function tts(): TtsFields {
    return localConfig.tts as unknown as TtsFields;
  }

  async function runInstaller(id: string) {
    installingFor = id;
    try {
      await invoke("install_engine", { engine: id });
      toast.success(`${id} installer launched`);
    } catch (e) {
      toast.error(`Install failed: ${e}`);
    } finally {
      installingFor = null;
    }
  }

  function handleExternalLinkClick(e: Event, url: string) {
    e.preventDefault();
    openExternal(url);
  }

  $effect(() => {
    if (engine === "local") {
      invoke<{ available: boolean }>("check_command_exists", { command: "uv" })
        .then((r) => (uvAvailable = r.available))
        .catch(() => {});
    } else {
      uvAvailable = null;
    }
  });
</script>

{#if meta.credential !== "none"}
  <div class="border-border bg-muted/30 space-y-3 rounded-lg border p-4">
    <div>
      <p class="text-sm font-medium">Engine Credentials</p>
      <p class="text-muted-foreground text-xs">{meta.description}</p>
    </div>

    {#if meta.credential !== "none" && meta.credentialTarget}
      <div class="space-y-2">
        <Label for="api-key">API Key</Label>
        <div class="flex items-center gap-2">
          <Key size={14} class="text-muted-foreground shrink-0" />
          <Input
            id="api-key"
            type="password"
            placeholder={meta.placeholderKey ?? ""}
            value={tts()?.[meta.credentialTarget]?.api_key ?? ""}
            oninput={(e) => {
              const t = tts();
              if (t && meta.credentialTarget) {
                t[meta.credentialTarget].api_key = e.currentTarget.value;
              }
            }}
          />
        </div>
      </div>
    {/if}

    {#if meta.credential === "api_key_endpoint" && meta.credentialTarget}
      <div class="space-y-2">
        <Label for="endpoint">Endpoint</Label>
        <Input
          id="endpoint"
          type="text"
          placeholder="https://your-resource.cognitiveservices.azure.com"
          value={tts()?.[meta.credentialTarget]?.endpoint ?? ""}
          oninput={(e) => {
            const t = tts();
            if (t && meta.credentialTarget) {
              t[meta.credentialTarget].endpoint = e.currentTarget.value;
            }
          }}
        />
      </div>
    {/if}

    {#if meta.docsUrl}
      <button
        onclick={(e) => handleExternalLinkClick(e, meta.docsUrl)}
        class="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 text-xs transition-colors"
      >
        <ExternalLink size={12} />
        {meta.docsUrl}
      </button>
    {/if}
  </div>
{:else if engine === "local"}
  <div class="border-border bg-muted/30 space-y-3 rounded-lg border p-4">
    <div>
      <p class="text-sm font-medium">Local Engine</p>
      <p class="text-muted-foreground text-xs">Runs on your machine. No API key needed.</p>
    </div>
    {#if uvAvailable === false}
      <div class="flex items-center gap-2">
        <Button variant="outline" size="sm" onclick={() => runInstaller("uv")}>
          <Download size={14} class="mr-2" />
          Install uv
        </Button>
      </div>
    {/if}
  </div>
{/if}
