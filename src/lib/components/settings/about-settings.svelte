<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import { openExternal } from "$lib/utils/external-link";
  import { invoke } from "@tauri-apps/api/core";
  import { VERSION } from "$lib/version";
  import { _ } from "svelte-i18n";
  import type { AppConfig } from "$lib/types";

  let {
    localConfig = $bindable(),
    showDebugMode = false
  }: {
    localConfig: AppConfig;
    showDebugMode?: boolean;
  } = $props();

  let logs = $state<string[]>([]);
  let logsPath = $state("");
  let isLoadingLogs = $state(false);

  async function loadLogs() {
    isLoadingLogs = true;
    try {
      const rawLogs = await invoke<string>("get_logs", { maxLines: 20 });
      logsPath = await invoke("get_logs_path");
      logs = rawLogs
        .split("\n")
        .reverse()
        .filter((line) => line.trim().length > 0);
    } catch (error) {
      logs = [`Error loading logs: ${error}`];
    } finally {
      isLoadingLogs = false;
    }
  }

  $effect(() => {
    if (!localConfig.general.debug_mode) {
      logs = [];
      logsPath = "";
      return;
    }
    loadLogs();
    const id = setInterval(loadLogs, 2000);
    return () => clearInterval(id);
  });
</script>

<div class="space-y-4">
  <!-- Version -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <Label>{$_("settings.about.version")}</Label>
      <InfoTooltip text={$_("settings.about.versionDescription")} />
    </div>
    <span class="text-muted-foreground font-mono text-sm">v{VERSION}</span>
  </div>

  <!-- Check for Updates -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <Label>Check for Updates</Label>
      <InfoTooltip text="Automatically check for new versions on startup" />
    </div>
    <Switch
      id="update-checks"
      checked={localConfig.general.update_checks_enabled ?? true}
      onchange={(v) => {
        localConfig.general.update_checks_enabled = v;
      }}
    />
  </div>

  <!-- Source Code -->
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <Label>{$_("settings.about.sourceCode")}</Label>
      <InfoTooltip text={$_("settings.about.sourceCodeDescription")} />
    </div>
    <Button
      variant="outline"
      size="sm"
      onclick={() => openExternal("https://github.com/ilyaizen/CopySpeak")}
    >
      {$_("settings.about.github")}
    </Button>
  </div>

  {#if showDebugMode}
    <div class="border-border border-t pt-4">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1.5">
          <Label>Debug Mode</Label>
          <InfoTooltip text="Enable verbose logging and additional status info" />
        </div>
        <Switch id="debug-mode" bind:checked={localConfig.general.debug_mode} />
      </div>

      {#if localConfig.general.debug_mode}
        <div class="mt-4 space-y-2">
          <div class="flex items-center gap-2">
            <Badge variant="secondary" class="text-[10px]">
              {logsPath || "Loading..."}
            </Badge>
            {#if isLoadingLogs}
              <span class="text-muted-foreground text-[10px]">refreshing...</span>
            {/if}
          </div>
          <div class="bg-muted rounded-md border p-2">
            <div class="max-h-64 overflow-auto">
              {#each logs as line}
                <div class="odd:bg-muted/50 even:bg-transparent">
                  <pre
                    class="px-1 py-0.5 font-mono text-[10px] leading-tight break-all whitespace-pre-wrap">{line}</pre>
                </div>
              {:else}
                <div class="text-muted-foreground font-mono text-[10px] p-2">No logs available</div>
              {/each}
            </div>
          </div>
        </div>
      {/if}
    </div>
  {/if}

  <!-- Acknowledgments Section -->
  <div class="border-border mt-6 border-t pt-4">
    <h3 class="mb-3 text-sm font-semibold">{$_("settings.about.acknowledgments")}</h3>
    <div class="space-y-3">
      <!-- OpenAI -->
      <div>
        <div class="flex items-center gap-1.5">
          <Label class="text-sm">{$_("settings.about.openai.title")}</Label>
          <InfoTooltip text={$_("settings.about.openai.description")} />
        </div>
        <p class="text-muted-foreground mt-1 text-xs">
          {$_("settings.about.openai.detail")}
        </p>
      </div>

      <!-- ElevenLabs -->
      <div>
        <div class="flex items-center gap-1.5">
          <Label class="text-sm">{$_("settings.about.elevenlabs.title")}</Label>
          <InfoTooltip text={$_("settings.about.elevenlabs.description")} />
        </div>
        <p class="text-muted-foreground mt-1 text-xs">
          {$_("settings.about.elevenlabs.detail")}
        </p>
      </div>

      <!-- Kokoro -->
      <div>
        <div class="flex items-center gap-1.5">
          <Label class="text-sm">{$_("settings.about.kokoro.title")}</Label>
          <InfoTooltip text={$_("settings.about.kokoro.description")} />
        </div>
        <p class="text-muted-foreground mt-1 text-xs">
          {$_("settings.about.kokoro.detail")}
        </p>
      </div>

      <!-- Piper -->
      <div>
        <div class="flex items-center gap-1.5">
          <Label class="text-sm">{$_("settings.about.piper.title")}</Label>
          <InfoTooltip text={$_("settings.about.piper.description")} />
        </div>
        <p class="text-muted-foreground mt-1 text-xs">
          {$_("settings.about.piper.detail")}
        </p>
      </div>
    </div>
  </div>
</div>
