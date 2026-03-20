<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { Button } from "$lib/components/ui/button/index.js";
  import LocalEngine from "$lib/components/engine/local-engine.svelte";
  import type { AppConfig } from "$lib/types";
  import { Download, Terminal, CheckCircle } from "@lucide/svelte";
  import { _ } from "svelte-i18n";

  let localConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  let isSaving = $state(false);
  let healthResult = $state<{
    success: boolean;
    message: string;
    error_type: string | null;
  } | null>(null);
  let isCheckingHealth = $state(false);
  let isInstalling = $state(false);
  let installComplete = $state(false);

  async function runHealthCheck() {
    isCheckingHealth = true;
    healthResult = null;
    try {
      const result = await invoke<{ success: boolean; message: string; error_type: string | null }>(
        "test_tts_engine"
      );
      healthResult = result;
    } catch (e) {
      healthResult = { success: false, message: String(e), error_type: "unavailable" };
    } finally {
      isCheckingHealth = false;
    }
  }

  async function loadDefaultConfig() {
    isLoading = true;
    try {
      const config = await invoke<AppConfig>("get_config");
      localConfig = config;
    } catch (e) {
      console.error("Failed to load config:", e);
      toast.error("Failed to load configuration");
    } finally {
      isLoading = false;
    }
  }

  async function runInstaller() {
    isInstalling = true;
    try {
      await invoke("run_kittentts_installer");
      toast.success("Kitten TTS installer launched in a new window");
      installComplete = true;
    } catch (e) {
      console.error("Failed to run installer:", e);
      toast.error(`Failed to run installer: ${e}`);
    } finally {
      isInstalling = false;
    }
  }

  async function skipOnboarding() {
    if (!localConfig) return;
    isSaving = true;
    try {
      await invoke("set_config", { newConfig: localConfig });
      toast.success("Welcome to CopySpeak!");
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

  onMount(async () => {
    await loadDefaultConfig();
    runHealthCheck();
  });
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
        <div class="border-border space-y-4 border-t border-b py-6">
          <div class="space-y-2">
            <h2 class="font-mono text-lg font-semibold">{$_("onboarding.engineConfig")}</h2>
            <p class="text-muted-foreground text-xs">
              {$_("onboarding.engineDescription")}
            </p>
          </div>

          <!-- Kitten TTS recommendation callout -->
          <div
            class="flex gap-3 rounded-md border border-emerald-500/30 bg-emerald-500/8 px-4 py-3"
          >
            <span class="mt-0.5 text-base leading-none">💡</span>
            <div class="space-y-1">
              <p class="text-sm font-medium text-emerald-700 dark:text-emerald-400">
                {$_("onboarding.recommendation.title")}
              </p>
              <p class="text-muted-foreground text-xs leading-relaxed">
                {$_("onboarding.recommendation.description")}
              </p>
            </div>
          </div>

          <!-- Install Button -->
          <div class="mt-4">
            <Button
              variant="outline"
              size="sm"
              onclick={runInstaller}
              disabled={isInstalling || installComplete}
              class="flex items-center gap-2"
            >
              {#if installComplete}
                <CheckCircle class="h-4 w-4 text-emerald-600" />
                <span class="text-emerald-700 dark:text-emerald-400"
                  >{$_("onboarding.installed")}</span
                >
              {:else if isInstalling}
                <Terminal class="h-4 w-4 animate-spin" />
                <span class="text-muted-foreground">{$_("onboarding.installing")}</span>
              {:else}
                <Download class="h-4 w-4" />
                <span class="text-muted-foreground">{$_("onboarding.openTerminal")}</span>
              {/if}
            </Button>
          </div>

          <LocalEngine bind:localConfig />
        </div>

        <!-- Health Check Status -->
        {#if isCheckingHealth}
          <div
            class="border-border bg-muted/30 flex items-center gap-2 rounded-md border px-4 py-3"
          >
            <div class="bg-muted h-2.5 w-2.5 animate-pulse rounded-full"></div>
            <span class="text-muted-foreground text-sm">{$_("onboarding.checkingEngine")}</span>
          </div>
        {:else if healthResult}
          <div
            class="flex items-center gap-2 rounded-md border px-4 py-3 {healthResult.success
              ? 'border-green-500/40 bg-green-500/10'
              : 'border-destructive/40 bg-destructive/10'}"
          >
            <div
              class="h-2.5 w-2.5 shrink-0 rounded-full {healthResult.success
                ? 'bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]'
                : 'bg-destructive shadow-[0_0_8px_rgba(239,68,68,0.6)]'}"
            ></div>
            <span
              class="text-sm {healthResult.success
                ? 'text-green-700 dark:text-green-400'
                : 'text-destructive'}"
            >
              {healthResult.message}
            </span>
          </div>
        {/if}

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
