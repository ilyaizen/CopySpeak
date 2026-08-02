<script lang="ts">
  // Engine install/uninstall dialog: voice selection + streamed progress log.
  //
  // Voice presentation is driven by the entry's `voiceMode` (see engine-meta):
  // per-voice engines download each checked voice, shared-model engines install
  // one model covering every voice, and `none` engines are a plain binary
  // install. All progress state lives in installStore (persists while
  // dismissed); this component is a view over it.

  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";
  import { toast } from "svelte-sonner";
  import { Download, X, Loader2, Check, AlertCircle, RotateCw, Trash2 } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { portal } from "$lib/utils";
  import { installStore, type VoiceStatus } from "$lib/stores/install-store.svelte";
  import type { EngineSetupEntry } from "./engine-meta";
  import type { VoiceCatalogEntry, EngineCatalogEntry } from "$lib/types";

  let {
    entry,
    onclose,
    onchange
  }: {
    entry: EngineSetupEntry;
    onclose: () => void;
    /** Fired after an install/uninstall settles so the parent can re-probe. */
    onchange?: () => void;
  } = $props();

  interface EngineStatus {
    installed: boolean;
    voices: string[];
  }

  const engineId = $derived(entry.installerId ?? entry.id);
  const mode = $derived(entry.voiceMode ?? "none");

  let voices = $state<VoiceCatalogEntry[]>([]);
  let installed = $state<Set<string>>(new Set());
  let isInstalled = $state(false);
  let selected = $state<Set<string>>(new Set());
  let logRef = $state<HTMLPreElement | null>(null);
  let toasted = $state(false);
  let confirmingUninstall = $state(false);
  // Distinguishes the two operations sharing the install-progress channel, so
  // the completion toast says the right thing.
  let lastAction = $state<"install" | "uninstall">("install");

  const run = $derived(installStore.run(engineId));
  const busy = $derived(run?.running ?? false);
  const finished = $derived(run ? !run.running : false);

  async function load() {
    if (mode !== "none") {
      try {
        const catalog = await invoke<EngineCatalogEntry[]>("list_tts_engines");
        voices = catalog.find((e) => e.engine === engineId)?.voices ?? [];
      } catch {
        voices = [];
      }
    }
    try {
      const status = await invoke<EngineStatus>("engine_status", { engine: engineId });
      isInstalled = status.installed;
      installed = new Set(status.voices);
    } catch {
      isInstalled = false;
      installed = new Set();
    }
    selected = new Set();
  }

  // Refresh status + toast once when a run settles.
  $effect(() => {
    const f = run ? !run.running : false;
    if (f && !toasted) {
      toasted = true;
      void load();
      onchange?.();
      const ok = run?.exitCode === 0;
      const done =
        lastAction === "uninstall" ? $_("engines.uninstallDone") : $_("engines.installDone");
      const failed =
        lastAction === "uninstall" ? $_("engines.uninstallFailed") : $_("engines.installFailed");
      if (ok) toast.success(`${engineId}: ${done}`);
      else toast.error(`${failed}: ${engineId}`);
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
    // `none` engines have nothing to select; shared engines install every
    // voice at once; per-voice engines install exactly what is checked.
    const ids = mode === "none" ? [] : mode === "shared" ? voices.map((v) => v.id) : [...selected];
    if (mode === "per-voice" && ids.length === 0) return;
    lastAction = "install";
    toasted = false;
    confirmingUninstall = false;
    await installStore.start(engineId, ids);
    try {
      await invoke("install_engine", { engine: engineId, voice: ids.length > 0 ? ids : null });
    } catch (e) {
      toast.error(`${$_("engines.installFailed")}: ${e}`);
    }
  }

  async function startUninstall() {
    lastAction = "uninstall";
    toasted = false;
    confirmingUninstall = false;
    await installStore.start(engineId, []);
    try {
      await invoke("uninstall_engine", { engine: engineId });
    } catch (e) {
      toast.error(`${$_("engines.uninstallFailed")}: ${e}`);
    }
  }

  async function retry(id: string) {
    lastAction = "install";
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
        <p class="text-muted-foreground text-sm">{entry.downloadSize ?? ""}</p>
      </div>
      <Button variant="ghost" size="sm" onclick={onclose}><X size={16} /></Button>
    </div>

    <div class="flex-1 space-y-3 overflow-y-auto p-4">
      <p class="text-muted-foreground text-sm">
        {#if mode === "none"}
          {isInstalled ? $_("engines.binaryInstalled") : $_("engines.binaryHint")}
        {:else if mode === "shared"}
          {isInstalled ? $_("engines.installedAllVoices") : $_("engines.sharedModelHint")}
        {:else}
          {$_("engines.piperPerVoiceHint")}
        {/if}
      </p>

      {#if mode !== "none"}
        <ul class="space-y-1">
          {#each voices as v (v.id)}
            {@const st = statusOf(v.id)}
            <li class="border-border flex items-center gap-2 rounded border px-2 py-1.5 text-sm">
              {#if mode === "per-voice"}
                <input
                  type="checkbox"
                  class="h-4 w-4 shrink-0"
                  disabled={st === "installed" || busy}
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
                  class="inline-flex items-center gap-1 text-xs text-emerald-600 dark:text-emerald-400"
                >
                  <Check size={14} />
                  {$_("engines.voiceInstalled")}
                </span>
              {:else if st === "done"}
                <Check size={14} class="text-emerald-500" />
              {:else if st === "installing" || (st === "pending" && busy)}
                <Loader2 size={14} class="text-muted-foreground animate-spin" />
              {:else if st === "failed"}
                <span class="text-destructive inline-flex items-center gap-1 text-xs">
                  <AlertCircle size={14} />
                  {#if mode === "per-voice"}
                    <Button variant="ghost" size="sm" onclick={() => retry(v.id)} disabled={busy}>
                      <RotateCw size={12} class="mr-1" />{$_("engines.retry")}
                    </Button>
                  {/if}
                </span>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}

      {#if confirmingUninstall}
        <div class="rounded-md border border-amber-500/30 bg-amber-500/10 p-3">
          <p class="text-sm text-amber-700 dark:text-amber-400">
            {$_("engines.uninstallConfirm")}
          </p>
          <div class="mt-2 flex gap-2">
            <Button variant="destructive" size="sm" onclick={startUninstall}>
              {$_("engines.uninstall")}
            </Button>
            <Button variant="outline" size="sm" onclick={() => (confirmingUninstall = false)}>
              {$_("engine.saveBar.cancel")}
            </Button>
          </div>
        </div>
      {/if}

      {#if run && run.lines.length > 0}
        <pre
          bind:this={logRef}
          class="bg-muted/50 border-border h-40 overflow-auto rounded-md border p-2 font-mono text-[11px] leading-tight whitespace-pre-wrap">{run.lines.join(
            "\n"
          )}</pre>
      {/if}
    </div>

    <div class="border-border flex items-center justify-between gap-2 border-t p-4">
      <span class="text-muted-foreground text-xs">
        {#if busy}
          <Loader2 size={12} class="mr-1 inline animate-spin" />{$_("engine.setup.installing")}
        {:else if finished && run?.exitCode === 0}
          <Check size={12} class="mr-1 inline text-emerald-500" />{$_("engines.installDone")}
        {:else if finished}
          <AlertCircle size={12} class="text-destructive mr-1 inline" />{$_(
            "engines.installFailed"
          )}
        {/if}
      </span>
      <div class="flex gap-2">
        <!-- uv is a shared prerequisite: removing it would break every other
             local engine, so it is install-only. -->
        {#if isInstalled && engineId !== "uv"}
          <Button
            variant="outline"
            disabled={busy}
            onclick={() => (confirmingUninstall = true)}
            class="text-destructive"
          >
            <Trash2 size={14} class="mr-2" />
            {$_("engines.uninstall")}
          </Button>
        {/if}
        <Button variant="outline" onclick={onclose}>{$_("common.close")}</Button>
        <Button
          onclick={startInstall}
          disabled={busy || (mode === "per-voice" && selected.size === 0)}
        >
          {#if busy}
            <Loader2 size={14} class="mr-2 animate-spin" />
          {:else}
            <Download size={14} class="mr-2" />
          {/if}
          {isInstalled && mode !== "per-voice"
            ? $_("engines.reinstall")
            : $_("engine.setup.install")}
        </Button>
      </div>
    </div>
  </div>
</div>
