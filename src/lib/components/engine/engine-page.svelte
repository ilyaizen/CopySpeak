<script lang="ts">
  import { onMount } from "svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { ExternalLink, Key, CheckCircle, XCircle } from "@lucide/svelte";
  import { Spinner } from "$lib/components/ui/spinner/index.js";
  import LocalEngine from "./local-engine.svelte";
  import OpenAiEngine from "./openai-engine.svelte";
  import ElevenLabsEngine from "./elevenlabs-engine.svelte";
  import CartesiaEngine from "./cartesia-engine.svelte";
  import type { AppConfig } from "$lib/types";
  import { openExternal } from "$lib/utils/external-link";
  import {
    ExternalLink,
    Key,
    CheckCircle,
    XCircle,
    Terminal,
    Download,
    Loader2
  } from "@lucide/svelte";
  import { Spinner } from "$lib/components/ui/spinner/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { openExternal } from "$lib/utils/external-link";
  import { _ } from "svelte-i18n";
  import type { AppConfig, TtsEngine } from "$lib/types";

  let localConfig = $state<AppConfig | null>(null);
  let originalConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  let isSaving = $state(false);
  let activeTab = $state<string>("");
  let isTesting = $state(false);

  // Cloud TTS Dialog state
  let cloudDialogOpen = $state(false);
  let cloudDialogEngine = $state<"openai" | "elevenlabs" | "cartesia" | null>(null);
  let tempApiKey = $state("");
  let isCheckingCreds = $state(false);
  let isTestingEngine = $state(false);
  let credCheckResult = $state<{ success: boolean; message: string } | null>(null);
  let engineTestResult = $state<{ success: boolean; message: string } | null>(null);

  const LOCAL_PRESET_CONFIGS: Record<string, { command: string; args: string[] }> = {
    "kitten-tts": {
      command: "py",
      args: [
        "-3.12",
        "{home_dir}/kittentts/kittentts-cli.py",
        "--text",
        "{raw_text}",
        "--voice",
        "{voice}",
        "--output",
        "{output}"
      ]
    },
    piper: {
      command: "python3",
      args: [
        "-m",
        "piper",
        "--data-dir",
        "{data_dir}",
        "-m",
        "{voice}",
        "-f",
        "{output}",
        "--input-file",
        "{input}"
      ]
    },
    "kokoro-tts": {
      command: "kokoro-tts",
      args: ["{input}", "{output}", "--voice", "{voice}"]
    },
    "pocket-tts": {
      command: "pocket-tts",
      args: ["generate", "--voice", "{voice}", "--text", "{raw_text}", "--output-path", "{output}"]
    }
  };

  type BadgeKind = "default" | "offline" | "free" | "cloud" | "paid" | "freemium";
  type Location = "local" | "cloud";
  type CredentialKind = "none" | "api_key" | "api_key_endpoint";
  type CredentialTarget = "openai" | "elevenlabs" | "cartesia" | "google" | "microsoft";

  interface EngineMeta {
    badges: BadgeKind[];
    location: LocationKind;
    link: string | null;
    linkLabel: string | null;
  }

  interface EngineCategory {
    id: string;
    location: Location;
    badges: BadgeKind[];
    docsUrl: string;
    credential: CredentialKind;
    credentialTarget?: CredentialTarget;
    /** Engine id passed to test_tts_engine_config (null = no setup test). */
    testEngine?: TtsEngine;
    /** Engine id passed to install_engine (null = no installer button). */
    installer?: string;
  }

  const ENGINE_TABS: EngineTab[] = [
    {
      id: "edge",
      location: "cloud",
      badges: ["default", "cloud", "free"],
      docsUrl: "https://github.com/rany2/edge-tts",
      credential: "none",
      testEngine: "edge",
      installer: "edge"
    },
    {
      id: "cartesia",
      location: "cloud",
      badges: ["cloud", "freemium"],
      docsUrl: "https://docs.cartesia.ai/api-reference/tts/bytes",
      credential: "api_key",
      credentialTarget: "cartesia",
      testEngine: "cartesia"
    },
    {
      id: "elevenlabs",
      location: "cloud",
      badges: ["cloud", "freemium"],
      docsUrl: "https://elevenlabs.io/docs/api-reference/text-to-speech/convert",
      credential: "api_key",
      credentialTarget: "elevenlabs",
      testEngine: "elevenlabs"
    },
    {
      id: "openai",
      location: "cloud",
      badges: ["cloud", "paid"],
      docsUrl: "https://platform.openai.com/docs/guides/text-to-speech",
      credential: "api_key",
      credentialTarget: "openai",
      testEngine: "openai"
    },
    {
      id: "google",
      location: "cloud",
      badges: ["cloud", "freemium"],
      docsUrl: "https://ai.google.dev/gemini-api/docs/speech-generation",
      credential: "api_key",
      credentialTarget: "google",
      testEngine: "google"
    },
    {
      id: "microsoft",
      location: "cloud",
      badges: ["cloud", "paid"],
      docsUrl:
        "https://learn.microsoft.com/en-us/azure/ai-services/speech-service/text-to-speech",
      credential: "api_key_endpoint",
      credentialTarget: "microsoft",
      testEngine: "microsoft"
    },
    {
      id: "kitten",
      location: "local",
      badges: ["offline", "free"],
      docsUrl: "https://github.com/KittenML/KittenTTS",
      credential: "none",
      installer: "kitten"
    },
    {
      id: "piper",
      location: "local",
      badges: ["offline", "free"],
      docsUrl: "https://github.com/OHF-Voice/piper1-gpl",
      credential: "none",
      installer: "piper"
    },
    {
      id: "kokoro",
      location: "local",
      badges: ["offline", "free"],
      docsUrl: "https://github.com/hexgrad/kokoro",
      credential: "none",
      installer: "kokoro"
    },
    {
      id: "pocket",
      location: "local",
      badges: ["offline", "free"],
      docsUrl: "https://github.com/pocket-tts/pocket-tts",
      credential: "none",
      installer: "pocket"
    },
    {
      id: "chatterbox",
      location: "local",
      badges: ["offline", "free"],
      docsUrl: "https://github.com/resemble-ai/chatterbox",
      credential: "none",
      installer: "chatterbox"
    },
    {
      id: "http",
      location: "cloud",
      badges: ["cloud"],
      docsUrl: "docs/profile-engine-settings.md",
      credential: "none"
    }
  ];

  const BADGE_STYLES: Record<BadgeKind, string> = {
    default: "bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 ring-1 ring-emerald-500/30",
    offline: "bg-blue-500/15 text-blue-700 dark:text-blue-400 ring-1 ring-blue-500/30",
    free: "bg-green-500/10 text-green-700 dark:text-green-400 ring-1 ring-green-500/25",
    cloud: "bg-violet-500/15 text-violet-700 dark:text-violet-400 ring-1 ring-violet-500/30",
    paid: "bg-amber-500/15 text-amber-700 dark:text-amber-400 ring-1 ring-amber-500/30",
    freemium: "bg-yellow-500/15 text-yellow-700 dark:text-yellow-400 ring-1 ring-yellow-500/30"
  };

  const getBadgeLabel = (badge: BadgeKind): string => $_(`engine.badges.${badge}`);

  const ENGINE_CATEGORIES: EngineCategory[] = [
    {
      id: "cartesia",
      meta: {
        badges: ["default", "cloud", "freemium"],
        location: "cloud",
        link: "https://docs.cartesia.ai/get-started/overview",
        linkLabel: "API Docs"
      }
    },
    {
      id: "kitten",
      meta: {
        badges: ["default", "offline", "free"],
        location: "local",
        link: "https://github.com/KittenML/KittenTTS",
        linkLabel: "GitHub"
      }
    },
    {
      id: "piper",
      meta: {
        badges: ["offline", "free"],
        location: "local",
        link: "https://github.com/OHF-Voice/piper1-gpl",
        linkLabel: "Setup Guide"
      }
    },
    {
      id: "kokoro",
      meta: {
        badges: ["offline", "free"],
        location: "local",
        link: "https://github.com/hexgrad/kokoro",
        linkLabel: "GitHub README"
      }
    },
    {
      id: "pocket",
      meta: {
        badges: ["offline", "free"],
        location: "local",
        link: null,
        linkLabel: null
      }
    },
    {
      id: "elevenlabs",
      meta: {
        badges: ["cloud", "freemium"],
        location: "cloud",
        link: "https://elevenlabs.io/docs/api-reference/text-to-speech",
        linkLabel: "API Docs"
      }
    },
    {
      id: "openai",
      meta: {
        badges: ["cloud", "paid"],
        location: "cloud",
        link: "https://platform.openai.com/docs/guides/text-to-speech",
        linkLabel: "API Docs"
      }
    }
  ];

  let localConfig = $state<AppConfig | null>(null);
  let originalConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  let isSaving = $state(false);
  let activeTab = $state<string>("edge");

  const currentMeta = $derived(getMeta(activeTab));

  function cloudEngineName(engine: "openai" | "elevenlabs" | "cartesia"): string {
    if (engine === "openai") return "OpenAI";
    if (engine === "cartesia") return "Cartesia";
    return "ElevenLabs";
  }

  const apiSetupTitle = $derived(() => {
    if (!cloudDialogEngine) return "";
    return $_("engine.apiSetup.title").replace("{engine}", cloudEngineName(cloudDialogEngine));
  });

  const apiSetupDescription = $derived(() => {
    if (!cloudDialogEngine) return "";
    return $_("engine.apiSetup.description").replace(
      "{engine}",
      cloudEngineName(cloudDialogEngine)
    );
  });

  const hasChanges = $derived(
    originalConfig !== null &&
      localConfig !== null &&
      JSON.stringify(localConfig) !== JSON.stringify(originalConfig)
  );

  async function loadConfig() {
    isLoading = true;
    try {
      const config = await invoke<AppConfig>("get_config");
      localConfig = JSON.parse(JSON.stringify(config));
      originalConfig = JSON.parse(JSON.stringify(config));
      activeTab ||= "cartesia";
    } catch (e) {
      console.error("Failed to load config:", e);
      toast.error("Failed to load configuration");
    } finally {
      isLoading = false;
    }
  }

  async function saveConfig() {
    if (!localConfig) return;
    isSaving = true;
    try {
      await invoke("set_config", { newConfig: localConfig });
      originalConfig = JSON.parse(JSON.stringify(localConfig));
      toast.success("Engine settings saved");
    } catch (e) {
      console.error("Failed to save config:", e);
      toast.error(`Failed to save settings: ${e}`);
    } finally {
      isSaving = false;
    }
  }

  function cancelChanges() {
    if (!originalConfig) return;
    localConfig = JSON.parse(JSON.stringify(originalConfig));
  }

  // ── Credential helpers ─────────────────────────────────────────────────────

  function apiKey(target: CredentialTarget): string {
    if (!localConfig) return "";
    return localConfig.tts[target].api_key ?? "";
  }

  function setApiKey(tab: EngineTab, value: string) {
    if (!localConfig || !tab.credentialTarget) return;
    localConfig.tts[tab.credentialTarget].api_key = value;
  }

  function endpoint(): string {
    if (!localConfig) return "";
    return localConfig.tts.microsoft.endpoint ?? "";
  }

  function setEndpoint(value: string) {
    if (!localConfig) return;
    localConfig.tts.microsoft.endpoint = value;
  }

  // ── Setup test (uses the engine's default voice via global config) ────────

  async function runSetupTest(tab: EngineTab) {
    if (!localConfig || !tab.testEngine) return;
    testingFor = tab.id;
    testResult = null;
    try {
      await invoke("speak_now", {
        text: "Hello from CopySpeak TTS. This is a voice test."
      });
    } catch (e) {
      testResult = { engine: tab.id, success: false, message: String(e) };
    } finally {
      testingFor = null;
    }
  }

  function handleExternalLinkClick(e: Event, url: string) {
    e.preventDefault();
    openExternal(url);
  }

  // Cloud TTS Dialog functions
  function openCloudDialog(engine: "openai" | "elevenlabs" | "cartesia") {
    cloudDialogEngine = engine;
    tempApiKey = localConfig?.tts[engine].api_key ?? "";
    credCheckResult = null;
    engineTestResult = null;
    cloudDialogOpen = true;
  }

  async function closeCloudDialog(reloadConfig = true) {
    cloudDialogOpen = false;
    cloudDialogEngine = null;
    tempApiKey = "";
    credCheckResult = null;
    engineTestResult = null;
    // Reload config to discard any temporary changes made during testing (unless we just saved)
    if (reloadConfig) {
      await loadConfig();
    }
  }

  function saveApiKeyToConfig() {
    if (!localConfig || !cloudDialogEngine) return;
    localConfig.tts[cloudDialogEngine].api_key = tempApiKey;
  }

  async function checkCloudCredentials() {
    if (!cloudDialogEngine || !localConfig) return;
    isCheckingCreds = true;
    credCheckResult = null;
    engineTestResult = null;

    // Save the temp API key to localConfig and persist to backend
    saveApiKeyToConfig();

    try {
      // Temporarily persist the config so backend can read it
      await invoke("set_config", { newConfig: localConfig });

      if (cloudDialogEngine === "cartesia") {
        credCheckResult = {
          success: true,
          message: "API key saved. Use Test to validate synthesis."
        };
        return;
      }

      const command =
        cloudDialogEngine === "openai"
          ? "check_openai_credentials"
          : "check_elevenlabs_credentials";
      const result = await invoke<{ success: boolean; message: string }>(command);
      credCheckResult = result;
    } catch (e) {
      credCheckResult = { success: false, message: String(e) };
    } finally {
      isCheckingCreds = false;
    }
  }

  async function testCloudEngine() {
    if (!cloudDialogEngine || !localConfig) return;
    isTestingEngine = true;
    engineTestResult = null;

    // Save the temp API key to localConfig and persist to backend
    saveApiKeyToConfig();

    try {
      // Temporarily persist the config so backend can read it
      await invoke("set_config", { newConfig: localConfig });

      const result = await invoke<{ success: boolean; message: string }>("test_tts_engine_config", {
        engine: cloudDialogEngine,
        preset: null
      });
      engineTestResult = result;
    } catch (e) {
      engineTestResult = { success: false, message: String(e) };
    } finally {
      isTestingEngine = false;
    }
  }

  onMount(() => {
    loadConfig();
  });
