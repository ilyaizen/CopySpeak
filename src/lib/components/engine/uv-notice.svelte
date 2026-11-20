<script lang="ts">
  // uv notice for the Engines page. Shows only while uv is missing, installing,
  // or failed to install. Ready and unknown render nothing.
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";
  import { invoke } from "@tauri-apps/api/core";
  import { AlertCircle, Loader2 } from "@lucide/svelte";
  import { Button } from "#lib/components/ui/button/index.js";
  import { installStore } from "#lib/stores/install-store.svelte";

  type View = "installing" | "failed" | "missing" | "hidden";

  // null means unknown: the probe has not run yet, or it rejected.
  let uvFound = $state<boolean | null>(null);

  async function probe() {
    try {
      const status = await invoke<{ installed: boolean }>("engine_status", { engine: "uv" });
      uvFound = status.installed;
    } catch {
      uvFound = null;
    }
  }

  const run = $derived(installStore.run("uv"));
  const uvRunning = $derived(installStore.isRunning("uv"));
  const anyRunning = $derived(Object.values(installStore.runs).some((r) => r.running));
  const lastLine = $derived(run?.lines.at(-1) ?? "");

  // Re-check the disk when a uv run ends, so the notice clears without a reload.
  let wasRunning = false;
  $effect(() => {
    if (wasRunning && !uvRunning) void probe();
    wasRunning = uvRunning;
  });

  onMount(() => {
    void probe();
  });

  function viewState(): View {
    if (uvRunning) return "installing";
    if (uvFound !== false) return "hidden";
    return run ? "failed" : "missing";
  }
  const view = $derived(viewState());

  async function install() {
    if (!(await installStore.start("uv", []))) return;
    try {
      await invoke("install_engine", { engine: "uv", voice: [], cuda: false });
    } catch (e) {
      installStore.fail("uv", "[ERROR] install did not start: " + String(e));
    }
  }
</script>

{#if view !== "hidden"}
  <div
    role="status"
    class="border-border bg-muted/30 flex items-start justify-between gap-3 rounded-lg border p-3"
  >
    <div class="flex min-w-0 items-start gap-2">
      {#if view === "installing"}
        <Loader2 size={16} class="text-muted-foreground mt-0.5 shrink-0 animate-spin" />
        <div class="min-w-0">
          <p class="text-sm font-medium">{$_("onboarding.installingUv")}</p>
          {#if lastLine}
            <p class="text-muted-foreground mt-1 truncate font-mono text-[11px]" title={lastLine}>
              {lastLine}
            </p>
          {/if}
        </div>
      {:else}
        <AlertCircle
          size={16}
          class="{view === 'failed' ? 'text-destructive' : 'text-muted-foreground'} mt-0.5 shrink-0"
        />
        <div class="min-w-0">
          {#if view === "failed"}
            <p class="text-destructive text-sm font-medium">{$_("onboarding.installFailed")}</p>
            <p class="text-muted-foreground text-xs">
              {run?.exitCode === -1
                ? $_("engines.uv.failedStart")
                : $_("engines.uv.failedNotFound")}
            </p>
          {:else}
            <p class="text-sm font-medium">{$_("engines.uv.title")}</p>
            <p class="text-muted-foreground text-xs">{$_("onboarding.uvRequired")}</p>
          {/if}
        </div>
      {/if}
    </div>
    {#if view !== "installing"}
      <Button variant="outline" size="sm" disabled={anyRunning} onclick={install}>
        {view === "failed" ? $_("engines.retry") : $_("onboarding.installUv")}
      </Button>
    {/if}
  </div>
{/if}
