<script lang="ts">
  import type { AppConfig } from "$lib/types";
  import { invoke } from "@tauri-apps/api/core";
  import { Alert, AlertTitle, AlertDescription } from "$lib/components/ui/alert/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { CheckCircle, XCircle, Terminal, ChevronDown, ChevronRight } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

  let isTesting = $state(false);
  let testResult = $state<{
    success: boolean;
    message: string;
    error_type?: string;
  } | null>(null);
  let dataDir = $state<string | null>(null);
  let homeDir = $state<string | null>(null);
  let previewExpanded = $state(false);

  // Resolved CLI command preview using actual data dir path
  const cliPreview = $derived.by(() => {
    const dir = dataDir ?? "{data_dir}";
    const home = homeDir ?? "{home_dir}";
    const voice = localConfig.tts.voice ?? "";
    const inputPath = "%TEMP%\\copyspeak_tts_input.txt";
    const outputPath = "%TEMP%\\copyspeak_tts_out.wav";

    const resolvedArgs = localConfig.tts.args_template.map((arg) =>
      arg
        .replace("{input}", inputPath)
        .replace("{text}", inputPath)
        .replace("{raw_text}", "<text content>")
        .replace("{output}", outputPath)
        .replace("{voice}", voice)
        .replace("{speed}", "1.0")
        .replace("{length_scale}", "1.000")
        .replace("{data_dir}", dir)
        .replace("{home_dir}", home)
    );

    // Quote args that contain spaces
    const quotedArgs = resolvedArgs.map((a) => (a.includes(" ") ? `"${a}"` : a));
    return [localConfig.tts.command, ...quotedArgs].join(" ");
  });

  async function testEngine() {
    isTesting = true;
    testResult = null;
    try {
      const result = (await invoke("test_tts_engine_config", {
        engine: "local",
        preset: localConfig.tts.preset
      })) as {
        success: boolean;
        message: string;
        error_type?: string;
      };
      testResult = result;
    } catch (e) {
      testResult = { success: false, message: String(e) };
    } finally {
      isTesting = false;
    }
  }

  onMount(async () => {
    try {
      dataDir = await invoke<string>("get_data_dir");
      homeDir = await invoke<string>("get_home_dir");
    } catch {
      dataDir = "%USERPROFILE%\\piper-voices";
      homeDir = "%USERPROFILE%";
    }
  });
</script>

<div class="space-y-4">
  <!-- CLI Command Preview -->
  <div class="border-border rounded-md border">
    <button
      type="button"
      class="text-muted-foreground hover:text-foreground flex w-full items-center gap-2 px-3 py-2 text-xs transition-colors"
      onclick={() => (previewExpanded = !previewExpanded)}
    >
      <Terminal size={13} class="shrink-0" />
      <span class="font-medium">{$_("engine.localEngine.commandPreview")}</span>
      <span class="ml-auto">
        {#if previewExpanded}
          <ChevronDown size={13} />
        {:else}
          <ChevronRight size={13} />
        {/if}
      </span>
    </button>
    {#if previewExpanded}
      <div class="border-border border-t px-3 py-2.5">
        <pre
          class="bg-muted/50 text-foreground overflow-x-auto rounded px-3 py-2 font-mono text-[11px] leading-relaxed break-all whitespace-pre-wrap">{cliPreview}</pre>
        <p class="text-muted-foreground mt-1.5 text-[10px]">
          <code>%TEMP%</code>
          {$_("engine.localEngine.tempDirectory")}
        </p>
      </div>
    {/if}
  </div>

  <div class="border-border border-t pt-4">
      <Button variant="outline" size="sm" onclick={testEngine} disabled={isTesting}>
        {#if isTesting}
          {$_("engine.localEngine.testing")}
        {:else}
          {$_("engine.localEngine.testEngine")}
        {/if}
      </Button>

      {#if testResult}
        <Alert variant={testResult.success ? "default" : "destructive"} class="mt-3">
          {#if testResult.success}
            <CheckCircle class="text-emerald-600" />
          {:else}
            <XCircle />
          {/if}
          <AlertTitle>
            {testResult.success
              ? $_("engine.localEngine.engineWorking")
              : $_("engine.localEngine.engineFailed")}
          </AlertTitle>
          <AlertDescription>
            {testResult.message}
          </AlertDescription>
        </Alert>
      {/if}
  </div>
</div>
