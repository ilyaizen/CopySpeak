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
  import {
    CLOUD_ENGINES,
    LOCAL_PRESETS,
    UV_ENTRY,
    type EngineSetupEntry,
    type TestState,
    type InstallState
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

  const ALL = [...CLOUD_ENGINES, ...LOCAL_PRESETS, UV_ENTRY];

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
  let installStates = $state<Record<string, InstallState>>({});
  let uvAvailable = $state<boolean | null>(null);

  const selected = $derived(ALL.find((e) => e.id === selectedId) ?? CLOUD_ENGINES[0]);

  function testState(id: string): TestState {
    return testStates[id] ?? "idle";
  }
  function installState(id: string): InstallState {
    return installStates[id] ?? "idle";
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

  // Real-synthesis test for a uv-installed local engine. The preset id (piper,
  // kokoro, kitten, chatterbox) maps to a stable CLI spec in Rust.
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

  async function runInstall(entry: EngineSetupEntry) {
    if (!entry.installerId) return;
    installStates = { ...installStates, [entry.id]: "installing" };
    try {
      await invoke("install_engine", { engine: entry.installerId });
      if (entry.id === "uv") await checkUv();
      toast.success(`${$_("engines.installerLaunched")}: ${entry.id}`);
    } catch (e) {
      toast.error(`${$_("engines.installFailed")}: ${e}`);
    } finally {
      installStates = { ...installStates, [entry.id]: "idle" };
    }
  }

  onMount(() => {
    checkUv();
  });
</script>

<div class="flex flex-row items-start gap-2">
  <aside class="w-36 shrink-0 self-stretch">
    <nav class="space-y-0.5">
      <p
        class="text-muted-foreground px-2 pt-1 pb-1 text-[11px] font-semibold tracking-wide uppercase"
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
        class="text-muted-foreground px-2 pt-3 pb-1 text-[11px] font-semibold tracking-wide uppercase"
      >
        {$_("engines.local")}
      </p>
      {#each LOCAL_PRESETS as entry (entry.id)}
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
      installState={installState(selected.id)}
      onInstall={() => runInstall(selected)}
    />
  </main>
</div>
