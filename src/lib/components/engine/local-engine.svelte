<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import type { AppConfig } from "$lib/types";
  import { invoke } from "@tauri-apps/api/core";
  import { Alert, AlertTitle, AlertDescription } from "$lib/components/ui/alert/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { CheckCircle, XCircle, Terminal, ChevronDown, ChevronRight } from "@lucide/svelte";
  import { onMount } from "svelte";
  import { openExternal } from "$lib/utils/external-link";
  import { _ } from "svelte-i18n";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

  // KittenTTS voices - 8 built-in voices
  const KITTEN_VOICES = [
    { value: "Jasper", label: "Jasper (default)" },
    { value: "Bella", label: "Bella" },
    { value: "Luna", label: "Luna" },
    { value: "Bruno", label: "Bruno" },
    { value: "Rosie", label: "Rosie" },
    { value: "Hugo", label: "Hugo" },
    { value: "Kiki", label: "Kiki" },
    { value: "Leo", label: "Leo" }
  ];

  // EN US voices available in piper1-gpl (medium quality)
  const PIPER_EN_VOICES = [
    { value: "en_US-joe-medium", label: "Joe (default)" },
    { value: "en_US-amy-medium", label: "Amy" },
    { value: "en_US-arctic-medium", label: "Arctic" },
    { value: "en_US-bryce-medium", label: "Bryce" },
    { value: "en_US-danny-medium", label: "Danny" },
    { value: "en_US-hfc_female-medium", label: "HFC Female" },
    { value: "en_US-hfc_male-medium", label: "HFC Male" },
    { value: "en_US-john-medium", label: "John" },
    { value: "en_US-kathleen-medium", label: "Kathleen" },
    { value: "en_US-kristin-medium", label: "Kristin" },
    { value: "en_US-kusal-medium", label: "Kusal" },
    { value: "en_US-l2arctic-medium", label: "L2Arctic" },
    { value: "en_US-lessac-medium", label: "Lessac" },
    { value: "en_US-libritts-medium", label: "LibriTTS" },
    { value: "en_US-libritts_r-medium", label: "LibriTTS-R" },
    { value: "en_US-ljspeech-medium", label: "LJSpeech" },
    { value: "en_US-norman-medium", label: "Norman" },
    { value: "en_US-reza_ibrahim-medium", label: "Reza Ibrahim" },
    { value: "en_US-ryan-medium", label: "Ryan" },
    { value: "en_US-sam-medium", label: "Sam" }
  ];

  const KOKORO_VOICES = [
    { value: "af_heart", label: "Heart (af_heart)" },
    { value: "af_bella", label: "Bella (af_bella)" },
    { value: "af_nicole", label: "Nicole (af_nicole)" },
    { value: "af_sarah", label: "Sarah (af_sarah)" },
    { value: "af_sky", label: "Sky (af_sky)" },
    { value: "am_adam", label: "Adam (am_adam)" },
    { value: "am_michael", label: "Michael (am_michael)" },
    { value: "bf_emma", label: "Emma (bf_emma)" },
    { value: "bf_isabella", label: "Isabella (bf_isabella)" },
    { value: "bm_george", label: "George (bm_george)" },
    { value: "bm_lewis", label: "Lewis (bm_lewis)" }
  ];

  const POCKET_VOICES = [
    { value: "alba", label: "Alba (Default)" },
    { value: "marius", label: "Marius" },
    { value: "javert", label: "Javert" },
    { value: "jean", label: "Jean" },
    { value: "fantine", label: "Fantine" },
    { value: "cosette", label: "Cosette" },
    { value: "eponine", label: "Eponine" },
    { value: "azelma", label: "Azelma" }
  ];

  let isTesting = $state(false);
  let testResult = $state<{
    success: boolean;
    message: string;
    error_type?: string;
  } | null>(null);
  let dataDir = $state<string | null>(null);
  let homeDir = $state<string | null>(null);
  let previewExpanded = $state(false);

  const isActiveBackend = $derived(localConfig.tts.active_backend === "local");

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
      const result = (await invoke("test_tts_engine")) as {
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
  <div class="space-y-2">
    <Label for="tts-voice">{$_("engine.localEngine.voice")}</Label>
    {#if localConfig.tts.preset === "kitten-tts"}
      <Select
        id="tts-voice"
        options={KITTEN_VOICES}
        value={localConfig.tts.voice}
        onchange={(e) => {
          localConfig.tts.voice = (e.target as HTMLSelectElement).value;
        }}
      />
      <p class="text-muted-foreground text-xs">
        {$_("engine.localEngine.kittenInstall")}
      </p>
    {:else if localConfig.tts.preset === "piper"}
      <Select
        id="tts-voice"
        options={PIPER_EN_VOICES}
        value={localConfig.tts.voice}
        onchange={(e) => {
          localConfig.tts.voice = (e.target as HTMLSelectElement).value;
        }}
      />
      <p class="text-muted-foreground text-xs">
        {$_("engine.localEngine.piperModels", {
          values: { dataDir: dataDir ?? "%APPDATA%\\CopySpeak" }
        })}
        <button
          onclick={() => openExternal("https://github.com/OHF-Voice/piper1-gpl")}
          class="cursor-pointer underline"
        >
          piper1-gpl
        </button>.
      </p>
    {:else if localConfig.tts.preset === "kokoro-tts"}
      <Select
        id="tts-voice"
        options={KOKORO_VOICES}
        value={localConfig.tts.voice}
        onchange={(e) => {
          localConfig.tts.voice = (e.target as HTMLSelectElement).value;
        }}
      />
    {:else if localConfig.tts.preset === "pocket-tts"}
      <Select
        id="tts-voice"
        options={POCKET_VOICES}
        value={localConfig.tts.voice}
        onchange={(e) => {
          localConfig.tts.voice = (e.target as HTMLSelectElement).value;
        }}
      />
    {/if}
  </div>

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

  {#if isActiveBackend}
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
  {/if}
</div>
