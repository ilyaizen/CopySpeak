<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { Button } from "$lib/components/ui/button/index.js";
  import { _ } from "svelte-i18n";
  import { Sparkles, Volume2, Zap, KeyRound } from "@lucide/svelte";

  // Issue #43 structural fix: onboarding never calls the full `get_config`
  // (its invoke() promise hangs on fresh installs). Everything here runs
  // through tiny dedicated commands: get_onboarding_status / complete_onboarding.

  type OnboardingStatus = {
    has_config: boolean;
    engine: string;
    engine_name: string;
    voice: string;
    keyless: boolean;
  };

  type EngineChoice = {
    id: string;
    name: string;
    tagline: string;
    detail: string;
    recommended: boolean;
    keyless: boolean;
  };

  const engineChoices: EngineChoice[] = [
    {
      id: "edge",
      name: "Edge-TTS",
      tagline: "Recommended — works instantly",
      detail:
        "Free Microsoft Read Aloud voices. No API key, no downloads — start listening right away.",
      recommended: true,
      keyless: true
    },
    {
      id: "cartesia",
      name: "Cartesia",
      tagline: "Premium cloud voices",
      detail: "Fast, high-quality voices. Requires an API key — you can add it later in Settings.",
      recommended: false,
      keyless: false
    }
  ];

  let selectedEngine = $state("edge");
  let status = $state<OnboardingStatus | null>(null);
  let isLoading = $state(true);
  let isSaving = $state(false);

  onMount(async () => {
    try {
      status = await invoke<OnboardingStatus>("get_onboarding_status");
      if (status.has_config) selectedEngine = status.engine;
    } catch (e) {
      // Even if this fails, defaults are safe: Edge is the fresh-install
      // default in Rust, so completing with "edge" is always valid.
      console.error("Failed to load onboarding status:", e);
      status = {
        has_config: false,
        engine: "edge",
        engine_name: "Edge-TTS",
        voice: "en-US-AvaMultilingualNeural",
        keyless: true
      };
    } finally {
      isLoading = false;
    }
  });

  async function finish(engineId: string) {
    isSaving = true;
    try {
      await invoke("complete_onboarding", { engine: engineId });
      toast.success("Welcome to CopySpeak!");
      await goto("/");
    } catch (e) {
      console.error("Failed to complete onboarding:", e);
      toast.error(`Failed to save settings: ${e}`);
    } finally {
      isSaving = false;
    }
  }
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

      {#if isLoading}
        <div class="flex min-h-50 items-center justify-center">
          <div class="text-muted-foreground">{$_("onboarding.loading")}</div>
        </div>
      {:else}
        <!-- Engine choice -->
        <div class="border-border space-y-3 border-y py-6">
          <div class="flex items-start gap-3">
            <div class="bg-primary/10 text-primary rounded-sm p-2">
              <Volume2 class="h-5 w-5" />
            </div>
            <div class="space-y-1">
              <h2 class="text-lg font-semibold">Choose your voice engine</h2>
              <p class="text-muted-foreground text-sm leading-relaxed">
                You can switch engines, voices, and speed anytime in Settings.
              </p>
            </div>
          </div>

          <div class="mt-4 grid gap-3">
            {#each engineChoices as choice (choice.id)}
              <button
                type="button"
                onclick={() => (selectedEngine = choice.id)}
                class={selectedEngine === choice.id
                  ? "border-primary ring-primary bg-primary/5 rounded-lg border p-4 text-left transition-all focus-visible:ring-2 focus-visible:outline-none"
                  : "border-border hover:border-primary/60 rounded-lg border p-4 text-left transition-all focus-visible:ring-2 focus-visible:outline-none"}
              >
                <div class="flex items-center justify-between gap-3">
                  <div class="flex items-center gap-2">
                    <span class="font-medium">{choice.name}</span>
                    {#if choice.recommended}
                      <span
                        class="bg-primary/10 text-primary inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium"
                      >
                        <Sparkles class="h-3 w-3" />
                        {choice.tagline}
                      </span>
                    {/if}
                  </div>
                  {#if choice.keyless}
                    <span class="text-muted-foreground inline-flex items-center gap-1 text-xs">
                      <Zap class="h-3 w-3" />
                      No key needed
                    </span>
                  {:else}
                    <span class="text-muted-foreground inline-flex items-center gap-1 text-xs">
                      <KeyRound class="h-3 w-3" />
                      API key in Settings
                    </span>
                  {/if}
                </div>
                <p class="text-muted-foreground mt-1.5 text-sm leading-relaxed">
                  {choice.detail}
                </p>
              </button>
            {/each}
          </div>

          <p class="text-muted-foreground mt-2 text-xs">
            Local engines (Kitten, Kokoro, Qwen, Piper) can be installed from the Engines page after
            setup.
          </p>
        </div>

        <!-- Action Button -->
        <div class="flex flex-col gap-3 pt-2">
          <Button
            size="lg"
            onclick={() => finish(selectedEngine)}
            disabled={isSaving}
            class="w-full"
          >
            {#if isSaving}
              {$_("common.saving")}
            {:else if selectedEngine === "edge"}
              {$_("onboarding.complete")}
            {:else}
              {$_("onboarding.complete")} — {engineChoices.find((c) => c.id === selectedEngine)
                ?.name}
            {/if}
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
