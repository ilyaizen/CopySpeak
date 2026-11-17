<script lang="ts">
  import { Button } from "#lib/components/ui/button/index.js";
  import { invoke } from "#lib/services/tauri";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { toast } from "svelte-sonner";
  import { goto } from "$app/navigation";
  import type { AppConfig } from "#lib/types";
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import {
    Drawer,
    DrawerClose,
    DrawerContent,
    DrawerTitle
  } from "#lib/components/ui/drawer/index.js";
  import { Upload, Download, RotateCcw, Copy, X } from "@lucide/svelte";
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
  let isDownloading = $state(false);
  let fileInput: HTMLInputElement | null = $state(null);

  function openExportDialog() {
    if (!localConfig) return;
    exportJson = JSON.stringify(localConfig, null, 2);
    showExportDialog = true;
  }

  function openImportDialog() {
    importJson = "";
    importError = null;
    showImportDialog = true;
  }

  async function copyToClipboard() {
    try {
      await navigator.clipboard.writeText(exportJson);
      toast.success($_("toast.success.settingsCopied"));
    } catch (e) {
      toast.error($_("toast.error.copyToClipboard"));
    }
  }

  async function downloadJson() {
    const suggested = await saveDialog({
      defaultPath: `copyspeech-settings-${new Date().toISOString().split("T")[0]}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }]
    });
    if (!suggested) return;
    isDownloading = true;
    try {
      await invoke("save_text_file", { path: suggested, contents: exportJson });
      toast.success($_("toast.success.settingsDownloaded"));
    } catch (e) {
      toast.error(String(e));
    } finally {
      isDownloading = false;
    }
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
      showImportDialog = false;
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

<!-- Export Drawer -->
<Drawer bind:open={showExportDialog}>
  <DrawerContent
    class="h-[min(85dvh,720px)] max-h-[85dvh] [&>div:first-child]:!hidden"
    onOpenAutoFocus={(e: Event) => e.preventDefault()}
  >
    <DrawerTitle class="sr-only">{$_("settings.importExport.exportTitle")}</DrawerTitle>
    <header class="flex shrink-0 items-center justify-between px-5 pt-3">
      <div>
        <h3 class="text-lg font-semibold">{$_("settings.importExport.exportTitle")}</h3>
        <p class="text-muted-foreground text-sm">{$_("settings.importExport.exportDescription")}</p>
      </div>
      <DrawerClose
        aria-label={$_("common.close")}
        class="text-muted-foreground hover:text-foreground focus-visible:ring-ring inline-flex size-9 items-center justify-center rounded-sm focus-visible:ring-2"
      >
        <X size={16} />
      </DrawerClose>
    </header>

    <div class="flex-1 overflow-hidden p-4">
      <textarea
        readonly
        class="bg-muted/50 border-border focus:ring-ring h-full w-full resize-none rounded-md border p-3 font-mono text-xs focus:ring-2 focus:outline-none"
        value={exportJson}></textarea>
    </div>

    <div class="border-border flex shrink-0 justify-end gap-2 border-t p-4">
      <Button variant="outline" onclick={() => (showExportDialog = false)}
        >{$_("common.close")}</Button
      >
      <Button variant="outline" onclick={copyToClipboard}>
        <Copy class="me-2" size={16} />
        Copy
      </Button>
      <Button onclick={downloadJson} disabled={isDownloading}>
        <Download class="me-2" size={16} />
        Download
      </Button>
    </div>
  </DrawerContent>
</Drawer>

<!-- Import Drawer -->
<Drawer bind:open={showImportDialog}>
  <DrawerContent
    class="h-[min(85dvh,720px)] max-h-[85dvh] [&>div:first-child]:!hidden"
    onOpenAutoFocus={(e: Event) => e.preventDefault()}
  >
    <DrawerTitle class="sr-only">{$_("settings.importExport.importTitle")}</DrawerTitle>
    <header class="flex shrink-0 items-center justify-between px-5 pt-3">
      <div>
        <h3 class="text-lg font-semibold">{$_("settings.importExport.importTitle")}</h3>
        <p class="text-muted-foreground text-sm">{$_("settings.importExport.importDescription")}</p>
      </div>
      <DrawerClose
        aria-label={$_("common.close")}
        class="text-muted-foreground hover:text-foreground focus-visible:ring-ring inline-flex size-9 items-center justify-center rounded-sm focus-visible:ring-2"
      >
        <X size={16} />
      </DrawerClose>
    </header>

    <div class="flex-1 space-y-4 overflow-hidden p-4">
      <label class="block">
        <input
          type="file"
          accept=".json"
          onchange={handleFileSelect}
          class="hidden"
          bind:this={fileInput}
        />
        <Button variant="outline" class="w-full" onclick={() => fileInput?.click()}>
          <Upload class="me-2" size={16} />
          Choose File
        </Button>
      </label>

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

    <div class="border-border flex shrink-0 justify-end gap-2 border-t p-4">
      <Button variant="outline" onclick={() => (showImportDialog = false)}
        >{$_("common.cancel")}</Button
      >
      <Button onclick={validateAndImport} disabled={isImporting || !importJson.trim()}>
        {#if isImporting}
          Importing...
        {:else}
          <Download class="me-2" size={16} />
          Import Settings
        {/if}
      </Button>
    </div>
  </DrawerContent>
</Drawer>

<!-- Reset Confirmation Drawer -->
<Drawer bind:open={showResetDialog}>
  <DrawerContent class="max-h-[85dvh] pb-6 [&>div:first-child]:!hidden">
    <DrawerTitle class="sr-only">{$_("settings.importExport.resetTitle")}</DrawerTitle>
    <div class="px-8 pt-4">
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
  </DrawerContent>
</Drawer>
