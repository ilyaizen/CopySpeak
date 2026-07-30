<script lang="ts">
  // Engine install dialog: voice selection + streamed progress log.
  //
  // Per-voice engines (piper) download each checked voice; shared-model engines
  // (kitten/kokoro) install one model and include every voice. All progress
  // state lives in installStore (persists while dismissed); this component is a
  // view over it. Mounted fresh on each open by engine-setup.

  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";
  import { toast } from "svelte-sonner";
  import { Download, X, Loader2, Check, AlertCircle, RotateCw } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { portal } from "$lib/utils";
  import { installStore, type VoiceStatus } from "$lib/stores/install-store.svelte";
  import type { VoiceCatalogEntry, EngineCatalogEntry } from "$lib/types";

  let {
    engineId,
    onclose
  }: {
    engineId: string;
    onclose: () => void;
  } = $props();

  // Shared-model engines install one download; piper is per-voice.
  const SHARED = new Set(["kitten", "kokoro"]);
  const ENGINE_SIZE: Record<string, string> = {
    kitten: "~25 MB shared model",
    kokoro: "~335 MB shared model",
    piper: "~20-100 MB / voice"
  };

  let voices = $state<VoiceCatalogEntry[]>([]);
  let installed = $state<Set<string>>(new Set());
  let selected = $state<Set<string>>(new Set());
  let logRef = $state<HTMLPreElement | null>(null);
  let toasted = $state(false);

  const shared = $derived(SHARED.has(engineId));
  const run = $derived(installStore.run(engineId));
  const finished = $derived(run ? !run.running : false);
  const isInstalled = $derived(shared && installed.size > 0);

  async function load() {
    try {
      const catalog = await invoke<EngineCatalogEntry[]>("list_tts_engines");
      voices = catalog.find((e) => e.engine === engineId)?.voices ?? [];
    } catch {
      voices = [];
    }
    try {
      installed = new Set(await invoke<string[]>("installed_voices", { engine: engineId }));
    } catch {
      installed = new Set();
    }
    selected = new Set();
  }

  // Refresh the installed set + toast once when a run settles.
  $effect(() => {
    const f = run ? !run.running : false;
    if (f && !toasted) {
      toasted = true;
      void load();
      if (run && run.exitCode === 0) toast.success(`${engineId}: ${$_("engines.installDone")}`);
      else toast.error(`${$_("engines.installFailed")}: ${engineId}`);
    }
  });

  // Autoscroll the log on new output.
  $effect(() => {
    void run?.lines.length;
    if (logRef) logRef.scrollTop = logRef.scrollHeight;
  });

  function toggle(id: string) {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selected = next;
  }

  function statusOf(id: string): VoiceStatus | "installed" {
    if (installed.has(id)) return "installed";
    return run?.voiceStatus[id] ?? "pending";
  }

  async function startInstall() {
    const ids = shared ? voices.map((v) => v.id) : [...selected];
    if (ids.length === 0) return;
    toasted = false;
    await installStore.start(engineId, ids);
    try {
      await invoke("install_engine", { engine: engineId, voice: ids });
    } catch (e) {
      toast.error(`${$_("engines.installFailed")}: ${e}`);
    }
  }

  async function retry(id: string) {
    toasted = false;
    installStore.markVoice(engineId, id, "pending");
    try {
      await invoke("install_engine", { engine: engineId, voice: [id] });
    } catch (e) {
      toast.error(`${$_("engines.installFailed")}: ${e}`);
    }
  }

  onMount(() => {
    // Suppress a spurious completion toast if the run was already finished
    // before this open (the store persists across opens).
    const r = installStore.run(engineId);
    if (r && !r.running) toasted = true;
    void load();
  });
</script>

<div use:portal class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
  <div class="bg-card border-border flex max-h-[90vh] w-full max-w-xl flex-col rounded-lg border">
    <div class="border-border flex items-center justify-between border-b p-4">
      <div>
        <h3 class="text-lg font-semibold">
          <span class="capitalize">{engineId}</span> — {$_("engine.setup.install")}
        </h3>
        <p class="text-muted-foreground text-sm">{ENGINE_SIZE[engineId] ?? ""}</p>
      </div>
      <Button variant="ghost" size="sm" onclick={onclose}><X size={16} /></Button>
    </div>

    <div class="flex-1 space-y-3 overflow-y-auto p-4">
      <p class="text-muted-foreground text-sm">
        {#if shared}
          {isInstalled ? $_("engines.installedAllVoices") : $_("engines.sharedModelHint")}
        {:else}
          {$_("engines.piperPerVoiceHint")}
        {/if}
      </p>

      <ul class="space-y-1">
        {#each voices as v (v.id)}
          {@const st = statusOf(v.id)}
          <li class="border-border flex items-center gap-2 rounded border px-2 py-1.5 text-sm">
            {#if !shared}
              <input
                type="checkbox"
                class="h-4 w-4 shrink-0"
                disabled={st === "installed" || run?.running}
                checked={selected.has(v.id)}
                onchange={() => toggle(v.id)}
              />
            {/if}
            <span class="min-w-0 flex-1">
              <span class="font-medium">{v.label}</span>
              <span class="text-muted-foreground ml-1 text-xs">{v.gender ?? ""}</span>
            </span>

            {#if st === "installed"}
              <span
                class="text-xs text-emerald-600 dark:text-emerald-400 inline-flex items-center gap-1"
              >
                <Check size={14} /> {$_("engines.voiceInstalled")}
              </span>
            {:else if st === "done"}
              <Check size={14} class="text-emerald-500" />
            {:else if st === "installing" || (st === "pending" && run?.running)}
              <Loader2 size={14} class="text-muted-foreground animate-spin" />
            {:else if st === "failed"}
              <span class="text-destructive inline-flex items-center gap-1 text-xs">
                <AlertCircle size={14} />
                {#if !shared}
                  <Button variant="ghost" size="sm" onclick={() => retry(v.id)} disabled={run?.running}>
                    <RotateCw size={12} class="mr-1" />{$_("engines.retry")}
                  </Button>
                {/if}
              </span>
            {/if}
          </li>
        {/each}
      </ul>

      {#if run && run.lines.length > 0}
        <pre
          bind:this={logRef}
          class="bg-muted/50 border-border h-40 overflow-auto rounded-md border p-2 font-mono text-[11px] leading-tight whitespace-pre-wrap"
        >{run.lines.join("\n")}</pre>
      {/if}
    </div>

    <div class="border-border flex items-center justify-between gap-2 border-t p-4">
      <span class="text-muted-foreground text-xs">
        {#if run?.running}
          <Loader2 size={12} class="mr-1 inline animate-spin" />{$_("engine.setup.installing")}
        {:else if finished && run?.exitCode === 0}
          <Check size={12} class="text-emerald-500 mr-1 inline" />{$_("engines.installDone")}
        {:else if finished}
          <AlertCircle size={12} class="text-destructive mr-1 inline" />{$_("engines.installFailed")}
        {/if}
      </span>
      <div class="flex gap-2">
        <Button variant="outline" onclick={onclose}>{$_("common.close")}</Button>
        <Button
          onclick={startInstall}
          disabled={run?.running || (!shared && selected.size === 0)}
        >
          {#if run?.running}
            <Loader2 size={14} class="mr-2 animate-spin" />
          {:else}
            <Download size={14} class="mr-2" />
          {/if}
          {shared && isInstalled ? $_("engines.reinstall") : $_("engine.setup.install")}
        </Button>
      </div>
    </div>
  </div>
</div>
