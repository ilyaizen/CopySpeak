<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { Button } from "#lib/components/ui/button/index.js";
  import { _ } from "svelte-i18n";
  import {
    Sparkles,
    Volume2,
    Zap,
    KeyRound,
    Download,
    Loader2,
    Check,
    AlertCircle,
    RotateCw
  } from "@lucide/svelte";
  import { installStore } from "#lib/stores/install-store.svelte";
  import { ENGINE_CHOICES } from "#lib/onboarding/engine-choices.js";

  // Issue #43 structural fix: onboarding never calls the full `get_config`
  // (its invoke() promise hangs on fresh installs). Everything here runs
  // through tiny dedicated commands: get_onboarding_status / complete_onboarding.
  //
  // Kokoro is the fresh-install default (schema v6): keyless offline voices.
  // The one-time ~335 MB install streams right here via the same
  // `install-progress` pipeline the Engines page uses (installStore is the
  // single owner of that listener), so "install from onboarding" and "install
  // from Settings" can never fight over progress state.
  // Card data and copy keys live in #lib/onboarding/engine-choices.ts.

  type OnboardingStatus = {
    has_config: boolean;
    engine: string;
    engine_name: string;
    voice: string;
    keyless: boolean;
  };

  // Shared-model engine: one download covers every voice. Mirrors the default
  // set the installer records in manifest.json (Settings path passes the same).
  const KOKORO_VOICES = [
    "af_heart",
    "af_bella",
    "af_nicole",
    "af_sarah",
    "am_adam",
    "am_michael",
    "bf_emma",
    "bm_george"
  ];

  let selectedEngine = $state("kokoro");
  let status = $state<OnboardingStatus | null>(null);
  let isLoading = $state(true);
  let isSaving = $state(false);
  let kokoroInstalled = $state<boolean | null>(null);
  // Guards so a settled install run is handled (auto-complete / failure toast)
  // exactly once, even if the effect re-runs.
  let settledHandled = $state(false);
  let launchFailed = $state(false);

  const kokoroRun = $derived(installStore.run("kokoro"));
  const isInstalling = $derived(kokoroRun?.running ?? false);

  async function probeKokoro() {
    try {
      const s = await invoke<{ installed: boolean }>("engine_status", { engine: "kokoro" });
      kokoroInstalled = s.installed;
    } catch {
      kokoroInstalled = false;
    }
  }

  onMount(async () => {
    try {
      status = await invoke<OnboardingStatus>("get_onboarding_status");
      if (status.has_config) selectedEngine = status.engine;
    } catch (e) {
      // Even if this fails, defaults are safe: Kokoro is the fresh-install
      // default in Rust, and completing with any offered engine is valid.
      console.error("Failed to load onboarding status:", e);
      status = {
        has_config: false,
        engine: "kokoro",
        engine_name: "Kokoro",
        voice: "af_heart",
        keyless: true
      };
    } finally {
      isLoading = false;
    }
    void probeKokoro();
  });

  // A settled kokoro install run either completes onboarding (exit 0) or
  // surfaces the failure and stays here for a retry / different choice.
  $effect(() => {
    const run = kokoroRun;
    if (!run || run.running || settledHandled) return;
    settledHandled = true;
    if (run.exitCode === 0) {
      void finish("kokoro");
    } else if (!launchFailed) {
      toast.error($_("toast.error.kokoroInstallFailed"));
    }
  });

  async function finish(engineId: string) {
    isSaving = true;
    try {
      await invoke("complete_onboarding", { engine: engineId });
      toast.success($_("toast.success.welcome"));
      await goto("/");
    } catch (e) {
      console.error("Failed to complete onboarding:", e);
      toast.error($_("toast.error.saveFailed", { values: { error: String(e) } }));
    } finally {
      isSaving = false;
    }
  }

  async function startKokoroInstall() {
    launchFailed = false;
    settledHandled = false;
    await installStore.start("kokoro", KOKORO_VOICES);
    try {
      await invoke("install_engine", { engine: "kokoro", voice: KOKORO_VOICES, cuda: false });
    } catch (e) {
      launchFailed = true;
      toast.error($_("toast.error.kokoroInstallStart", { values: { error: String(e) } }));
    }
  }

  function primaryAction() {
    if (selectedEngine === "kokoro" && !kokoroInstalled) {
      void startKokoroInstall();
    } else {
      void finish(selectedEngine);
    }
  }

  let lastLogLine = $derived(kokoroRun?.lines[kokoroRun.lines.length - 1] ?? "");
</script>

