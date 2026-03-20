<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { AppConfig, SupportedLocale } from "$lib/types";
  import { _ } from "svelte-i18n";
  import { setLocale } from "$lib/i18n";
  import { getSupportedLocales } from "$lib/i18n/utils";

  let {
    localConfig = $bindable(),
    showDebugMode
  }: {
    localConfig: AppConfig;
    showDebugMode: boolean;
  } = $props();

  const localeOptions = getSupportedLocales();

  function handleLocaleChange(newLocale: string) {
    localConfig.general.locale = newLocale as SupportedLocale;
    setLocale(newLocale as SupportedLocale);
  }

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
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <Label for="start-windows">Start with Windows</Label>
      <InfoTooltip text="Launch CopySpeak when Windows starts" />
    </div>
    <Switch id="start-windows" bind:checked={localConfig.general.start_with_windows} />
  </div>

  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <Label for="start-minimized">Start Minimized</Label>
      <InfoTooltip text="Start the application minimized to system tray" />
    </div>
    <Switch id="start-minimized" bind:checked={localConfig.general.start_minimized} />
  </div>

  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <Label for="show-notifications">Show Notifications</Label>
      <InfoTooltip text="Show system notifications for TTS events" />
    </div>
    <Switch id="show-notifications" bind:checked={localConfig.general.show_notifications} />
  </div>

  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <Label for="minimize-to-tray">Minimize to Tray on Close</Label>
      <InfoTooltip text="Minimize to system tray instead of exiting when closing the window" />
    </div>
    <Switch
      id="minimize-to-tray"
      checked={localConfig.general.close_behavior === "minimize-to-tray"}
      onchange={(v) => {
        localConfig.general.close_behavior = v ? "minimize-to-tray" : "exit";
      }}
    />
  </div>

  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <Label for="update-checks">Check for Updates</Label>
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

  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1.5">
      <Label for="language">{$_("settings.general.language")}</Label>
      <InfoTooltip text={$_("settings.general.languageDescription")} />
    </div>
    <Select
      id="language"
      options={localeOptions}
      value={localConfig.general.locale}
      onchange={(e: Event) => handleLocaleChange((e.target as HTMLSelectElement).value)}
      class="w-32"
    />
  </div>

  {#if showDebugMode}
    <div class="border-border mt-4 border-t pt-4">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1.5">
          <Label for="debug-mode" class="text-amber-600 dark:text-amber-400">Debug Mode</Label>
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
</div>
