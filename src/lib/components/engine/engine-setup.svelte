<script lang="ts">
  // Save credentials before cloud tests, which read persisted configuration.

  import { onDestroy, untrack } from "svelte";
  import { toast } from "svelte-sonner";
  import { _ } from "svelte-i18n";
  import { invoke } from "@tauri-apps/api/core";
  import EnginePanel from "./engine-panel.svelte";
  import InstallDialog from "./install-dialog.svelte";
  import UvNotice from "./uv-notice.svelte";
  import { Button } from "#lib/components/ui/button/index.js";
  import { installStore } from "#lib/stores/install-store.svelte";
  import {
    ALL_SETUP_ENTRIES,
    LOCAL_ENGINES,
    type EngineSetupEntry,
    type TestState
  } from "./engine-meta";
  import type { AppConfig } from "#lib/types";

  let {
    localConfig = $bindable(),
    isDirty = false,
    onSave,
    envKeySources = {}
  }: {
    localConfig: AppConfig;
    isDirty: boolean;
    onSave: () => Promise<void>;
    /** engine → winning env var for keys currently provided by .env. */
    envKeySources?: Record<string, string>;
  } = $props();

  let filter = $state<"all" | "cloud" | "local">("all");
  const filters = ["all", "local", "cloud"] as const;
  const visibleEntries = $derived(
    ALL_SETUP_ENTRIES.filter((entry) => filter === "all" || entry.kind === filter)
  );
  let activeTest = $state<string | null>(null);
  const testing = $derived(activeTest !== null);
  const installing = $derived(Object.values(installStore.runs).some((run) => run.running));

  let testStates = $state<Record<string, TestState>>({});
  let testMessages = $state<Record<string, string>>({});
  const testRevisions: Record<string, number> = {};
  let destroyed = false;
  onDestroy(() => {
    destroyed = true;
  });

  function clearTest(id: string) {
    testRevisions[id] = (testRevisions[id] ?? 0) + 1;
    testStates = { ...testStates, [id]: "idle" };
    testMessages = { ...testMessages, [id]: "" };
  }

  // Observe replacements too, including Cancel from the shared save bar.
  for (const entry of ALL_SETUP_ENTRIES) {
    const target = entry.credentialTarget;
    if (!target) continue;
    $effect(() => {
      void localConfig.tts[target].api_key;
      if (target === "microsoft") void localConfig.tts.microsoft.endpoint;
      void envKeySources[target];
      untrack(() => clearTest(entry.id));
    });
  }
  // Bumped whenever an install/uninstall settles so the panel's runtime
  // status chip (installed / CPU / GPU) re-probes.
  let runtimeProbeKey = $state(0);
  // Non-null opens the streamed install/uninstall dialog for that engine.
  let installDialogEntry = $state<EngineSetupEntry | null>(null);

  for (const entry of LOCAL_ENGINES) {
    $effect(() => {
      void installStore.isRunning(entry.installerId ?? entry.id);
      untrack(() => clearTest(entry.id));
    });
  }

  function testState(id: string): TestState {
    return activeTest === id ? "testing" : (testStates[id] ?? "idle");
  }

  async function runTest(entry: EngineSetupEntry) {
    if (testing || installing || installDialogEntry) return;
    activeTest = entry.id;
    clearTest(entry.id);
    const revision = testRevisions[entry.id];
    const isCurrent = () => !destroyed && testRevisions[entry.id] === revision;
    try {
      if (entry.kind === "cloud" && isDirty) {
        try {
          await onSave();
        } catch {
          if (isCurrent()) {
            clearTest(entry.id);
            toast.error($_("engines.saveBeforeTestFailed"));
          }
          return;
        }
      }
      if (!isCurrent()) return;
      const result = await invoke<{ success: boolean; message: string }>(
        entry.kind === "local" ? "test_local_engine" : "test_tts_engine_config",
        { engine: entry.installerId ?? entry.id }
      );
      if (!isCurrent()) return;
      testStates = { ...testStates, [entry.id]: result.success ? "success" : "fail" };
      testMessages = { ...testMessages, [entry.id]: result.message };
    } catch (e) {
      if (!isCurrent()) return;
      testStates = { ...testStates, [entry.id]: "fail" };
      testMessages = { ...testMessages, [entry.id]: String(e) };
    } finally {
      activeTest = null;
    }
  }

  // Every installable engine goes through the dialog: it owns the streamed
  // log, the voice picker, and uninstall. Nothing installs fire-and-forget,
  // so a failure can no longer look like a success.
  function runInstall(entry: EngineSetupEntry) {
    if (!entry.installerId || testing || (installing && !installStore.isRunning(entry.installerId)))
      return;
    installDialogEntry = entry;
  }
