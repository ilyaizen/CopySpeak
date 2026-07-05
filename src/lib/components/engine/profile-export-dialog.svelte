<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Copy, Download, Upload, X } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import { portal } from "$lib/utils";
  import type { VoiceProfile, TtsEngine, EffectId } from "$lib/types";

  type DialogMode = "export" | "import" | "delete";

  let {
    mode,
    profile,
    onImport,
    onDelete,
    onClose
  }: {
    mode: DialogMode;
    profile: VoiceProfile | null;
    onImport: (profile: VoiceProfile) => void;
    onDelete: () => void;
    onClose: () => void;
  } = $props();

  const ENGINES: TtsEngine[] = [
    "local", "http", "openai", "elevenlabs", "cartesia", "google", "microsoft", "edge"
  ];
  const EFFECTS: EffectId[] = ["none", "walkie_talkie", "game_boy"];

  let exportJson = $state("");
  let importJson = $state("");
  let importError = $state<string | null>(null);

  function isValidProfile(p: unknown): p is VoiceProfile {
    if (!p || typeof p !== "object") return false;
    const o = p as Record<string, unknown>;
    return (
      typeof o.id === "string" &&
      typeof o.name === "string" &&
      typeof o.engine === "string" &&
      ENGINES.includes(o.engine as TtsEngine) &&
      typeof o.voice === "string" &&
      Number.isFinite(o.speed) &&
      Number.isFinite(o.pitch) &&
      typeof o.effects === "object" &&
      o.effects !== null &&
      EFFECTS.includes((o.effects as Record<string, unknown>).active_effect as EffectId)
    );
  }

  $effect(() => {
    if (mode === "export" && profile) {
      exportJson = JSON.stringify({ schema_version: 3, ...profile }, null, 2);
    }
  });

  async function copyToClipboard() {
    try {
      await navigator.clipboard.writeText(exportJson);
      toast.success("Copied to clipboard");
    } catch {
      toast.error("Failed to copy");
    }
  }

  function downloadJson() {
    if (!profile) return;
    const blob = new Blob([exportJson], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${profile.id}.json`;
    a.click();
    URL.revokeObjectURL(url);
    toast.success("Downloaded");
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

  function validateAndImport() {
    importError = null;
    if (!importJson.trim()) {
      importError = "Paste or upload profile JSON";
      return;
    }
    try {
      const parsed = JSON.parse(importJson);
      if (!isValidProfile(parsed)) {
        importError = "Invalid profile: missing required fields (id, name, engine, voice, speed, pitch, effects)";
        return;
      }
      onImport(parsed as VoiceProfile);
    } catch (e) {
      importError = e instanceof SyntaxError ? "Invalid JSON" : String(e);
    }
  }
</script>

<div use:portal class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
  <div class="bg-card border-border flex max-h-[90vh] w-full max-w-2xl flex-col rounded-lg border">
    <!-- Header -->
    <div class="border-border border-b p-4">
      <div class="flex items-center justify-between">
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
        <Button variant="ghost" size="sm" onclick={onClose}>
          <X size={16} />
        </Button>
      </div>
    </div>

    <!-- Body -->
    {#if mode === "export"}
      <div class="flex-1 overflow-hidden p-4">
        <textarea
          readonly
          class="bg-muted/50 border-border focus:ring-ring h-64 w-full resize-none rounded-md border p-3 font-mono text-xs focus:ring-2 focus:outline-none"
          value={exportJson}
        ></textarea>
      </div>
      <div class="border-border flex justify-end gap-2 border-t p-4">
        <Button variant="outline" onclick={onClose}>Close</Button>
        <Button variant="outline" onclick={copyToClipboard}>
          <Copy class="mr-2" size={16} />
          Copy
        </Button>
        <Button onclick={downloadJson}>
          <Download class="mr-2" size={16} />
          Download
        </Button>
      </div>
    {:else if mode === "import"}
      <div class="flex-1 space-y-4 overflow-hidden p-4">
        <label class="block">
          <input type="file" accept=".json" onchange={handleFileSelect} class="hidden" />
          <Button
            variant="outline"
            class="w-full"
            onclick={() => document.querySelector<HTMLInputElement>('input[type="file"]')?.click()}
          >
            <Upload class="mr-2" size={16} />
            Choose File
          </Button>
        </label>

        <textarea
          class="bg-background border-border focus:ring-ring h-48 w-full resize-none rounded-md border p-3 font-mono text-xs focus:ring-2 focus:outline-none"
          placeholder="Paste profile JSON here..."
          bind:value={importJson}
        ></textarea>

        {#if importError}
          <div class="text-destructive bg-destructive/10 rounded-md p-3 text-sm">
            {importError}
          </div>
        {/if}
      </div>
      <div class="border-border flex justify-end gap-2 border-t p-4">
        <Button variant="outline" onclick={onClose}>Cancel</Button>
        <Button onclick={validateAndImport} disabled={!importJson.trim()}>
          <Upload class="mr-2" size={16} />
          Import Profile
        </Button>
      </div>
    {:else if mode === "delete" && profile}
      <div class="p-6">
        <p class="text-muted-foreground">
          Delete <strong class="text-foreground">{profile.name}</strong>? This cannot be undone.
        </p>
      </div>
      <div class="border-border flex justify-end gap-2 border-t p-4">
        <Button variant="outline" onclick={onClose}>Cancel</Button>
        <Button variant="destructive" onclick={onDelete}>Delete</Button>
      </div>
    {/if}
  </div>
</div>
