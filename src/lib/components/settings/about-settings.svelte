<script lang="ts">
  import { Button } from "#lib/components/ui/button/index.js";
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { Switch } from "#lib/components/ui/switch/index.js";
  import { Badge } from "#lib/components/ui/badge/index.js";
  import UpdateChecker from "#lib/components/update-checker.svelte";
  import { openExternal } from "#lib/utils/external-link";
  import { invoke } from "@tauri-apps/api/core";
  import { VERSION } from "#lib/version";
  import { _ } from "svelte-i18n";
  import type { AppConfig } from "#lib/types";
  import type { Snippet } from "svelte";

  let {
    localConfig = $bindable(),
    showDebugMode = false,
    children
  }: {
    localConfig: AppConfig;
    showDebugMode?: boolean;
    children?: Snippet;
  } = $props();

  const providers = [
    {
      name: "OpenAI",
      url: "https://platform.openai.com/docs/guides/text-to-speech",
      mark: "openai",
      w: 80.35,
      h: 17.66
    },
    { name: "ElevenLabs", url: "https://elevenlabs.io/docs", mark: "elevenlabs", w: 123, h: 16 },
    { name: "Fish Audio", url: "https://docs.fish.audio/", mark: "fish", w: 128, h: 16 },
    { name: "Cartesia", url: "https://docs.cartesia.ai/", mark: "cartesia", w: 113, h: 16 },
    { name: "xAI", url: "https://docs.x.ai/", mark: "xai", w: 42, h: 16 },
    { name: "Mistral", url: "https://docs.mistral.ai/", mark: "mistral", w: 98, h: 22 },
    {
      name: "Google Gemini",
      url: "https://ai.google.dev/gemini-api/docs/speech-generation",
      mark: "gemini",
      w: 71,
      h: 16
    },
    {
      name: "Microsoft",
      url: "https://microsoft.ai/models/mai-voice-2-1/",
      mark: "mai",
      w: 37.1,
      h: 17.2
    },
    {
      name: "Kitten TTS",
      url: "https://github.com/KittenML/KittenTTS",
      mark: "kitten",
      w: 100.5,
      h: 18
    },
    { name: "Qwen3-TTS", url: "https://github.com/QwenLM/Qwen3-TTS", mark: "qwen", w: 98.2, h: 16 },
    {
      name: "Piper",
      url: "https://github.com/OHF-Voice/piper1-gpl",
      mark: "piper",
      w: 51.27,
      h: 16
    },
    { name: "Kokoro", url: "https://github.com/thewh1teagle/kokoro-onnx", mark: "", w: 0, h: 0 },
    {
      name: "Pocket TTS",
      url: "https://github.com/kyutai-labs/pocket-tts",
      mark: "pocket",
      w: 102.9,
      h: 16
    },
    { name: "Edge-TTS", url: "https://github.com/rany2/edge-tts", mark: "", w: 0, h: 0 }
  ];

  let logs = $state<string[]>([]);
  let logsPath = $state("");
  let isLoadingLogs = $state(false);
  let credit = $derived($_("settings.about.madeWith", { values: { heart: "{heart}" } }));
  let creditParts = $derived(credit.split("{heart}"));

  async function loadLogs() {
    isLoadingLogs = true;
    try {
      const rawLogs = await invoke<string>("get_logs", { maxLines: 20 });
      logsPath = await invoke("get_logs_path");
      logs = rawLogs
        .split("\n")
        .reverse()
        .filter((line) => line.trim().length > 0);
    } catch (error) {
      logs = [`Error loading logs: ${error}`];
    } finally {
      isLoadingLogs = false;
    }
  }

  $effect(() => {
    if (!localConfig.general.debug_mode) {
      logs = [];
      logsPath = "";
      return;
    }
    loadLogs();
    const id = setInterval(loadLogs, 2000);
    return () => clearInterval(id);
  });
</script>

