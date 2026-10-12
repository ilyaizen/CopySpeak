<script lang="ts">
  import { Button } from "#lib/components/ui/button/index.js";
  import { invoke } from "#lib/services/tauri";
  import { toast } from "svelte-sonner";
  import { goto } from "$app/navigation";
  import type { AppConfig } from "#lib/types";
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { Upload, Download, RotateCcw, Copy } from "@lucide/svelte";
  import { _ } from "svelte-i18n";
  import { portal } from "#lib/utils";

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
      toast.error($_("toast.error.copyToClipboard"));
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
    // SAFETY: this handler is only bound to the file <input>, whose target is that element.
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (e) => {
      // SAFETY: readAsText resolves result as string in this onload handler.
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
      toast.error($_("toast.error.resetSettings", { values: { error: String(e) } }));
    }
  }
</script>

<!-- Settings: unified Export / Import / Reset -->
<SettingRow
  label={$_("settings.sections.importExport")}
  tooltip={$_("settings.importExport.tooltip")}
>
  <div class="flex gap-2">
    <Button variant="outline" size="sm" onclick={openExportDialog}>
      <Upload class="me-1.5" size={14} />
      Export
    </Button>
    <Button variant="outline" size="sm" onclick={openImportDialog}>
      <Download class="me-1.5" size={14} />
      Import
    </Button>
    <Button variant="destructive" size="sm" onclick={() => (showResetDialog = true)}>
      <RotateCcw class="me-1.5" size={14} />
      Reset
    </Button>
  </div>
</SettingRow>

<!-- Export Dialog -->
{#if showExportDialog}
  <div use:portal class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
    <div
      class="bg-card border-border flex max-h-[90vh] w-full max-w-2xl flex-col rounded-lg border"
    >
      <div class="border-border border-b p-4">
        <h3 class="text-lg font-semibold">{$_("settings.importExport.exportTitle")}</h3>
        <p class="text-muted-foreground text-sm">{$_("settings.importExport.exportDescription")}</p>
      </div>

      <div class="flex-1 overflow-hidden p-4">
        <textarea
          readonly
          class="bg-muted/50 border-border focus:ring-ring h-64 w-full resize-none rounded-md border p-3 font-mono text-xs focus:ring-2 focus:outline-none"
          value={exportJson}></textarea>
      </div>

      <div class="border-border flex justify-end gap-2 border-t p-4">
        <Button variant="outline" onclick={closeExportDialog}>{$_("common.close")}</Button>
        <Button variant="outline" onclick={copyToClipboard}>
          <Copy class="me-2" size={16} />
          Copy
        </Button>
        <Button onclick={downloadJson}>
          <Upload class="me-2" size={16} />
          Download
        </Button>
      </div>
    </div>
  </div>
{/if}

<!-- Import Dialog -->
{#if showImportDialog}
  <div use:portal class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
    <div
      class="bg-card border-border flex max-h-[90vh] w-full max-w-2xl flex-col rounded-lg border"
    >
      <div class="border-border border-b p-4">
        <h3 class="text-lg font-semibold">{$_("settings.importExport.importTitle")}</h3>
        <p class="text-muted-foreground text-sm">{$_("settings.importExport.importDescription")}</p>
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
              <Download class="me-2" size={16} />
              Choose File
            </Button>
          </label>
        </div>

        <textarea
          class="bg-background border-border focus:ring-ring h-48 w-full resize-none rounded-md border p-3 font-mono text-xs focus:ring-2 focus:outline-none"
          placeholder={$_("settings.importExport.pastePlaceholder")}
          bind:value={importJson}></textarea>

        {#if importError}
          <div class="text-destructive bg-destructive/10 rounded-md p-3 text-sm">
            {importError}
          </div>
        {/if}
      </div>

      <div class="border-border flex justify-end gap-2 border-t p-4">
        <Button variant="outline" onclick={closeImportDialog}>{$_("common.cancel")}</Button>
        <Button onclick={validateAndImport} disabled={isImporting || !importJson.trim()}>
          {#if isImporting}
            Importing...
          {:else}
            <Download class="me-2" size={16} />
            Import Settings
          {/if}
        </Button>
      </div>
    </div>
  </div>
{/if}

<!-- Reset Confirmation Dialog -->
{#if showResetDialog}
  <div use:portal class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
    <div class="bg-card border-border w-full max-w-sm rounded-lg border p-6">
      <h3 class="mb-2 text-lg font-semibold">{$_("settings.importExport.resetTitle")}</h3>
      <p class="text-muted-foreground mb-6">
        This will restore all settings to their default values and open the initial setup wizard.
        This action cannot be undone.
      </p>
      <div class="flex justify-end gap-2">
        <Button variant="outline" onclick={() => (showResetDialog = false)}
          >{$_("common.cancel")}</Button
        >
        <Button variant="destructive" onclick={handleReset}
          >{$_("settings.importExport.resetConfirm")}</Button
        >
      </div>
    </div>
  </div>
{/if}
