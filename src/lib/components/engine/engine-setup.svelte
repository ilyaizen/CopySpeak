<script lang="ts">
  // Engine setup orchestrator: sidebar (cloud + local groups) + panel.
  // Owns test/install state machines and IPC. Ensures credentials are saved
  // before a test, since test_tts_engine_config reads from persisted state.

  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { toast } from "svelte-sonner";
  import { _ } from "svelte-i18n";
  import { Button } from "$lib/components/ui/button/index.js";
  import EnginePanel from "./engine-panel.svelte";
  import InstallDialog from "./install-dialog.svelte";
  import {
    CLOUD_ENGINES,
    LOCAL_ENGINES,
    UV_ENTRY,
    type EngineSetupEntry,
    type TestState
  } from "./engine-meta";
  import type { AppConfig } from "$lib/types";

  let {
    localConfig = $bindable(),
    isDirty = false,
    onSave
  }: {
    localConfig: AppConfig;
    isDirty: boolean;
    onSave: () => Promise<void>;
  } = $props();

  const ALL = [...CLOUD_ENGINES, ...LOCAL_ENGINES, UV_ENTRY];

  // Default to the active profile's cloud engine when possible, else first cloud.
  const initialId = (() => {
    const active = localConfig.tts.profiles.find((p) => p.id === localConfig.tts.active_profile_id);
    if (active && CLOUD_ENGINES.some((e) => e.id === active.engine)) {
      return active.engine as string;
    }
    return CLOUD_ENGINES[0].id;
  })();
  let selectedId = $state<string>(initialId);

  let testStates = $state<Record<string, TestState>>({});
  let testMessages = $state<Record<string, string>>({});
  let uvAvailable = $state<boolean | null>(null);
  // Non-null opens the streamed install/uninstall dialog for that engine.
  let installDialogEntry = $state<EngineSetupEntry | null>(null);

  const selected = $derived(ALL.find((e) => e.id === selectedId) ?? CLOUD_ENGINES[0]);

  function testState(id: string): TestState {
    return testStates[id] ?? "idle";
  }

  async function checkUv() {
    try {
      const r = await invoke<{ available: boolean }>("check_command_exists", { command: "uv" });
      uvAvailable = r.available;
    } catch {
      uvAvailable = null;
    }
  }

  async function runTest(entry: EngineSetupEntry) {
    // Local engines are tested by real synthesis (test_local_engine); they
    // have no credentials, so skip the save-before-test flush. Cloud tests
    // read from persisted state, so unsaved creds must be flushed first.
    if (entry.kind === "local") {
      await runLocalTest(entry);
      return;
    }
    if (isDirty) {
      try {
        await onSave();
      } catch {
        toast.error($_("engines.saveBeforeTestFailed"));
        return;
      }
    }
    testStates = { ...testStates, [entry.id]: "testing" };
    try {
      const result = await invoke<{ success: boolean; message: string }>("test_tts_engine_config", {
        engine: entry.id
      });
      testStates = { ...testStates, [entry.id]: result.success ? "success" : "fail" };
      testMessages = { ...testMessages, [entry.id]: result.message };
      if (result.success) toast.success($_("engine.apiSetup.testPassed"));
    } catch (e) {
      testStates = { ...testStates, [entry.id]: "fail" };
      testMessages = { ...testMessages, [entry.id]: String(e) };
      toast.error(`${$_("engine.apiSetup.testFailed")}: ${e}`);
    }
  }

  // Real-synthesis test for a uv-installed local engine. The engine id (piper,
  // kokoro, kitten) maps to a stable CLI spec in Rust.
  async function runLocalTest(entry: EngineSetupEntry) {
    testStates = { ...testStates, [entry.id]: "testing" };
    try {
      const result = await invoke<{ success: boolean; message: string }>("test_local_engine", {
        engine: entry.installerId ?? entry.id
      });
      testStates = { ...testStates, [entry.id]: result.success ? "success" : "fail" };
      testMessages = { ...testMessages, [entry.id]: result.message };
      if (result.success) toast.success($_("engine.localEngine.engineWorking"));
      else toast.error($_("engine.localEngine.engineFailed"));
    } catch (e) {
      testStates = { ...testStates, [entry.id]: "fail" };
      testMessages = { ...testMessages, [entry.id]: String(e) };
      toast.error(`${$_("engine.localEngine.engineFailed")}: ${e}`);
    }
  }

  // Every installable engine goes through the dialog: it owns the streamed
  // log, the voice picker, and uninstall. Nothing installs fire-and-forget,
  // so a failure can no longer look like a success.
  function runInstall(entry: EngineSetupEntry) {
    if (!entry.installerId) return;
    installDialogEntry = entry;
  }

  onMount(() => {
    checkUv();
  });
