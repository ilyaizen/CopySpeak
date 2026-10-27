<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import HistoryPage from "#lib/components/history-page.svelte";
  import { invoke, isTauri } from "#lib/services/tauri";
  import type { AppConfig } from "#lib/types";
  import { isHistoryEffective } from "#lib/experimental-gates";
  import { _ } from "svelte-i18n";

  let historyAllowed = $state<boolean | null>(null);

  onMount(async () => {
    if (!isTauri) {
      historyAllowed = false;
      goto("/", { replaceState: true });
      return;
    }
    try {
      const config = await invoke<AppConfig>("get_config");
      historyAllowed = isHistoryEffective(config);
    } catch {
      historyAllowed = false;
    }
    if (historyAllowed === false) {
      goto("/", { replaceState: true });
    }
  });
</script>

{#if historyAllowed === true}
  <HistoryPage />
{:else if historyAllowed === false}
  <div
    class="flex flex-col items-center justify-center gap-3 py-16 text-center"
    data-testid="history-inactive"
  >
    <p class="text-foreground text-sm font-medium">{$_("settings.history.enabled")}</p>
    <p class="text-muted-foreground max-w-sm text-xs">
      {$_("settings.history.enabledDescription")}
    </p>
    <a
      href="/settings"
      class="text-primary text-xs font-medium hover:underline"
      data-testid="history-settings-link"
    >
      {$_("header.settings")}
    </a>
  </div>
{/if}
