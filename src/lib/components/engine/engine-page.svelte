<script lang="ts">
  import { onMount } from "svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
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

  // ── Engine tab registry ───────────────────────────────────────────────────
  // The single source of truth for what the Engine page renders. Profiles own
  // voice/model/knobs; this page owns credentials, install, and setup tests only.

  type BadgeKind = "default" | "offline" | "free" | "cloud" | "paid" | "freemium";
  type Location = "local" | "cloud";
  type CredentialKind = "none" | "api_key" | "api_key_endpoint";
  type CredentialTarget = "openai" | "elevenlabs" | "cartesia" | "google" | "microsoft";

  interface EngineTab {
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

  // ── State ────────────────────────────────────────────────────────────────

  let localConfig = $state<AppConfig | null>(null);
  let originalConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  let isSaving = $state(false);
  let activeTab = $state<string>("edge");

  let testingFor = $state<string | null>(null);
  let testResult = $state<{ engine: string; success: boolean; message: string } | null>(null);

  let installingFor = $state<string | null>(null);
  let uvAvailable = $state<boolean | null>(null);

  const active = $derived(ENGINE_TABS.find((t) => t.id === activeTab) ?? ENGINE_TABS[0]);

  const hasChanges = $derived(
    originalConfig !== null &&
      localConfig !== null &&
      JSON.stringify(localConfig) !== JSON.stringify(originalConfig)
  );

  // ── Config load / save ────────────────────────────────────────────────────

  async function loadConfig() {
    isLoading = true;
    try {
      const config = await invoke<AppConfig>("get_config");
      localConfig = JSON.parse(JSON.stringify(config));
      originalConfig = JSON.parse(JSON.stringify(config));
      void checkUv();
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
      // Persist so the backend reads the just-typed credential.
      await invoke("set_config", { newConfig: localConfig });
      const result = await invoke<{ success: boolean; message: string }>(
        "test_tts_engine_config",
        { engine: tab.testEngine, preset: null }
      );
      testResult = { engine: tab.id, ...result };
    } catch (e) {
      testResult = { engine: tab.id, success: false, message: String(e) };
    } finally {
      testingFor = null;
    }
  }

  // ── Installer launch ───────────────────────────────────────────────────────

  async function checkUv() {
    try {
      const r = await invoke<{ available: boolean }>("check_command_exists", { command: "uv" });
      uvAvailable = r.available;
    } catch {
      uvAvailable = false;
    }
  }

  async function runInstaller(installer: string) {
    if (installer !== "uv" && uvAvailable === false) {
      toast.error("uv is not installed. Install uv first.");
      return;
    }
    installingFor = installer;
    try {
      await invoke("install_engine", { engine: installer });
      toast.success(`Installer launched in a new window. Follow the prompts there.`);
    } catch (e) {
      toast.error(`Failed to launch installer: ${e}`);
    } finally {
      // The window runs detached; clear after a beat so the spinner is visible.
      setTimeout(() => (installingFor = null), 1200);
    }
  }

  function handleExternalLinkClick(e: Event, url: string) {
    e.preventDefault();
    openExternal(url);
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
            <p class="text-muted-foreground mt-1 text-sm">
              {$_(`engine.${active.id}.description`)}
            </p>
            <button
              onclick={(e) => handleExternalLinkClick(e, active.docsUrl)}
              class="text-muted-foreground hover:text-foreground mt-2 inline-flex items-center gap-1 text-xs transition-colors"
            >
              <ExternalLink size={12} />
              {$_("engine.setup.docs")}
            </button>
          </div>

          <div class="space-y-4 p-4">
            <!-- Credentials -->
            {#if active.credential === "api_key" && active.credentialTarget}
              <div class="space-y-2">
                <Label for="api-key">{$_("engine.apiSetup.apiKey")}</Label>
                <Input
                  id="api-key"
                  type="password"
                  placeholder={$_(`engine.${active.id}.apiKeyPlaceholder`)}
                  value={apiKey(active.credentialTarget)}
                  onchange={(e) => setApiKey(active, (e.target as HTMLInputElement).value)}
                />
                <p class="text-muted-foreground text-xs">{$_(`engine.${active.id}.apiKeyDescription`)}</p>
              </div>
            {:else if active.credential === "api_key_endpoint"}
              <div class="space-y-3">
                <div class="space-y-2">
                  <Label for="endpoint">{$_("engine.microsoft.endpoint")}</Label>
                  <Input
                    id="endpoint"
                    placeholder="https://<deployment>.azure..."
                    value={endpoint()}
                    onchange={(e) => setEndpoint((e.target as HTMLInputElement).value)}
                  />
                  <p class="text-muted-foreground text-xs">{$_("engine.microsoft.endpointDescription")}</p>
                </div>
                <div class="space-y-2">
                  <Label for="api-key">{$_("engine.apiSetup.apiKey")}</Label>
                  <Input
                    id="api-key"
                    type="password"
                    placeholder={$_(`engine.${active.id}.apiKeyPlaceholder`)}
                    value={apiKey("microsoft")}
                    onchange={(e) => setApiKey(active, (e.target as HTMLInputElement).value)}
                  />
                  <p class="text-muted-foreground text-xs">{$_("engine.microsoft.apiKeyDescription")}</p>
                </div>
              </div>
            {/if}

            <!-- Setup test result -->
            {#if testResult && testResult.engine === active.id}
              <div
                class="rounded-lg border p-3 {testResult.success
                  ? "border-emerald-500/30 bg-emerald-500/10"
                  : "border-destructive/30 bg-destructive/10"}"
              >
                <div class="flex items-start gap-2">
                  {#if testResult.success}
                    <CheckCircle class="mt-0.5 h-4 w-4 text-emerald-600" />
                  {:else}
                    <XCircle class="text-destructive mt-0.5 h-4 w-4" />
                  {/if}
                  <div>
                    <p
                      class="text-sm font-medium {testResult.success
                        ? "text-emerald-700"
                        : "text-destructive"}"
                    >
                      {testResult.success
                        ? $_("engine.apiSetup.testPassed")
                        : $_("engine.apiSetup.testFailed")}
                    </p>
                    <p class="text-muted-foreground text-xs">{testResult.message}</p>
                  </div>
                </div>
              </div>
            {/if}

            <!-- HTTP note -->
            {#if active.id === "http"}
              <p class="text-muted-foreground text-sm">{$_("engine.http.note")}</p>
            {/if}

            <!-- Actions -->
            <div class="flex flex-wrap gap-2">
              {#if active.testEngine}
                <Button
                  variant="outline"
                  size="sm"
                  onclick={() => runSetupTest(active)}
                  disabled={testingFor === active.id}
                >
                  {#if testingFor === active.id}
                    <Spinner class="mr-2 h-4 w-4" />
                  {:else}
                    <Key size={14} class="mr-2" />
                  {/if}
                  {$_("engine.setup.testSetup")}
                </Button>
              {/if}
              {#if active.installer}
                <Button
                  variant="outline"
                  size="sm"
                  onclick={() => active.installer && runInstaller(active.installer)}
                  disabled={installingFor === active.installer || (active.installer !== "uv" && uvAvailable === false)}
                >
                  {#if installingFor === active.installer}
                    <Loader2 size={14} class="mr-2 animate-spin" />
                  {:else}
                    <Terminal size={14} class="mr-2" />
                  {/if}
                  {$_("engine.setup.install")}
                </Button>
              {/if}
            </div>
          </div>
        </section>

        <p class="text-muted-foreground text-xs">
          {$_("engine.setup.profilesHint")}
        </p>
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
