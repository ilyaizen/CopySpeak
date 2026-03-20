<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { invoke } from "$lib/services/tauri";
  import { toast } from "svelte-sonner";
  import { goto } from "$app/navigation";
  import type { AppConfig } from "$lib/types";
  import InfoTooltip from "../ui/info-tooltip.svelte";
  import { Upload, Download, RotateCcw, Copy } from "@lucide/svelte";
  import { _ } from "svelte-i18n";

  let {
    localConfig,
    onImport,
    onReset
  }: {
    localConfig: AppConfig | null;
    onImport: (config: AppConfig) => void;
    onReset: () => Promise<void>;
  } = $props();

  let showExportDialog = $state(false);
  let showImportDialog = $state(false);
  let showResetDialog = $state(false);
  let exportJson = $state("");
  let importJson = $state("");
  let importError = $state<string | null>(null);
  let isImporting = $state(false);

  function openExportDialog() {
    if (!localConfig) return;
    exportJson = JSON.stringify(localConfig, null, 2);
    showExportDialog = true;
  }

  function closeExportDialog() {
    showExportDialog = false;
    exportJson = "";
  }

  function openImportDialog() {
    importJson = "";
    importError = null;
    showImportDialog = true;
  }

  function closeImportDialog() {
    showImportDialog = false;
    importJson = "";
    importError = null;
  }

  async function copyToClipboard() {
    try {
      await navigator.clipboard.writeText(exportJson);
      toast.success($_("toast.success.settingsCopied"));
    } catch (e) {
      toast.error("Failed to copy to clipboard");
    }
  }

  function downloadJson() {
    const blob = new Blob([exportJson], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `copyspeak-settings-${new Date().toISOString().split("T")[0]}.json`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    toast.success($_("toast.success.settingsDownloaded"));
  }

  function handleFileSelect(event: Event) {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (e) => {
      importJson = e.target?.result as string;
      importError = null;
    };
    reader.onerror = () => {
      importError = "Failed to read file";
    };
    reader.readAsText(file);
    input.value = "";
  }

  async function validateAndImport() {
    if (!importJson.trim()) {
      importError = "Please enter or upload settings JSON";
      return;
    }

    isImporting = true;
    importError = null;

    try {
      const parsed = JSON.parse(importJson);

      // Basic validation - check for required top-level keys
      const requiredKeys = [
        "general",
        "tts",
        "playback",
        "trigger",
        "pagination",
        "filter",
        "sanitization",
        "app_filter",
        "hotkeys",
        "hud"
      ];
      const missingKeys = requiredKeys.filter((key) => !(key in parsed));

      if (missingKeys.length > 0) {
        throw new Error(`Invalid settings format. Missing keys: ${missingKeys.join(", ")}`);
      }

      // Validate with backend
      await invoke("validate_config", { config: parsed });

      onImport(parsed);
      closeImportDialog();
      toast.success($_("toast.success.settingsImported"));
    } catch (e) {
      if (e instanceof SyntaxError) {
        importError = "Invalid JSON format";
      } else {
        importError = e instanceof Error ? e.message : String(e);
      }
    } finally {
      isImporting = false;
    }
  }

  async function handleReset() {
    try {
      await onReset();
      showResetDialog = false;
      toast.success($_("toast.success.settingsReset"));
      await goto("/onboarding");
    } catch (e) {
      console.error("Failed to reset config:", e);
      toast.error(`Failed to reset settings: ${e}`);
    }
  }
</script>

<!-- Settings: unified Export / Import / Reset -->
<div class="border-border mb-4 border-b pb-4">
  <div class="flex items-center justify-between">
    <div>
      <div class="flex items-center gap-1.5">
        <p class="text-sm font-medium">Import / Export</p>
        <InfoTooltip text="Export, import, or reset your settings" />
      </div>
    </div>
    <div class="flex gap-2">
      <Button variant="outline" size="sm" onclick={openExportDialog}>
        <Upload class="mr-1.5" size={14} />
        Export
      </Button>
      <Button variant="outline" size="sm" onclick={openImportDialog}>
        <Download class="mr-1.5" size={14} />
        Import
      </Button>
      <Button variant="destructive" size="sm" onclick={() => (showResetDialog = true)}>
        <RotateCcw class="mr-1.5" size={14} />
        Reset
      </Button>
    </div>
  </div>
</div>

<!-- Export Dialog -->
{#if showExportDialog}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
    <div
      class="bg-card border-border flex max-h-[90vh] w-full max-w-2xl flex-col rounded-lg border"
    >
      <div class="border-border border-b p-4">
        <h3 class="text-lg font-semibold">Export Settings</h3>
        <p class="text-muted-foreground text-sm">Copy the JSON below or download it as a file</p>
      </div>

      <div class="flex-1 overflow-hidden p-4">
        <textarea
          readonly
          class="bg-muted/50 border-border focus:ring-ring h-64 w-full resize-none rounded-md border p-3 font-mono text-xs focus:ring-2 focus:outline-none"
          value={exportJson}
        ></textarea>
      </div>

      <div class="border-border flex justify-end gap-2 border-t p-4">
        <Button variant="outline" onclick={closeExportDialog}>Close</Button>
        <Button variant="outline" onclick={copyToClipboard}>
          <Copy class="mr-2" size={16} />
          Copy
        </Button>
        <Button onclick={downloadJson}>
          <Upload class="mr-2" size={16} />
          Download
        </Button>
      </div>
    </div>
  </div>
{/if}

<!-- Import Dialog -->
{#if showImportDialog}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
    <div
      class="bg-card border-border flex max-h-[90vh] w-full max-w-2xl flex-col rounded-lg border"
    >
      <div class="border-border border-b p-4">
        <h3 class="text-lg font-semibold">Import Settings</h3>
        <p class="text-muted-foreground text-sm">Paste settings JSON below or upload a file</p>
      </div>

      <div class="flex-1 space-y-4 overflow-hidden p-4">
        <div class="flex items-center gap-2">
          <label class="flex-1">
            <input type="file" accept=".json" onchange={handleFileSelect} class="hidden" />
            <Button
              variant="outline"
              class="w-full"
              onclick={() =>
                document.querySelector<HTMLInputElement>('input[type="file"]')?.click()}
            >
              <Download class="mr-2" size={16} />
              Choose File
            </Button>
          </label>
        </div>

        <textarea
          class="bg-background border-border focus:ring-ring h-48 w-full resize-none rounded-md border p-3 font-mono text-xs focus:ring-2 focus:outline-none"
          placeholder="Paste settings JSON here..."
          bind:value={importJson}
        ></textarea>

        {#if importError}
          <div class="text-destructive bg-destructive/10 rounded-md p-3 text-sm">
            {importError}
          </div>
        {/if}
      </div>

      <div class="border-border flex justify-end gap-2 border-t p-4">
        <Button variant="outline" onclick={closeImportDialog}>Cancel</Button>
        <Button onclick={validateAndImport} disabled={isImporting || !importJson.trim()}>
          {#if isImporting}
            Importing...
          {:else}
            <Download class="mr-2" size={16} />
            Import Settings
          {/if}
        </Button>
      </div>
    </div>
  </div>
{/if}

<!-- Reset Confirmation Dialog -->
{#if showResetDialog}
  <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
    <div class="bg-card border-border w-full max-w-sm rounded-lg border p-6">
      <h3 class="mb-2 text-lg font-semibold">Reset All Settings?</h3>
      <p class="text-muted-foreground mb-6">
        This will restore all settings to their default values and open the initial setup wizard.
        This action cannot be undone.
      </p>
      <div class="flex justify-end gap-2">
        <Button variant="outline" onclick={() => (showResetDialog = false)}>Cancel</Button>
        <Button variant="destructive" onclick={handleReset}>Reset &amp; Restart Setup</Button>
      </div>
    </div>
  </div>
{/if}
