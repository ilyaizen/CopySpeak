<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { Button } from "$lib/components/ui/button/index.js";

  import type { AppConfig } from "$lib/types";
  import { Volume2 } from "@lucide/svelte";
  import { _ } from "svelte-i18n";

  let localConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  let isSaving = $state(false);
  let testing = $state(false);
  let installing = $state(false);


  async function loadDefaultConfig() {
    isLoading = true;
    try {
      const config = await invoke<AppConfig>("get_config");
      config.tts.active_backend = "edge";
      config.tts.edge.voice = "en-US-AvaMultilingualNeural";
      config.pagination.fragment_size = 500;
      localConfig = config;
    } catch (e) {
      console.error("Failed to load config:", e);
      toast.error("Failed to load configuration");
    } finally {
      isLoading = false;
    }
  }



  async function testEdgeTts() {
    testing = true;
    try {
      const result = await invoke<{ success: boolean; message: string }>(
        "test_tts_engine_config",
        { engine: "edge", preset: null }
      );
      if (result.success) toast.success(result.message || "Edge-TTS is working.");
      else toast.error(result.message || "Edge-TTS test failed.");
    } catch (e) {
      toast.error(`Edge-TTS test failed: ${e}`);
    } finally {
      testing = false;
    }
  }

  async function installEdgeTts() {
    installing = true;
    try {
      await invoke("install_engine", { engine: "edge" });
      toast.success("Installer launched in a new window. Follow the prompts there.");
    } catch (e) {
      toast.error(`Failed to launch installer: ${e}`);
    } finally {
      setTimeout(() => (installing = false), 1200);
    }
  }

  async function skipOnboarding() {
    if (!localConfig) return;
    isSaving = true;
    try {
      await invoke("set_config", { newConfig: localConfig });
      toast.success("Welcome to CopySpeak TTS!");
      await goto("/");
    } catch (e) {
      console.error("Failed to save config:", e);
      toast.error(`Failed to save settings: ${e}`);
    } finally {
      isSaving = false;
    }
  }

  async function completeOnboarding() {
    if (!localConfig) return;
    isSaving = true;
    try {
      await invoke("set_config", { newConfig: localConfig });
      toast.success("Configuration saved! Let's get started.");
      await goto("/");
    } catch (e) {
      console.error("Failed to save config:", e);
      toast.error(`Failed to save settings: ${e}`);
    } finally {
      isSaving = false;
    }
  }

  onMount(loadDefaultConfig);
</script>

<div
  class="from-background to-muted/30 flex min-h-screen items-center justify-center bg-linear-to-br p-4 sm:p-6"
>
  <div class="w-full max-w-2xl">
    <!-- Main Card -->
    <div class="border-border bg-card space-y-6 rounded-lg border p-6 shadow-lg sm:p-8">
      <!-- Header -->
      <div class="space-y-2 text-center">
        <h1
          class="from-foreground to-foreground/70 bg-linear-to-r bg-clip-text font-mono text-3xl font-bold tracking-tight sm:text-4xl"
        >
          {$_("onboarding.welcome.title")}
        </h1>
        <p class="text-muted-foreground text-sm sm:text-base">
          {$_("onboarding.welcome.subtitle")}
        </p>
      </div>

      <!-- Configuration Section -->
      {#if isLoading}
        <div class="flex min-h-50 items-center justify-center">
          <div class="text-muted-foreground">{$_("onboarding.loading")}</div>
        </div>
      {:else if localConfig}
        <div class="border-border space-y-5 border-t border-b py-6">
          <div class="rounded-lg border border-sky-500/30 bg-sky-500/8 p-5">
            <div class="flex items-start gap-3">
              <div class="rounded-md bg-sky-500/15 p-2 text-sky-700 dark:text-sky-300">
                <Volume2 class="h-5 w-5" />
              </div>
              <div class="space-y-1">
                <h2 class="font-mono text-lg font-semibold">Ready to go with Edge-TTS</h2>
                <p class="text-muted-foreground text-sm leading-relaxed">
                  CopySpeak TTS is set to Cartesia by default for fast, high-quality speech. Paste your
                  API key, verify it without spending synthesis credits, then start listening.
                </p>
              </div>
            </div>
          </div>
        </div>

        <!-- Install / Test -->
        <div class="flex flex-col gap-3 pt-2 sm:flex-row">
          <Button
            variant="outline"
            size="lg"
            onclick={installEdgeTts}
            disabled={installing}
            class="flex-1"
          >
            {installing ? "Launching…" : "Install edge-tts"}
          </Button>
          <Button
            variant="outline"
            size="lg"
            onclick={testEdgeTts}
            disabled={testing}
            class="flex-1"
          >
            {testing ? "Testing…" : "Test"}
          </Button>
        </div>

        <!-- Action Buttons -->
        <div class="flex flex-col gap-3 pt-2 sm:flex-row">
          <Button
            variant="outline"
            size="lg"
            onclick={skipOnboarding}
            disabled={isSaving}
            class="flex-1"
          >
            {$_("onboarding.skip")}
          </Button>
          <Button size="lg" onclick={completeOnboarding} disabled={isSaving} class="flex-1">
            {isSaving ? $_("common.saving") : $_("onboarding.complete")}
          </Button>
        </div>

        <!-- Help Text -->
        <p class="text-muted-foreground text-center text-xs">
          {$_("onboarding.helpText")}
        </p>
      {/if}
    </div>
  </div>
</div>