</script>

<div class="flex min-w-0 flex-col items-stretch gap-4 sm:flex-row sm:items-start sm:gap-5">
  <aside class="min-w-0 shrink-0 self-stretch sm:w-36">
    <nav class="grid grid-cols-2 gap-0.5 sm:block sm:space-y-0.5">
      <p
        class="text-muted-foreground col-span-2 px-2 pt-1 pb-1 text-[11px] font-semibold tracking-wide uppercase sm:block"
      >
        {$_("engines.cloud")}
      </p>
      {#each CLOUD_ENGINES as entry (entry.id)}
        <button
          class="block w-full rounded-md px-2 py-1.5 text-left text-sm transition-colors {selectedId ===
          entry.id
            ? 'border-primary bg-primary/10 text-primary border-l-2 font-medium'
            : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
          onclick={() => (selectedId = entry.id)}
        >
          {$_(`engine.${entry.id}.title`)}
        </button>
      {/each}
      <p
        class="text-muted-foreground col-span-2 px-2 pt-3 pb-1 text-[11px] font-semibold tracking-wide uppercase sm:block"
      >
        {$_("engines.local")}
      </p>
      {#each LOCAL_ENGINES as entry (entry.id)}
        <button
          class="block w-full rounded-md px-2 py-1.5 text-left text-sm transition-colors {selectedId ===
          entry.id
            ? 'border-primary bg-primary/10 text-primary border-l-2 font-medium'
            : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
          onclick={() => (selectedId = entry.id)}
        >
          {$_(`engine.${entry.id}.title`)}
        </button>
      {/each}
      <button
        class="block w-full rounded-md px-2 py-1.5 text-left text-sm transition-colors {selectedId ===
        UV_ENTRY.id
          ? 'border-primary bg-primary/10 text-primary border-l-2 font-medium'
          : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground'}"
        onclick={() => (selectedId = UV_ENTRY.id)}
      >
        {$_("engine.setup.installUv")}
      </button>
    </nav>
  </aside>

  <main class="min-w-0 flex-1 space-y-6 pb-20">
    {#if uvAvailable === false && selected.id !== "uv"}
      <div
        class="flex items-center justify-between gap-3 rounded-md border border-amber-500/30 bg-amber-500/10 p-3"
      >
        <p class="text-sm text-amber-700 dark:text-amber-400">{$_("engine.setup.uvMissing")}</p>
        <Button variant="outline" size="sm" onclick={() => (selectedId = UV_ENTRY.id)}>
          {$_("engine.setup.installUv")}
        </Button>
      </div>
    {/if}

    <EnginePanel
      bind:localConfig
      entry={selected}
      testState={testState(selected.id)}
      testMessage={testMessages[selected.id] ?? ""}
      onTest={() => runTest(selected)}
      onInstall={() => runInstall(selected)}
    />
  </main>

  {#if installDialogEntry}
    <InstallDialog
      entry={installDialogEntry}
      onclose={() => (installDialogEntry = null)}
      onchange={() => {
        // A uv install/uninstall flips the prerequisite banner; a stale test
        // verdict from before the change would be misleading.
        void checkUv();
        testStates = {};
      }}
    />
  {/if}
</div>
