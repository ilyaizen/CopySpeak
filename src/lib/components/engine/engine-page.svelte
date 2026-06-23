<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
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
    Dialog,
    DialogContent,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogTitle
  } from "$lib/components/ui/dialog/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import { _ } from "svelte-i18n";

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
  type LocationKind = "local" | "cloud";

  interface EngineMeta {
    badges: BadgeKind[];
    location: LocationKind;
    link: string | null;
    linkLabel: string | null;
  }

  interface EngineCategory {
    id: string;
    meta: EngineMeta;
  }

  const BADGE_STYLES: Record<BadgeKind, string> = {
    default: "bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 ring-1 ring-emerald-500/30",
    offline: "bg-blue-500/15 text-blue-700 dark:text-blue-400 ring-1 ring-blue-500/30",
    free: "bg-green-500/10 text-green-700 dark:text-green-400 ring-1 ring-green-500/25",
    cloud: "bg-violet-500/15 text-violet-700 dark:text-violet-400 ring-1 ring-violet-500/30",
    paid: "bg-amber-500/15 text-amber-700 dark:text-amber-400 ring-1 ring-amber-500/30",
    freemium: "bg-yellow-500/15 text-yellow-700 dark:text-yellow-400 ring-1 ring-yellow-500/30"
  };

  const getBadgeLabel = (badge: BadgeKind): string => {
    const labels: Record<BadgeKind, string> = {
      default: $_("engine.badges.default"),
      offline: $_("engine.badges.offline"),
      free: $_("engine.badges.free"),
      cloud: $_("engine.badges.cloud"),
      paid: $_("engine.badges.paid"),
      freemium: $_("engine.badges.freemium")
    };
    return labels[badge];
  };

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

  function getMeta(tabId: string): EngineMeta {
    return ENGINE_CATEGORIES.find((cat) => cat.id === tabId)?.meta ?? ENGINE_CATEGORIES[0].meta;
  }

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
      // Migrate qwen3-tts and custom presets to piper
      if (config.tts.preset === "qwen3-tts" || config.tts.preset === "custom") {
        config.tts.preset = "piper";
        const piperCfg = LOCAL_PRESET_CONFIGS.piper;
        config.tts.command = piperCfg.command;
        config.tts.args_template = piperCfg.args;
      }
      // Migrate stale pocket-tts args (-o → --output-path, {input} → {raw_text})
      if (config.tts.preset === "pocket-tts" && config.tts.args_template?.includes("-o")) {
        const pocketCfg = LOCAL_PRESET_CONFIGS["pocket-tts"];
        config.tts.args_template = pocketCfg.args;
      }
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
      toast.success("Engine saved successfully");
      // Navigate to Play page after successful save
      goto("/");
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
    activeTab ||= "cartesia";
  }

  function handleTabChange(newTab: string) {
    activeTab = newTab;
  }

  async function handleTestVoice() {
    isTesting = true;
    try {
      await invoke("speak_now", {
        text: "Hello from CopySpeak. This is a voice test."
      });
    } catch (e) {
      toast.error(`Voice test failed: ${e}`);
    } finally {
      isTesting = false;
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
    <div class="flex flex-row items-start gap-2">
      <!-- Left Sidebar Navigation -->
      <aside class="w-28 shrink-0 self-stretch">
        <nav class="sticky top-0 space-y-0.5">
          {#each ENGINE_CATEGORIES as category}
            <button
              class="w-full rounded-md px-2 py-1.5 text-left text-sm transition-colors {activeTab ===
              category.id
                ? 'bg-primary/10 text-primary border-primary border-l-2 font-medium'
                : 'text-muted-foreground hover:text-foreground hover:bg-muted/50'}"
              onclick={() => handleTabChange(category.id)}
            >
              {$_(`engine.${category.id}.title`)}
            </button>
          {/each}
        </nav>
      </aside>

      <!-- Main Content -->
      <main class="flex-1 space-y-6 pb-20">
        <!-- Engine Configuration -->
        {#if activeTab === "cartesia"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div class="flex items-center justify-between">
                <div>
                  <div class="flex flex-wrap items-center gap-2">
                    <h2 class="text-lg font-semibold">{$_("engine.cartesia.title")}</h2>
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
                    {$_("engine.cartesia.description")}
                  </p>
                </div>
                <Button variant="outline" size="sm" onclick={() => openCloudDialog("cartesia")}>
                  <Key size={14} class="mr-2" />
                  {$_("engine.cartesia.apiKey")}
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
        <Button
          size="sm"
          variant="ghost"
          onclick={cancelChanges}
          disabled={isSaving}
          class="h-8 px-3"
        >
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
        <p class="text-muted-foreground mb-4">
          {$_("engine.error.loadDescription")}
        </p>
        <Button onclick={loadConfig}>{$_("settings.actions.tryAgain")}</Button>
      </div>
    </div>
  {/if}

  <!-- Cloud TTS API Key Dialog -->
  <Dialog bind:open={cloudDialogOpen}>
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>
          {apiSetupTitle()}
        </DialogTitle>
        <DialogDescription>
          {apiSetupDescription()}
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-4 py-4">
        <!-- API Key Input -->
        <div class="space-y-2">
          <Label for="cloud-api-key">{$_("engine.apiSetup.apiKey")}</Label>
          <Input
            id="cloud-api-key"
            type="password"
            placeholder={cloudDialogEngine === "openai"
              ? $_("engine.apiSetup.placeholderOpenai")
              : cloudDialogEngine === "cartesia"
                ? $_("engine.apiSetup.placeholderCartesia")
                : $_("engine.apiSetup.placeholderElevenlabs")}
            bind:value={tempApiKey}
          />
          <p class="text-muted-foreground text-xs">
            {cloudDialogEngine === "openai"
              ? $_("engine.openai.apiKeyDescription")
              : cloudDialogEngine === "cartesia"
                ? $_("engine.cartesia.apiKeyDescription")
                : $_("engine.elevenlabs.apiKeyDescription")}
          </p>
        </div>

        <!-- Test Results -->
        {#if credCheckResult}
          <div
            class="rounded-lg border p-3 {credCheckResult.success
              ? 'border-emerald-500/30 bg-emerald-500/10'
              : 'border-destructive/30 bg-destructive/10'}"
          >
            <div class="flex items-start gap-2">
              {#if credCheckResult.success}
                <CheckCircle class="mt-0.5 h-4 w-4 text-emerald-600" />
              {:else}
                <XCircle class="text-destructive mt-0.5 h-4 w-4" />
              {/if}
              <div>
                <p
                  class="text-sm font-medium {credCheckResult.success
                    ? 'text-emerald-700'
                    : 'text-destructive'}"
                >
                  {credCheckResult.success
                    ? $_("engine.apiSetup.valid")
                    : $_("engine.apiSetup.invalid")}
                </p>
                <p class="text-muted-foreground text-xs">{credCheckResult.message}</p>
              </div>
            </div>
          </div>
        {/if}

        {#if engineTestResult}
          <div
            class="rounded-lg border p-3 {engineTestResult.success
              ? 'border-emerald-500/30 bg-emerald-500/10'
              : 'border-destructive/30 bg-destructive/10'}"
          >
            <div class="flex items-start gap-2">
              {#if engineTestResult.success}
                <CheckCircle class="mt-0.5 h-4 w-4 text-emerald-600" />
              {:else}
                <XCircle class="text-destructive mt-0.5 h-4 w-4" />
              {/if}
              <div>
                <p
                  class="text-sm font-medium {engineTestResult.success
                    ? 'text-emerald-700'
                    : 'text-destructive'}"
                >
                  {engineTestResult.success
                    ? $_("engine.apiSetup.testPassed")
                    : $_("engine.apiSetup.testFailed")}
                </p>
                <p class="text-muted-foreground text-xs">{engineTestResult.message}</p>
              </div>
            </div>
          </div>
        {/if}
      </div>

      <DialogFooter class="flex-col gap-2 sm:flex-row">
        <div class="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            onclick={checkCloudCredentials}
            disabled={isCheckingCreds || isTestingEngine || !tempApiKey}
          >
            {#if isCheckingCreds}
              <Spinner class="mr-2 h-4 w-4" />
            {/if}
            {$_("engine.apiSetup.checkButton")}
          </Button>
          <Button
            variant="outline"
            size="sm"
            onclick={testCloudEngine}
            disabled={isCheckingCreds || isTestingEngine || !tempApiKey}
          >
            {#if isTestingEngine}
              <Spinner class="mr-2 h-4 w-4" />
            {/if}
            {$_("engine.apiSetup.testButton")}
          </Button>
        </div>
        <div class="flex gap-2 sm:ml-auto">
          <Button variant="ghost" size="sm" onclick={() => closeCloudDialog()}>
            {$_("engine.apiSetup.cancel")}
          </Button>
          <Button
            size="sm"
            onclick={() => {
              saveApiKeyToConfig();
              closeCloudDialog(false);
            }}
          >
            {$_("engine.apiSetup.save")}
          </Button>
        </div>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</div>
