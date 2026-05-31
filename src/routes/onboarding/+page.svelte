<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { Label } from "$lib/components/ui/label/index.js";
  import type { AppConfig } from "$lib/types";
  import { CheckCircle, KeyRound, Loader2, Sparkles } from "@lucide/svelte";
  import { _ } from "svelte-i18n";

  let localConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  let isSaving = $state(false);
  let isCheckingKey = $state(false);
  let keyResult = $state<{
    success: boolean;
    message: string;
    error_type: string | null;
  } | null>(null);

  async function loadDefaultConfig() {
    isLoading = true;
    try {
      const config = await invoke<AppConfig>("get_config");
      config.tts.active_backend = "cartesia";
      config.pagination.fragment_size = 500;
      localConfig = config;
    } catch (e) {
      console.error("Failed to load config:", e);
      toast.error("Failed to load configuration");
    } finally {
      isLoading = false;
    }
  }

  function updateCartesiaApiKey(value: string) {
    if (!localConfig) return;
    localConfig.tts.cartesia.api_key = value.trim();
    keyResult = null;
  }

  async function checkCartesiaKey() {
    if (!localConfig) return;
    isCheckingKey = true;
    keyResult = null;
    try {
      await invoke("set_config", { newConfig: localConfig });
      const result = await invoke<{ success: boolean; message: string; error_type: string | null }>(
        "check_cartesia_credentials"
      );
      keyResult = result;
      if (result.success) toast.success(result.message);
      else toast.error(result.message);
    } catch (e) {
      keyResult = { success: false, message: String(e), error_type: "unavailable" };
      toast.error(`Failed to check Cartesia key: ${e}`);
    } finally {
      isCheckingKey = false;
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
                <Sparkles class="h-5 w-5" />
              </div>
              <div class="space-y-1">
                <h2 class="font-mono text-lg font-semibold">Start with Cartesia Cloud</h2>
                <p class="text-muted-foreground text-sm leading-relaxed">
                  CopySpeak TTS is set to Cartesia by default for fast, high-quality speech. Paste your
                  API key, verify it without spending synthesis credits, then start listening.
                </p>
              </div>
            </div>
          </div>

          <div class="grid gap-4 sm:grid-cols-[1fr_auto] sm:items-end">
            <div class="space-y-2">
              <Label for="cartesia-api-key">Cartesia API key</Label>
              <Input
                id="cartesia-api-key"
                type="password"
                placeholder="Paste your Cartesia API key"
                value={localConfig.tts.cartesia.api_key}
                oninput={(e) => updateCartesiaApiKey((e.target as HTMLInputElement).value)}
              />
            </div>
            <Button
              variant="outline"
              onclick={checkCartesiaKey}
              disabled={isCheckingKey || !localConfig.tts.cartesia.api_key.trim()}
              class="gap-2"
            >
              {#if isCheckingKey}
                <Loader2 class="h-4 w-4 animate-spin" />
                Checking
              {:else}
                <KeyRound class="h-4 w-4" />
                Verify key
              {/if}
            </Button>
          </div>

          {#if keyResult}
            <div
              class="flex items-center gap-2 rounded-md border px-4 py-3 {keyResult.success
                ? 'border-green-500/40 bg-green-500/10'
                : 'border-destructive/40 bg-destructive/10'}"
            >
              {#if keyResult.success}
                <CheckCircle class="h-4 w-4 text-green-600" />
              {:else}
                <div class="bg-destructive h-2.5 w-2.5 shrink-0 rounded-full"></div>
              {/if}
              <span
                class="text-sm {keyResult.success
                  ? 'text-green-700 dark:text-green-400'
                  : 'text-destructive'}"
              >
                {keyResult.message}
              </span>
            </div>
          {/if}
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