</script>

<section class="min-w-0 space-y-5 pb-20" aria-label={$_("engines.dashboard.title")}>
  <header>
    <div class="space-y-1.5">
      <h1 class="text-2xl font-semibold tracking-tight">{$_("engines.dashboard.title")}</h1>
      <p class="text-muted-foreground max-w-2xl text-sm">{$_("engines.dashboard.description")}</p>
    </div>
  </header>

  <UvNotice />

  <div class="engine-table-wrapper settings-card bg-card rounded-lg border">
    <div class="flex flex-wrap items-center justify-between gap-3 pb-3">
      <div
        class="inline-flex flex-wrap gap-1"
        role="group"
        aria-label={$_("engines.dashboard.filter")}
      >
        {#each filters as kind (kind)}
          <Button
            variant={filter === kind ? "default" : "ghost"}
            size="sm"
            aria-pressed={filter === kind}
            onclick={() => (filter = kind)}
          >
            {kind === "all" ? $_("engines.dashboard.all") : $_(`engine.kind.${kind}`)}
          </Button>
        {/each}
      </div>
      <p class="text-muted-foreground text-xs">{$_("engines.dashboard.profileHint")}</p>
    </div>
    <div class="border-border rounded-md border">
      <table
        aria-label={$_("engines.dashboard.title")}
        class="engine-table w-full table-fixed border-collapse text-start text-sm"
      >
        <thead class="bg-muted/40 text-muted-foreground text-xs">
          <tr>
            <th scope="col" class="w-[23%] px-4 py-3 text-start font-medium"
              >{$_("engines.dashboard.engine")}</th
            >
            <th scope="col" class="w-[35%] px-4 py-3 text-start font-medium"
              >{$_("engines.dashboard.configuration")}</th
            >
            <th scope="col" class="px-4 py-3 text-start font-medium"
              >{$_("engines.dashboard.status")}</th
            >
            <th scope="col" class="px-4 py-3 text-end font-medium"
              >{$_("engines.dashboard.actions")}</th
            >
          </tr>
        </thead>
        <tbody>
          {#each visibleEntries as entry (entry.id)}
            <EnginePanel
              bind:localConfig
              {entry}
              testState={testState(entry.id)}
              testMessage={testMessages[entry.id] ?? ""}
              testDisabled={testing || installing || installDialogEntry !== null}
              onTest={() => runTest(entry)}
              onInstall={() => runInstall(entry)}
              onCredentialChange={() => clearTest(entry.id)}
              {runtimeProbeKey}
              {envKeySources}
            />
          {/each}
        </tbody>
      </table>
    </div>
  </div>

  {#if installDialogEntry}
    <InstallDialog
      entry={installDialogEntry}
      onclose={() => (installDialogEntry = null)}
      onchange={() => {
        // A settled install flips the runtime chip and stale test verdicts
        // would be misleading after the change.
        for (const entry of LOCAL_ENGINES) clearTest(entry.id);
        runtimeProbeKey += 1;
      }}
    />
  {/if}
</section>

<style>
  .engine-table-wrapper {
    container-type: inline-size;
  }

  @container (max-width: 41.999rem) {
    .engine-table,
    .engine-table :global(tbody) {
      display: block;
    }

    .engine-table thead {
      position: absolute;
      width: 1px;
      height: 1px;
      overflow: hidden;
      clip-path: inset(50%);
    }

    .engine-table :global(tbody tr) {
      display: grid;
      grid-template-columns: minmax(0, 1fr);
      gap: 0.75rem;
      padding: 1rem;
    }

    .engine-table :global(tbody tr:first-child) {
      border-top: 0;
    }

    .engine-table :global(tbody th),
    .engine-table :global(tbody td) {
      min-width: 0;
      padding: 0;
      text-align: start;
    }

    .engine-table :global(.engine-actions) {
      justify-content: flex-start;
    }
  }
</style>
