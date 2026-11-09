<script lang="ts">
  import { Button } from "#lib/components/ui/button/index.js";
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { Switch } from "#lib/components/ui/switch/index.js";
  import { Badge } from "#lib/components/ui/badge/index.js";
  import UpdateChecker from "#lib/components/update-checker.svelte";
  import { openExternal } from "#lib/utils/external-link";
  import { invoke } from "@tauri-apps/api/core";
  import { VERSION } from "#lib/version";
  import { _ } from "svelte-i18n";
  import type { AppConfig } from "#lib/types";
  import type { Snippet } from "svelte";

  let {
    localConfig = $bindable(),
    showDebugMode = false,
    children
  }: {
    localConfig: AppConfig;
    showDebugMode?: boolean;
    children?: Snippet;
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
  <SettingRow
    label={$_("settings.about.version")}
    tooltip={$_("settings.about.versionDescription")}
  >
    <span class="text-muted-foreground font-mono text-sm">v{VERSION}</span>
  </SettingRow>

  <!-- Check for Updates -->
  <SettingRow
    label={$_("settings.about.checkUpdates")}
    tooltip={$_("settings.about.checkUpdatesTooltip")}
  >
    <Switch
      id="update-checks"
      checked={localConfig.general.update_checks_enabled ?? true}
      onchange={(v) => {
        localConfig.general.update_checks_enabled = v;
      }}
    />
  </SettingRow>

  <SettingRow
    label={$_("settings.about.updateStatus")}
    tooltip={$_("settings.about.updateStatusTooltip")}
  >
    <UpdateChecker
      enabled={localConfig.general.update_checks_enabled ?? true}
      autoCheckOnMount={false}
      listenForBackendEvent={false}
    />
  </SettingRow>

  <!-- Contribute -->
  <SettingRow
    label={$_("settings.about.sourceCode")}
    tooltip={$_("settings.about.sourceCodeDescription")}
  >
    <Button
      variant="outline"
      size="sm"
      onclick={() => openExternal("https://github.com/ilyaizen/CopySpeak")}
    >
      {$_("settings.about.github")}
    </Button>
  </SettingRow>

  <SettingRow label={$_("settings.about.donate")} tooltip={$_("settings.about.buyMeCoffee")}>
    <Button
      href="https://www.buymeacoffee.com/ilyaizen"
      target="_blank"
      rel="noopener noreferrer"
      variant="outline"
      size="sm"
      onclick={(event) => {
        if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey)
          return;
        event.preventDefault();
        void openExternal("https://www.buymeacoffee.com/ilyaizen");
      }}>{$_("settings.about.buyMeCoffee")}</Button
    >
  </SettingRow>

  {#if showDebugMode}
    <div>
      <SettingRow
        label={$_("settings.about.debugMode")}
        tooltip={$_("settings.about.debugModeTooltip")}
      >
        <Switch id="debug-mode" bind:checked={localConfig.general.debug_mode} />
      </SettingRow>

      {#if localConfig.general.debug_mode}
        <div class="mt-4 space-y-2">
          <div class="flex items-center gap-2">
            <Badge variant="secondary" class="text-[10px]">
              {logsPath || "Loading..."}
            </Badge>
            {#if isLoadingLogs}
              <span class="text-muted-foreground text-[10px]"
                >{$_("settings.about.refreshing")}</span
              >
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
                <div class="text-muted-foreground p-2 font-mono text-[10px]">
                  {$_("settings.about.noLogs")}
                </div>
              {/each}
            </div>
          </div>
        </div>
      {/if}
    </div>
  {/if}

  {@render children?.()}
</div>
