<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { check } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { cn } from "$lib/utils.js";
  import { Download } from "@lucide/svelte";
  import { Spinner } from "$lib/components/ui/spinner/index.js";
  import type { AppConfig } from "$lib/types";
  import { VERSION } from "$lib/version";

  let updateAvailable = $state(false);
  let isChecking = $state(false);
  let isInstalling = $state(false);
  let downloadProgress = $state(0);
  let showUpToDate = $state(false);
  let updateChecksEnabled = $state(true);
  let errorMessage = $state<string | null>(null);

  let unlisten: (() => void) | null = null;
  let upToDateTimeout: ReturnType<typeof setTimeout> | null = null;
  let errorTimeout: ReturnType<typeof setTimeout> | null = null;
  let downloadedBytes = 0;
  let contentLength = 0;

  onMount(async () => {
    // Load config to check if updates are enabled
    try {
      const config = await invoke<AppConfig>("get_config");
      updateChecksEnabled = config.general.update_checks_enabled ?? true;
    } catch {
      // Default to enabled if config load fails
      updateChecksEnabled = true;
    }

    // Listen for update check events from backend
    const unlistenEvent = await listen("check-for-updates", () => {
      handleManualCheck();
    });
    unlisten = unlistenEvent;

    // Auto-check on startup if enabled
    if (updateChecksEnabled) {
      checkForUpdates();
    }
  });

  onDestroy(() => {
    if (unlisten) unlisten();
    if (upToDateTimeout) clearTimeout(upToDateTimeout);
    if (errorTimeout) clearTimeout(errorTimeout);
  });

  function showError(message: string) {
    errorMessage = message;
    if (errorTimeout) clearTimeout(errorTimeout);
    errorTimeout = setTimeout(() => {
      errorMessage = null;
    }, 5000);
  }

  async function checkForUpdates() {
    if (!updateChecksEnabled || isChecking) return;

    try {
      isChecking = true;
      errorMessage = null;
      console.log(`[UpdateChecker v${VERSION}] Checking for updates...`);
      const update = await check();

      if (update) {
        console.log(`[UpdateChecker v${VERSION}] Update available: ${update.version}`);
        updateAvailable = true;
        showUpToDate = false;
      } else {
        console.log(`[UpdateChecker v${VERSION}] No update available - already on latest`);
        updateAvailable = false;
        // Only show "up to date" for manual checks
      }
    } catch (error) {
      const errorMsg = error instanceof Error ? error.message : String(error);
      console.error(`[UpdateChecker v${VERSION}] Update check failed:`, errorMsg);

      // Provide accurate error messages based on error type
      if (errorMsg.includes("Could not fetch") || errorMsg.includes("release JSON")) {
        // This error can occur in both dev and production
        // Common causes: no release exists, latest.json missing, network issues
        console.error(`[UpdateChecker v${VERSION}] Possible causes:`);
        console.error(`  - No release exists at https://github.com/ilyaizen/CopySpeak/releases`);
        console.error(`  - latest.json file missing from release`);
        console.error(`  - Network connectivity issues`);
        showError("Cannot reach update server");
      } else if (errorMsg.includes("signature") || errorMsg.includes("verify")) {
        showError("Update signature invalid");
      } else if (errorMsg.includes("network") || errorMsg.includes("fetch")) {
        showError("Network error");
      } else {
        showError("Update check failed");
      }
    } finally {
      isChecking = false;
    }
  }

  async function handleManualCheck() {
    await checkForUpdates();
    // Show "up to date" message for manual checks when no update available
    if (!updateAvailable) {
      showUpToDate = true;
      if (upToDateTimeout) clearTimeout(upToDateTimeout);
      upToDateTimeout = setTimeout(() => {
        showUpToDate = false;
      }, 3000);
    }
  }

  async function installUpdate() {
    if (!updateChecksEnabled) return;

    try {
      isInstalling = true;
      downloadProgress = 0;
      downloadedBytes = 0;
      contentLength = 0;

      const update = await check();
      if (!update) {
        console.log("No update available during install attempt");
        return;
      }

      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            downloadedBytes = 0;
            contentLength = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloadedBytes += event.data.chunkLength;
            const progress =
              contentLength > 0 ? Math.round((downloadedBytes / contentLength) * 100) : 0;
            downloadProgress = Math.min(progress, 100);
            break;
        }
      });

      await relaunch();
    } catch (error) {
      console.error("Failed to install update:", error);
      showError("Failed to install update");
    } finally {
      isInstalling = false;
      downloadProgress = 0;
    }
  }

  function getStatusText(): string {
    if (!updateChecksEnabled) return "Updates disabled";
    if (isInstalling) {
      if (downloadProgress > 0 && downloadProgress < 100) {
        return `Downloading ${downloadProgress}%`;
      }
      return downloadProgress === 100 ? "Installing..." : "Preparing...";
    }
    if (errorMessage) return errorMessage;
    if (isChecking) return "Checking...";
    if (showUpToDate) return "Up to date";
    if (updateAvailable) return "Update available";
    return "Check for updates";
  }

  function handleClick() {
    if (!updateChecksEnabled || isChecking || isInstalling) return;
    if (updateAvailable) {
      installUpdate();
    } else {
      handleManualCheck();
    }
  }
</script>

<div class="flex items-center gap-2">
  {#if isInstalling && downloadProgress > 0 && downloadProgress < 100}
    <div class="bg-muted h-1.5 w-20 overflow-hidden rounded-full">
      <div class="bg-primary h-full transition-all" style="width: {downloadProgress}%"></div>
    </div>
  {/if}

  <button
    onclick={handleClick}
    disabled={!updateChecksEnabled || isChecking || isInstalling}
    class={cn(
      "text-xs transition-colors disabled:opacity-50",
      errorMessage
        ? "text-destructive hover:text-destructive/80"
        : updateAvailable
          ? "text-primary hover:text-primary/80 font-medium"
          : "text-muted-foreground hover:text-foreground"
    )}
  >
    {#if isChecking || isInstalling}
      <Spinner class="mr-1 inline h-3 w-3" />
    {:else if updateAvailable}
      <Download class="mr-1 inline h-3 w-3" />
    {:else}
      <!-- No icon when no update available and not checking -->
    {/if}
    {getStatusText()}
  </button>
</div>
