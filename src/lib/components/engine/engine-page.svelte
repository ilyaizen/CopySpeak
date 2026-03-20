<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { Button } from "$lib/components/ui/button/index.js";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { ExternalLink, Key, CheckCircle, XCircle, Loader2 } from "@lucide/svelte";
  import LocalEngine from "./local-engine.svelte";
  import OpenAiEngine from "./openai-engine.svelte";
  import ElevenLabsEngine from "./elevenlabs-engine.svelte";
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
  let cloudDialogEngine = $state<"openai" | "elevenlabs" | null>(null);
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

  const TAB_PRESET_MAP: Record<string, string> = {
    kitten: "kitten-tts",
    piper: "piper",
    kokoro: "kokoro-tts",
    pocket: "pocket-tts"
  };

  const DEFAULT_VOICES: Record<string, string> = {
    kitten: "Jasper",
    piper: "en_US-joe-medium",
    kokoro: "af_heart",
    pocket: "alba",
    openai: "alloy",
    elevenlabs: "21m00Tcm4TlvDq8ikWAM"
  };

  type BadgeKind = "active" | "default" | "offline" | "free" | "cloud" | "paid" | "freemium";
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
    active: "bg-cyan-500/15 text-cyan-700 dark:text-cyan-400 ring-1 ring-cyan-500/30",
    default: "bg-emerald-500/15 text-emerald-700 dark:text-emerald-400 ring-1 ring-emerald-500/30",
    offline: "bg-blue-500/15 text-blue-700 dark:text-blue-400 ring-1 ring-blue-500/30",
    free: "bg-green-500/10 text-green-700 dark:text-green-400 ring-1 ring-green-500/25",
    cloud: "bg-violet-500/15 text-violet-700 dark:text-violet-400 ring-1 ring-violet-500/30",
    paid: "bg-amber-500/15 text-amber-700 dark:text-amber-400 ring-1 ring-amber-500/30",
    freemium: "bg-yellow-500/15 text-yellow-700 dark:text-yellow-400 ring-1 ring-yellow-500/30"
  };

  const getBadgeLabel = (badge: BadgeKind): string => {
    const labels: Record<BadgeKind, string> = {
      active: $_("engine.badges.active"),
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

  function isActiveEngine(tabId: string, config: AppConfig | null): boolean {
    if (!config) return false;
    if (tabId === "openai") return config.tts.active_backend === "openai";
    if (tabId === "elevenlabs") return config.tts.active_backend === "elevenlabs";
    const preset = TAB_PRESET_MAP[tabId];
    return config.tts.active_backend === "local" && config.tts.preset === preset;
  }

  function getMetaWithActive(tabId: string, config: AppConfig | null): EngineMeta {
    const cat = ENGINE_CATEGORIES.find((cat) => cat.id === tabId);
    if (!cat) return ENGINE_CATEGORIES[0].meta;
    const baseMeta = cat.meta;
    if (isActiveEngine(tabId, config)) {
      const badges: BadgeKind[] = ["active", ...baseMeta.badges.filter((b) => b !== "active")];
      return { ...baseMeta, badges };
    }
    return baseMeta;
  }

  const currentMeta = $derived(getMetaWithActive(activeTab, originalConfig));

  // Derived stores for translations that depend on reactive values
  const engineDescription = $derived(() => {
    if (!activeTab) return "";
    return $_(`engine.${activeTab}.description`);
  });

  const apiSetupTitle = $derived(() => {
    if (!cloudDialogEngine) return "";
    const engineName = cloudDialogEngine === "openai" ? "OpenAI" : "ElevenLabs";
    return $_("engine.apiSetup.title").replace("{engine}", engineName);
  });

  const apiSetupDescription = $derived(() => {
    if (!cloudDialogEngine) return "";
    const engineName = cloudDialogEngine === "openai" ? "OpenAI" : "ElevenLabs";
    return $_("engine.apiSetup.description").replace("{engine}", engineName);
  });

  function presetToTab(preset: string): string {
    if (preset === "kitten-tts") return "kitten";
    if (preset === "piper") return "piper";
    if (preset === "kokoro-tts") return "kokoro";
    if (preset === "pocket-tts") return "pocket";
    return "kitten";
  }

  function getTabFromConfig(config: AppConfig): string {
    if (config.tts.active_backend === "openai") return "openai";
    if (config.tts.active_backend === "elevenlabs") return "elevenlabs";
    return presetToTab(config.tts.preset ?? "piper");
  }

  const hasChanges = $derived(
    originalConfig !== null &&
      localConfig !== null &&
      JSON.stringify(localConfig) !== JSON.stringify(originalConfig)
  );

  async function loadConfig() {
    isLoading = true;
    try {
      const config = await invoke<AppConfig>("get_config");
      // Migrate stale HTTP backend to local (HTTP engine removed)
      if (config.tts.active_backend === ("http" as any)) {
        config.tts.active_backend = "local";
        toast.info("HTTP engine has been removed. Switched to Local engine.");
      }
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
      // Set active tab based on loaded config
      activeTab = getTabFromConfig(config);
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
    // Reset to the original active engine tab
    activeTab = getTabFromConfig(originalConfig);
  }

  function handleTabChange(newTab: string) {
    if (!localConfig) return;

    activeTab = newTab;

    if (newTab === "openai") {
      localConfig.tts.active_backend = "openai";
      if (!localConfig.tts.openai.voice) {
        localConfig.tts.openai.voice = DEFAULT_VOICES.openai;
      }
    } else if (newTab === "elevenlabs") {
      localConfig.tts.active_backend = "elevenlabs";
      if (!localConfig.tts.elevenlabs.voice_id) {
        localConfig.tts.elevenlabs.voice_id = DEFAULT_VOICES.elevenlabs;
      }
    } else {
      localConfig.tts.active_backend = "local";
      const preset = TAB_PRESET_MAP[newTab];
      localConfig.tts.preset = preset;
      const cfg = LOCAL_PRESET_CONFIGS[preset];
      if (cfg) {
        localConfig.tts.command = cfg.command;
        localConfig.tts.args_template = cfg.args;
      }
      localConfig.tts.voice = DEFAULT_VOICES[newTab];
    }
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
  function openCloudDialog(engine: "openai" | "elevenlabs") {
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

      const result = await invoke<{ success: boolean; message: string }>("test_tts_engine");
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
    <div class="flex flex-row gap-4">
      <!-- Left Sidebar Navigation -->
      <aside class="w-28 shrink-0">
        <nav class="sticky top-24 space-y-0.5">
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
        {#if activeTab === "elevenlabs"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div class="flex items-center justify-between">
                <div>
                  <h2 class="text-lg font-semibold">{$_("engine.elevenlabs.title")}</h2>
                  <p class="text-muted-foreground text-sm">{$_("engine.elevenlabs.description")}</p>
                </div>
                <Button variant="outline" size="sm" onclick={() => openCloudDialog("elevenlabs")}>
                  <Key size={14} class="mr-2" />
                  {$_("engine.elevenlabs.apiKey")}
                </Button>
              </div>
            </div>
            <div class="p-4">
              <div class="mb-4 space-y-3">
                <div class="flex items-start justify-between gap-3">
                  <div>
                    <h3 class="text-sm font-semibold">{$_("engine.info.engineInformation")}</h3>
                  </div>
                  {#if currentMeta.link}
                    <button
                      onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                      class="text-muted-foreground hover:text-foreground flex shrink-0 cursor-pointer items-center gap-1 text-xs transition-colors"
                    >
                      <ExternalLink size={12} />
                      {currentMeta.linkLabel}
                    </button>
                  {/if}
                </div>
                <div class="flex flex-wrap gap-1.5">
                  {#each currentMeta.badges as badge}
                    <span
                      class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[badge]}"
                    >
                      {getBadgeLabel(badge)}
                    </span>
                  {/each}
                </div>
                <p class="text-muted-foreground text-xs leading-relaxed">
                  {engineDescription()}
                </p>
              </div>
              <ElevenLabsEngine bind:localConfig />
            </div>
          </div>
        {:else if activeTab === "openai"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div class="flex items-center justify-between">
                <div>
                  <h2 class="text-lg font-semibold">{$_("engine.openai.title")}</h2>
                  <p class="text-muted-foreground text-sm">{$_("engine.openai.description")}</p>
                </div>
                <Button variant="outline" size="sm" onclick={() => openCloudDialog("openai")}>
                  <Key size={14} class="mr-2" />
                  {$_("engine.openai.apiKey")}
                </Button>
              </div>
            </div>
            <div class="p-4">
              <div class="mb-4 space-y-3">
                <div class="flex items-start justify-between gap-3">
                  <div>
                    <h3 class="text-sm font-semibold">{$_("engine.info.engineInformation")}</h3>
                  </div>
                  {#if currentMeta.link}
                    <button
                      onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                      class="text-muted-foreground hover:text-foreground flex shrink-0 cursor-pointer items-center gap-1 text-xs transition-colors"
                    >
                      <ExternalLink size={12} />
                      {currentMeta.linkLabel}
                    </button>
                  {/if}
                </div>
                <div class="flex flex-wrap gap-1.5">
                  {#each currentMeta.badges as badge}
                    <span
                      class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[badge]}"
                    >
                      {getBadgeLabel(badge)}
                    </span>
                  {/each}
                </div>
                <p class="text-muted-foreground text-xs leading-relaxed">
                  {engineDescription()}
                </p>
              </div>
              <OpenAiEngine bind:localConfig />
            </div>
          </div>
        {:else if activeTab === "kitten"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div>
                <h2 class="text-lg font-semibold">{$_("engine.kitten.title")}</h2>
                <p class="text-muted-foreground text-sm">{$_("engine.kitten.description")}</p>
              </div>
            </div>
            <div class="p-4">
              <div class="mb-4 space-y-3">
                <div class="flex items-start justify-between gap-3">
                  <div>
                    <h3 class="text-sm font-semibold">{$_("engine.info.engineInformation")}</h3>
                  </div>
                  {#if currentMeta.link}
                    <button
                      onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                      class="text-muted-foreground hover:text-foreground flex shrink-0 cursor-pointer items-center gap-1 text-xs transition-colors"
                    >
                      <ExternalLink size={12} />
                      {currentMeta.linkLabel}
                    </button>
                  {/if}
                </div>
                <div class="flex flex-wrap gap-1.5">
                  {#each currentMeta.badges as badge}
                    <span
                      class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[badge]}"
                    >
                      {getBadgeLabel(badge)}
                    </span>
                  {/each}
                </div>
                <p class="text-muted-foreground text-xs leading-relaxed">
                  {engineDescription()}
                </p>
              </div>
              <LocalEngine bind:localConfig />
            </div>
          </div>
        {:else if activeTab === "piper"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div>
                <h2 class="text-lg font-semibold">{$_("engine.piper.title")}</h2>
                <p class="text-muted-foreground text-sm">{$_("engine.piper.description")}</p>
              </div>
            </div>
            <div class="p-4">
              <div class="mb-4 space-y-3">
                <div class="flex items-start justify-between gap-3">
                  <div>
                    <h3 class="text-sm font-semibold">{$_("engine.info.engineInformation")}</h3>
                  </div>
                  {#if currentMeta.link}
                    <button
                      onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                      class="text-muted-foreground hover:text-foreground flex shrink-0 cursor-pointer items-center gap-1 text-xs transition-colors"
                    >
                      <ExternalLink size={12} />
                      {currentMeta.linkLabel}
                    </button>
                  {/if}
                </div>
                <div class="flex flex-wrap gap-1.5">
                  {#each currentMeta.badges as badge}
                    <span
                      class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[badge]}"
                    >
                      {getBadgeLabel(badge)}
                    </span>
                  {/each}
                </div>
                <p class="text-muted-foreground text-xs leading-relaxed">
                  {engineDescription()}
                </p>
              </div>
              <LocalEngine bind:localConfig />
            </div>
          </div>
        {:else if activeTab === "kokoro"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div>
                <h2 class="text-lg font-semibold">{$_("engine.kokoro.title")}</h2>
                <p class="text-muted-foreground text-sm">{$_("engine.kokoro.description")}</p>
              </div>
            </div>
            <div class="p-4">
              <div class="mb-4 space-y-3">
                <div class="flex items-start justify-between gap-3">
                  <div>
                    <h3 class="text-sm font-semibold">{$_("engine.info.engineInformation")}</h3>
                  </div>
                  {#if currentMeta.link}
                    <button
                      onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                      class="text-muted-foreground hover:text-foreground flex shrink-0 cursor-pointer items-center gap-1 text-xs transition-colors"
                    >
                      <ExternalLink size={12} />
                      {currentMeta.linkLabel}
                    </button>
                  {/if}
                </div>
                <div class="flex flex-wrap gap-1.5">
                  {#each currentMeta.badges as badge}
                    <span
                      class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[badge]}"
                    >
                      {getBadgeLabel(badge)}
                    </span>
                  {/each}
                </div>
                <p class="text-muted-foreground text-xs leading-relaxed">
                  {engineDescription()}
                </p>
              </div>
              <LocalEngine bind:localConfig />
            </div>
          </div>
        {:else if activeTab === "pocket"}
          <div class="border-border overflow-hidden rounded-lg border">
            <div class="bg-muted/50 border-border border-b p-4">
              <div>
                <h2 class="text-lg font-semibold">{$_("engine.pocket.title")}</h2>
                <p class="text-muted-foreground text-sm">{$_("engine.pocket.description")}</p>
              </div>
            </div>
            <div class="p-4">
              <div class="mb-4 space-y-3">
                <div class="flex items-start justify-between gap-3">
                  <div>
                    <h3 class="text-sm font-semibold">{$_("engine.info.engineInformation")}</h3>
                  </div>
                  {#if currentMeta.link}
                    <button
                      onclick={(e) => handleExternalLinkClick(e, currentMeta.link!)}
                      class="text-muted-foreground hover:text-foreground flex shrink-0 cursor-pointer items-center gap-1 text-xs transition-colors"
                    >
                      <ExternalLink size={12} />
                      {currentMeta.linkLabel}
                    </button>
                  {/if}
                </div>
                <div class="flex flex-wrap gap-1.5">
                  {#each currentMeta.badges as badge}
                    <span
                      class="rounded-full px-2 py-0.5 text-[11px] font-medium {BADGE_STYLES[badge]}"
                    >
                      {getBadgeLabel(badge)}
                    </span>
                  {/each}
                </div>
                <p class="text-muted-foreground text-xs leading-relaxed">
                  {engineDescription()}
                </p>
              </div>
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
              : $_("engine.apiSetup.placeholderElevenlabs")}
            bind:value={tempApiKey}
          />
          <p class="text-muted-foreground text-xs">
            {cloudDialogEngine === "openai"
              ? $_("engine.openai.apiKeyDescription")
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
              <Loader2 class="mr-2 h-4 w-4 animate-spin" />
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
              <Loader2 class="mr-2 h-4 w-4 animate-spin" />
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