<div class="flex min-h-dvh items-center justify-center p-4 sm:p-8">
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
              <h2 class="text-lg font-semibold">{$_("onboarding.chooseEngine")}</h2>
              <p class="text-muted-foreground text-sm leading-relaxed">
                {$_("onboarding.switchAnytime")}
              </p>
            </div>
          </div>

          <div class="mt-4 grid gap-3">
            {#each ENGINE_CHOICES as choice (choice.id)}
              <button
                type="button"
                onclick={() => (selectedEngine = choice.id)}
                class={selectedEngine === choice.id
                  ? "border-primary ring-primary bg-primary/5 rounded-lg border p-4 text-start transition-all focus-visible:ring-2 focus-visible:outline-none"
                  : "border-border hover:border-primary/60 rounded-lg border p-4 text-start transition-all focus-visible:ring-2 focus-visible:outline-none"}
              >
                <div class="flex items-center justify-between gap-3">
                  <div class="flex flex-wrap items-center gap-2">
                    <span class="font-medium">{choice.name}</span>
                    {#if choice.recommended}
                      <span
                        class="bg-primary/10 text-primary inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium"
                      >
                        <Sparkles class="h-3 w-3" />
                        {$_("onboarding.recommended")}
                      </span>
                    {/if}
                    {#if choice.installable && kokoroInstalled}
                      <span
                        class="inline-flex items-center gap-1 rounded-full bg-emerald-500/10 px-2 py-0.5 text-xs font-medium text-emerald-600 dark:text-emerald-400"
                      >
                        <Check class="h-3 w-3" />
                        {$_("onboarding.installed")}
                      </span>
                    {/if}
                  </div>
                  {#if choice.keyless}
                    <span class="text-muted-foreground inline-flex items-center gap-1 text-xs">
                      <Zap class="h-3 w-3" />
                      {$_("onboarding.noKeyNeeded")}
                    </span>
                  {:else}
                    <span class="text-muted-foreground inline-flex items-center gap-1 text-xs">
                      <KeyRound class="h-3 w-3" />
                      {$_("onboarding.apiKeyInSettings")}
                    </span>
                  {/if}
                </div>
                <p class="text-muted-foreground mt-1.5 text-sm leading-relaxed">
                  {$_(choice.detailKey)}
                </p>
              </button>
            {/each}
          </div>

          {#if selectedEngine === "kokoro"}
            <!-- Kokoro inline install status: stream here, or show the green verdict. -->
            <div class="border-border bg-muted/30 rounded-lg border p-3">
              {#if isInstalling}
                <div class="flex items-start gap-2">
                  <Loader2 size={16} class="text-muted-foreground mt-0.5 animate-spin" />
                  <div class="min-w-0 flex-1">
                    <p class="text-sm font-medium">{$_("onboarding.installingKokoro")}</p>
                    {#if lastLogLine}
                      <p
                        class="text-muted-foreground mt-1 truncate font-mono text-[11px]"
                        title={lastLogLine}
                      >
                        {lastLogLine}
                      </p>
                    {/if}
                  </div>
                </div>
              {:else if kokoroRun && !kokoroRun.running && kokoroRun.exitCode !== 0}
                <div class="flex items-start justify-between gap-2">
                  <div class="flex items-start gap-2">
                    <AlertCircle size={16} class="text-destructive mt-0.5" />
                    <p class="text-destructive text-sm">{$_("onboarding.installFailed")}</p>
                  </div>
                  <Button variant="outline" size="sm" onclick={startKokoroInstall}>
                    <RotateCw size={12} class="me-1" />
                    {$_("common.tryAgain")}
                  </Button>
                </div>
              {:else if kokoroInstalled}
                <p
                  class="inline-flex items-center gap-1 text-sm text-emerald-600 dark:text-emerald-400"
                >
                  <Check size={14} />
                  {$_("onboarding.kokoroReady")}
                </p>
              {:else}
                <p class="text-muted-foreground text-sm">
                  {$_("onboarding.kokoroInstallHint")}
                </p>
              {/if}
            </div>
          {:else}
            <p class="text-muted-foreground mt-2 text-xs">
              {$_("onboarding.otherLocalEngines")}
            </p>
          {/if}
        </div>

        <!-- Action Button -->
        <div class="flex flex-col gap-3 pt-2">
          <Button
            size="lg"
            onclick={primaryAction}
            disabled={isSaving ||
              isInstalling ||
              (selectedEngine === "kokoro" && kokoroInstalled === null)}
            class="w-full"
          >
            {#if isSaving}
              {$_("common.saving")}
            {:else if isInstalling}
              <Loader2 size={14} class="me-2 animate-spin" />
              {$_("onboarding.installing")}
            {:else if selectedEngine === "kokoro" && !kokoroInstalled}
              <Download size={14} class="me-2" />
              {$_("onboarding.installKokoroCta")}
            {:else}
              {$_("onboarding.complete")}
            {/if}
          </Button>
        </div>
      {/if}
    </div>
  </div>
</div>
