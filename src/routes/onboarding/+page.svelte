<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";

  import type { AppConfig } from "$lib/types";
  import { Volume2 } from "@lucide/svelte";
  import { _ } from "svelte-i18n";

  let localConfig = $state<AppConfig | null>(null);
  let isLoading = $state(true);
  let isSaving = $state(false);
  let testing = $state(false);

  async function loadDefaultConfig() {
    isLoading = true;
    try {
      const config = await invoke<AppConfig>("get_config");
      const cartesiaProfile = config.tts.profiles.find((profile) => profile.engine === "cartesia");
      config.tts.active_backend = "cartesia";
      if (cartesiaProfile) config.tts.active_profile_id = cartesiaProfile.id;
      config.pagination.fragment_size = 500;
      localConfig = config;
    } catch (e) {
      console.error("Failed to load config:", e);
      toast.error("Failed to load configuration");
    } finally {
      isLoading = false;
    }
  }

  async function testCartesia() {
    if (!localConfig) return;
    testing = true;
    try {
      await invoke("set_config", { newConfig: localConfig });
      const result = await invoke<{ success: boolean; message: string }>(
        "check_cartesia_credentials"
      );
      if (result.success) toast.success(result.message || "Cartesia is ready.");
      else toast.error(result.message || "Cartesia API key check failed.");
    } catch (e) {
      toast.error(`Cartesia API key check failed: ${e}`);
    } finally {
      testing = false;
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

  onMount(loadDefaultConfig);
</script>

<div class="bg-background flex min-h-screen items-center justify-center p-4 sm:p-8">
  <div class="w-full max-w-2xl">
    <div class="space-y-6">
      <!-- Header -->
      <div class="space-y-2">
        <h1 class="text-3xl font-bold tracking-tight sm:text-4xl">
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
        <div class="border-border space-y-5 border-y py-6">
          <div class="p-1">
            <div class="flex items-start gap-3">
              <div class="bg-primary/10 text-primary rounded-sm p-2">
                <Volume2 class="h-5 w-5" />
              </div>
              <div class="space-y-1">
                <h2 class="text-lg font-semibold">Set up Cartesia</h2>
                <p class="text-muted-foreground text-sm leading-relaxed">
                  CopySpeak is set to Cartesia by default for fast, high-quality speech. Paste your
                  API key, verify it without spending synthesis credits, then start listening.
                </p>
              </div>
            </div>
            <label for="cartesia-api-key" class="mt-5 block text-sm font-medium">
              Cartesia API key
            </label>
            <Input
              id="cartesia-api-key"
              type="password"
              bind:value={localConfig.tts.cartesia.api_key}
              placeholder="sk_car_…"
              class="mt-2"
            />
          </div>
        </div>

        <!-- Test -->
        <div class="flex flex-col gap-3 pt-2 sm:flex-row">
          <Button
            variant="outline"
            size="lg"
            onclick={testCartesia}
            disabled={testing || !localConfig.tts.cartesia.api_key.trim()}
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