</script>

<div class="w-full">
  {#if isLoading}
    <div class="flex min-h-[60vh] items-center justify-center">
      <div class="text-center">
        <div
          class="border-primary mx-auto mb-4 h-8 w-8 animate-spin rounded-full border-4 border-t-transparent"
        ></div>
        <p class="text-muted-foreground">{$_("engine.loading")}</p>
      </div>
    </div>
  {:else if localConfig}
    <!-- uv missing banner -->
    {#if uvAvailable === false}
      <div
        class="border-amber-500/30 bg-amber-500/10 mb-4 flex items-center justify-between gap-3 rounded-md border p-3"
      >
        <p class="text-amber-700 text-sm dark:text-amber-400">
          {$_("engine.setup.uvMissing")}
        </p>
        <Button variant="outline" size="sm" onclick={() => runInstaller("uv")}>
          <Download size={14} class="mr-2" />
          {$_("engine.setup.installUv")}
        </Button>
      </div>
    {/if}

    <div class="flex flex-row items-start gap-2">
      <!-- Tab sidebar -->
      <aside class="w-28 shrink-0 self-stretch">
        <nav class="sticky top-0 space-y-0.5">
          {#each ENGINE_TABS as tab}
            <button
              class="w-full rounded-md px-2 py-1.5 text-left text-sm transition-colors {activeTab ===
              tab.id
                ? "bg-primary/10 text-primary border-primary border-l-2 font-medium"
                : "text-muted-foreground hover:text-foreground hover:bg-muted/50"}"
              onclick={() => (activeTab = tab.id)}
            >
              {$_(`engine.${tab.id}.title`)}
            </button>
          {/each}
        </nav>
      </aside>

      <!-- Active engine panel -->
      <main class="flex-1 space-y-6 pb-20">
        <section class="border-border overflow-hidden rounded-lg border">
          <div class="bg-muted/50 border-border border-b p-4">
            <div class="flex flex-wrap items-center gap-2">
              <h2 class="text-lg font-semibold">{$_(`engine.${active.id}.title`)}</h2>
              {#each active.badges as badge}
                <span class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[badge]}">
                  {getBadgeLabel(badge)}
                </span>
              {/each}
            </div>
            <div class="p-4">
              {#if currentMeta.link}
                <div class="mb-4">
                  <button
                    onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                    class="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 text-xs transition-colors"
                  >
                    <ExternalLink size={12} />
                    {currentMeta.linkLabel}
                  </button>
                </div>
              {/if}
              <CartesiaEngine bind:localConfig />
            </div>
          </div>
        {:else if activeTab === "elevenlabs"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div class="flex items-center justify-between">
                <div>
                  <div class="flex flex-wrap items-center gap-2">
                    <h2 class="text-lg font-semibold">{$_("engine.elevenlabs.title")}</h2>
                    {#each currentMeta.badges as badge}
                      <span
                        class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[
                          badge
                        ]}"
                      >
                        {getBadgeLabel(badge)}
                      </span>
                    {/each}
                  </div>
                  <p class="text-muted-foreground mt-1 text-sm">
                    {$_("engine.elevenlabs.description")}
                  </p>
                </div>
                <Button variant="outline" size="sm" onclick={() => openCloudDialog("elevenlabs")}>
                  <Key size={14} class="mr-2" />
                  {$_("engine.elevenlabs.apiKey")}
                </Button>
              </div>
            </div>
            <div class="p-4">
              {#if currentMeta.link}
                <div class="mb-4">
                  <button
                    onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                    class="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 text-xs transition-colors"
                  >
                    <ExternalLink size={12} />
                    {currentMeta.linkLabel}
                  </button>
                </div>
              {/if}
              <ElevenLabsEngine bind:localConfig />
            </div>
          </div>
        {:else if activeTab === "openai"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div class="flex items-center justify-between">
                <div>
                  <div class="flex flex-wrap items-center gap-2">
                    <h2 class="text-lg font-semibold">{$_("engine.openai.title")}</h2>
                    {#each currentMeta.badges as badge}
                      <span
                        class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[
                          badge
                        ]}"
                      >
                        {getBadgeLabel(badge)}
                      </span>
                    {/each}
                  </div>
                  <p class="text-muted-foreground mt-1 text-sm">
                    {$_("engine.openai.description")}
                  </p>
                </div>
                <Button variant="outline" size="sm" onclick={() => openCloudDialog("openai")}>
                  <Key size={14} class="mr-2" />
                  {$_("engine.openai.apiKey")}
                </Button>
              </div>
            </div>
            <div class="p-4">
              {#if currentMeta.link}
                <div class="mb-4">
                  <button
                    onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                    class="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 text-xs transition-colors"
                  >
                    <ExternalLink size={12} />
                    {currentMeta.linkLabel}
                  </button>
                </div>
              {/if}
              <OpenAiEngine bind:localConfig />
            </div>
          </div>
        {:else if activeTab === "kitten"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div class="flex flex-wrap items-center gap-2">
                <h2 class="text-lg font-semibold">{$_("engine.kitten.title")}</h2>
                {#each currentMeta.badges as badge}
                  <span
                    class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[badge]}"
                  >
                    {getBadgeLabel(badge)}
                  </span>
                {/each}
              </div>
              <p class="text-muted-foreground mt-1 text-sm">{$_("engine.kitten.description")}</p>
            </div>
            <div class="p-4">
              {#if currentMeta.link}
                <div class="mb-4">
                  <button
                    onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                    class="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 text-xs transition-colors"
                  >
                    <ExternalLink size={12} />
                    {currentMeta.linkLabel}
                  </button>
                </div>
              {/if}
              <LocalEngine bind:localConfig />
            </div>
          </div>
        {:else if activeTab === "piper"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div class="flex flex-wrap items-center gap-2">
                <h2 class="text-lg font-semibold">{$_("engine.piper.title")}</h2>
                {#each currentMeta.badges as badge}
                  <span
                    class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[badge]}"
                  >
                    {getBadgeLabel(badge)}
                  </span>
                {/each}
              </div>
              <p class="text-muted-foreground mt-1 text-sm">{$_("engine.piper.description")}</p>
            </div>
            <div class="p-4">
              {#if currentMeta.link}
                <div class="mb-4">
                  <button
                    onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                    class="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 text-xs transition-colors"
                  >
                    <ExternalLink size={12} />
                    {currentMeta.linkLabel}
                  </button>
                </div>
              {/if}
              <LocalEngine bind:localConfig />
            </div>
          </div>
        {:else if activeTab === "kokoro"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div class="flex flex-wrap items-center gap-2">
                <h2 class="text-lg font-semibold">{$_("engine.kokoro.title")}</h2>
                {#each currentMeta.badges as badge}
                  <span
                    class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[badge]}"
                  >
                    {getBadgeLabel(badge)}
                  </span>
                {/each}
              </div>
              <p class="text-muted-foreground mt-1 text-sm">{$_("engine.kokoro.description")}</p>
            </div>
            <div class="p-4">
              {#if currentMeta.link}
                <div class="mb-4">
                  <button
                    onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                    class="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 text-xs transition-colors"
                  >
                    <ExternalLink size={12} />
                    {currentMeta.linkLabel}
                  </button>
                </div>
              {/if}
              <LocalEngine bind:localConfig />
            </div>
          </div>
        {:else if activeTab === "pocket"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div class="flex flex-wrap items-center gap-2">
                <h2 class="text-lg font-semibold">{$_("engine.pocket.title")}</h2>
                {#each currentMeta.badges as badge}
                  <span
                    class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[badge]}"
                  >
                    {getBadgeLabel(badge)}
                  </span>
                {/each}
              </div>
              <p class="text-muted-foreground mt-1 text-sm">{$_("engine.pocket.description")}</p>
            </div>
            <div class="p-4">
              {#if currentMeta.link}
                <div class="mb-4">
                  <button
                    onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                    class="text-muted-foreground hover:text-foreground flex cursor-pointer items-center gap-1 text-xs transition-colors"
                  >
                    <ExternalLink size={12} />
                    {currentMeta.linkLabel}
                  </button>
                </div>
              {/if}
              <LocalEngine bind:localConfig />
            </div>
          </div>
        {/if}
        <!-- Test Voice -->
        <div class="border-border hidden border-t pt-4">
          <Button variant="outline" size="sm" onclick={handleTestVoice} disabled={isTesting}>
            {isTesting ? $_("engine.testVoice.playing") : $_("engine.testVoice.title")}
          </Button>
          <span class="text-muted-foreground ml-3 text-xs">
            {$_("engine.testVoice.description")}
          </span>
        </div>
      </main>
    </div>

    <!-- Save Bar -->
    {#if hasChanges}
      <div
        class="border-border bg-card fixed right-4 bottom-12 z-60 flex items-center gap-3 border px-4 py-2.5 shadow-lg"
      >
        <Button size="sm" variant="ghost" onclick={cancelChanges} disabled={isSaving} class="h-8 px-3">
          {$_("engine.saveBar.cancel")}
        </Button>
        <Button size="sm" onclick={saveConfig} disabled={isSaving} class="h-8 px-4">
          {isSaving ? $_("engine.saveBar.saving") : $_("engine.saveBar.saveChanges")}
        </Button>
      </div>
    {/if}
  {:else}
    <div class="flex min-h-[60vh] items-center justify-center px-6">
      <div class="mx-auto w-full max-w-sm text-center">
        <h2 class="mb-2 text-xl font-semibold">{$_("engine.error.loadFailed")}</h2>
        <p class="text-muted-foreground mb-4">{$_("engine.error.loadDescription")}</p>
        <Button onclick={loadConfig}>{$_("engine.setup.tryAgain")}</Button>
      </div>
    </div>
  {/if}
</div>
