<script lang="ts">
  import { _ } from "svelte-i18n";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { Button } from "#lib/components/ui/button/index.js";
  import {
    Drawer,
    DrawerClose,
    DrawerContent,
    DrawerTitle
  } from "#lib/components/ui/drawer/index.js";
  import { Copy, Download, Upload, X } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import { invoke } from "#lib/services/tauri";
  import type { VoiceProfile } from "#lib/types";
  import { isValidProfile, normalizeImportedProfile } from "./profile-validation";

  type DialogMode = "export" | "import" | "delete";

  let {
    mode,
    profile,
    onImport,
    onDelete,
    onClose
  }: {
    mode: DialogMode | null;
    profile: VoiceProfile | null;
    onImport: (profile: VoiceProfile) => void;
    onDelete: () => void;
    onClose: () => void;
  } = $props();

  const open = $derived(mode !== null);

  let exportJson = $state("");
  let importJson = $state("");
  let importError = $state<string | null>(null);
  let isDownloading = $state(false);
  let fileInput: HTMLInputElement | null = $state(null);

  $effect(() => {
    if (mode === "export" && profile) {
      exportJson = JSON.stringify({ schema_version: 3, ...profile }, null, 2);
    }
  });

  async function copyToClipboard() {
    try {
      await navigator.clipboard.writeText(exportJson);
      toast.success($_("profiles.toast.copied"));
    } catch {
      toast.error($_("profiles.toast.copyFailed"));
    }
  }

  async function downloadJson() {
    if (!profile) return;
    const suggested = await saveDialog({
      defaultPath: `${profile.id}.json`,
      filters: [{ name: "JSON", extensions: ["json"] }]
    });
    if (!suggested) return;
    isDownloading = true;
    try {
      await invoke("save_text_file", { path: suggested, contents: exportJson });
      toast.success($_("profiles.toast.downloaded"));
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

  function validateAndImport() {
    importError = null;
    if (!importJson.trim()) {
      importError = "Paste or upload profile JSON";
      return;
    }
    try {
      const parsed = JSON.parse(importJson);
      if (!isValidProfile(parsed)) {
        importError =
          "Invalid profile: missing required fields (id, name, engine, voice, speed, pitch, effects)";
        return;
      }
      onImport(normalizeImportedProfile(parsed));
    } catch (e) {
      importError = e instanceof SyntaxError ? "Invalid JSON" : String(e);
    }
  }
</script>

<Drawer {open} onOpenChange={(o) => (o ? null : onClose())}>
  <DrawerContent
    class="h-[min(85dvh,720px)] max-h-[85dvh] [&>div:first-child]:!hidden"
    onOpenAutoFocus={(e: Event) => e.preventDefault()}
  >
    <DrawerTitle class="sr-only">
      {#if mode === "export"}
        Export Profile
      {:else if mode === "import"}
        Import Profile
      {:else}
        Delete Profile
      {/if}
    </DrawerTitle>

    <header class="flex shrink-0 items-center justify-between px-5 pt-3">
      <div>
        <h3 class="text-lg font-semibold">
          {#if mode === "export"}
            Export Profile
          {:else if mode === "import"}
            Import Profile
          {:else}
            Delete Profile
          {/if}
        </h3>
        {#if mode === "export" && profile}
          <p class="text-muted-foreground text-sm">{profile.name}</p>
        {/if}
      </div>
      <DrawerClose
        aria-label={$_("common.close")}
        class="text-muted-foreground hover:text-foreground focus-visible:ring-ring inline-flex size-9 items-center justify-center rounded-sm focus-visible:ring-2"
      >
        <X size={16} />
      </DrawerClose>
    </header>

    {#if mode === "export"}
      <div class="flex-1 overflow-hidden p-4">
        <textarea
          readonly
          class="bg-muted/50 border-border focus:ring-ring h-full w-full resize-none rounded-md border p-3 font-mono text-xs focus:ring-2 focus:outline-none"
          value={exportJson}></textarea>
      </div>
      <div class="border-border flex shrink-0 justify-end gap-2 border-t p-4">
        <Button variant="outline" onclick={onClose}>{$_("common.close")}</Button>
        <Button variant="outline" onclick={copyToClipboard}>
          <Copy class="me-2" size={16} />
          Copy
        </Button>
        <Button onclick={downloadJson} disabled={isDownloading}>
          <Download class="me-2" size={16} />
          Download
        </Button>
      </div>
    {:else if mode === "import"}
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
          placeholder={$_("profiles.importPlaceholder")}
          bind:value={importJson}></textarea>

        {#if importError}
          <div class="text-destructive bg-destructive/10 rounded-md p-3 text-sm">
            {importError}
          </div>
        {/if}
      </div>
      <div class="border-border flex shrink-0 justify-end gap-2 border-t p-4">
        <Button variant="outline" onclick={onClose}>{$_("common.cancel")}</Button>
        <Button onclick={validateAndImport} disabled={!importJson.trim()}>
          <Upload class="me-2" size={16} />
          Import Profile
        </Button>
      </div>
    {:else if mode === "delete" && profile}
      <div class="flex-1 p-6">
        <p class="text-muted-foreground">
          Delete <strong class="text-foreground">{profile.name}</strong>? This cannot be undone.
        </p>
      </div>
      <div class="border-border flex shrink-0 justify-end gap-2 border-t p-4">
        <Button variant="outline" onclick={onClose}>{$_("common.cancel")}</Button>
        <Button variant="destructive" onclick={onDelete}>{$_("common.delete")}</Button>
      </div>
    {/if}
  </DrawerContent>
</Drawer>
