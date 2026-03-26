<script lang="ts">
  import { SettingRow } from "$lib/components/ui/setting-row/index.js";
  import { Switch } from "$lib/components/ui/switch/index.js";
  import { Badge } from "$lib/components/ui/badge/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { AppConfig } from "$lib/types";

  let {
    localConfig = $bindable(),
    showDebugMode
  }: {
    localConfig: AppConfig;
    showDebugMode: boolean;
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
  <SettingRow label="Start with Windows" tooltip="Launch CopySpeak when Windows starts">
    <Switch id="start-windows" bind:checked={localConfig.general.start_with_windows} />
  </SettingRow>

  <SettingRow label="Start Minimized" tooltip="Start the application minimized to system tray">
    <Switch id="start-minimized" bind:checked={localConfig.general.start_minimized} />
  </SettingRow>

  <SettingRow label="Show Notifications" tooltip="Show system notifications for TTS events">
    <Switch id="show-notifications" bind:checked={localConfig.general.show_notifications} />
  </SettingRow>

  <SettingRow
    label="Minimize to Tray on Close"
    tooltip="Minimize to system tray instead of exiting when closing the window"
  >
    <Switch
      id="minimize-to-tray"
      checked={localConfig.general.close_behavior === "minimize-to-tray"}
      onchange={(v) => {
        localConfig.general.close_behavior = v ? "minimize-to-tray" : "exit";
      }}
    />
  </SettingRow>

  <SettingRow label="Check for Updates" tooltip="Automatically check for new versions on startup">
    <Switch
      id="update-checks"
      checked={localConfig.general.update_checks_enabled ?? true}
      onchange={(v) => {
        localConfig.general.update_checks_enabled = v;
      }}
    />
  </SettingRow>

  {#if showDebugMode}
    <div class="border-border mt-4 border-t pt-4">
      <SettingRow label="Debug Mode" tooltip="Enable verbose logging and additional status info">
        <Switch id="debug-mode" bind:checked={localConfig.general.debug_mode} />
      </SettingRow>

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