<div class="space-y-4">
  <!-- Version -->
  <SettingRow
    label={$_("settings.about.version")}
    tooltip={$_("settings.about.versionDescription")}
  >
    <span class="text-muted-foreground font-mono text-sm">v{VERSION}</span>
  </SettingRow>

  <!-- Check for Updates -->
  <SettingRow
    label={$_("settings.about.checkUpdates")}
    tooltip={$_("settings.about.checkUpdatesTooltip")}
  >
    <Switch
      id="update-checks"
      checked={localConfig.general.update_checks_enabled ?? true}
      onchange={(v) => {
        localConfig.general.update_checks_enabled = v;
      }}
    />
  </SettingRow>

  <SettingRow
    label={$_("settings.about.updateStatus")}
    tooltip={$_("settings.about.updateStatusTooltip")}
  >
    <UpdateChecker
      enabled={localConfig.general.update_checks_enabled ?? true}
      autoCheckOnMount={false}
      listenForBackendEvent={false}
    />
  </SettingRow>

  <!-- Source Code -->
  <SettingRow
    label={$_("settings.about.sourceCode")}
    tooltip={$_("settings.about.sourceCodeDescription")}
  >
    <Button
      variant="outline"
      size="sm"
      onclick={() => openExternal("https://github.com/ilyaizen/CopySpeak")}
    >
      {$_("settings.about.github")}
    </Button>
  </SettingRow>

  {#if showDebugMode}
    <div>
      <SettingRow
        label={$_("settings.about.debugMode")}
        tooltip={$_("settings.about.debugModeTooltip")}
      >
        <Switch id="debug-mode" bind:checked={localConfig.general.debug_mode} />
      </SettingRow>

      {#if localConfig.general.debug_mode}
        <div class="mt-4 space-y-2">
          <div class="flex items-center gap-2">
            <Badge variant="secondary" class="text-[10px]">
              {logsPath || "Loading..."}
            </Badge>
            {#if isLoadingLogs}
              <span class="text-muted-foreground text-[10px]"
                >{$_("settings.about.refreshing")}</span
              >
            {/if}
          </div>
          <div class="bg-muted rounded-md border p-2">
            <div class="max-h-64 overflow-auto">
              {#each logs as line}
                <div class="odd:bg-muted/50 even:bg-transparent">
                  <pre
                    class="px-1 py-0.5 font-mono text-[10px] leading-tight break-all whitespace-pre-wrap">{line}</pre>
                </div>
              {:else}
                <div class="text-muted-foreground p-2 font-mono text-[10px]">
                  {$_("settings.about.noLogs")}
                </div>
              {/each}
            </div>
          </div>
        </div>
      {/if}
    </div>
  {/if}

  {@render children?.()}

  <section class="border-border border-t pt-4" aria-labelledby="acknowledgments-title">
    <h3 id="acknowledgments-title" class="mb-4 text-center text-sm font-semibold">
      {$_("settings.about.acknowledgments")}
    </h3>
    <ul class="flex flex-wrap justify-center gap-x-8 gap-y-5">
      {#each providers as provider (provider.name)}
        <li class="flex min-w-0">
          <!-- Mark and name are ONE link: hovering either highlights both and
               underlines the name. Only a plain primary click is intercepted,
               so modified and middle clicks keep native behavior. -->
          <a
            href={provider.url}
            target="_blank"
            rel="noopener noreferrer"
            class="text-muted-foreground hover:text-foreground group flex min-w-0 flex-1 flex-col items-center gap-2 rounded-sm transition-colors focus-visible:outline focus-visible:outline-2"
            onclick={(event) => {
              if (
                event.button !== 0 ||
                event.ctrlKey ||
                event.metaKey ||
                event.shiftKey ||
                event.altKey
              )
                return;
              event.preventDefault();
              void openExternal(provider.url);
            }}
          >
            <span class="flex h-7 items-center justify-center" aria-hidden="true">
              {#if provider.mark}
                <span
                  class="provider-mark"
                  style:width="{provider.w}px"
                  style:height="{provider.h}px"
                  style:--mark={`url('/landing/providers/${provider.mark}.svg')`}
                ></span>
              {:else}
                <span class="provider-text-mark">{provider.name}</span>
              {/if}
            </span>
            <span class="text-center text-xs underline-offset-4 group-hover:underline"
              >{provider.name}</span
            >
          </a>
        </li>
      {/each}
    </ul>
  </section>

  <div class="text-muted-foreground space-y-1 border-t pt-4 text-center text-xs">
    <p>🄯 {new Date().getFullYear()} Ilya Aizenberg</p>
    <p aria-label={credit.replace("{heart}", "♥")}>
      {creditParts[0]}<svg
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 24 24"
        class="credit-heart"
        aria-hidden="true"
        ><path
          d="M12 21 3.3 12.9C-1.4 8.5 4.9 1.3 10 5.3L12 7l2-1.7c5.1-4 11.4 3.2 6.7 7.6Z"
          fill="#e5484d"
        /></svg
      >{creditParts[1]}
    </p>
    <a
      href="https://www.buymeacoffee.com/ilyaizen"
      target="_blank"
      rel="noopener noreferrer"
      class="hover:text-foreground inline-block underline underline-offset-4 focus-visible:outline focus-visible:outline-2"
      onclick={(event) => {
        if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey)
          return;
        event.preventDefault();
        void openExternal("https://www.buymeacoffee.com/ilyaizen");
      }}>{$_("settings.about.buyMeCoffee")}</a
    >
  </div>
</div>

<style>
  .provider-mark {
    display: inline-block;
    background-color: currentColor;
    -webkit-mask: var(--mark) center / contain no-repeat;
    mask: var(--mark) center / contain no-repeat;
  }

  .provider-text-mark {
    font-size: 14px;
    font-weight: 600;
    line-height: 20px;
  }

  .credit-heart {
    display: inline-block;
    width: 1em;
    height: 1em;
    vertical-align: -0.12em;
  }
</style>
