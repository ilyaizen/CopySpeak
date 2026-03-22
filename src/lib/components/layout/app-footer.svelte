<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { cn } from "$lib/utils.js";
  import { listeningStore } from "$lib/stores/listening-store.svelte";
  import { VERSION } from "$lib/version";
  import type { AppConfig, TtsEngine } from "$lib/types";
  import { isTauri } from "$lib/services/tauri.js";
  import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuTrigger
  } from "$lib/components/ui/dropdown-menu";
  import { Check, Loader2 } from "@lucide/svelte";
  import { toast } from "svelte-sonner";
  import UpdateChecker from "../update-checker.svelte";
  import { _ } from "svelte-i18n";

  let isListening = $derived(listeningStore.isListening);
  let error = $derived(listeningStore.error);

  let engineLabel = $state<string | null>(null);
  let voiceLabel = $state<string | null>(null);
  let currentConfig = $state<AppConfig | null>(null);

  let isHudRoute = $state(false);
  let unlisten: (() => void) | null = null;

  // Dropdown state
  let dropdownOpen = $state(false);

  // Availability status type
  type AvailabilityStatus = "unknown" | "checking" | "available" | "unavailable" | "error";

  // Cached availability state (only checked when user explicitly switches engines)
  let availability = $state<Map<string, AvailabilityStatus>>(new Map());

  // Engine metadata
  interface EngineMeta {
    id: string;
    name: string;
    type: "cloud" | "local";
    tier: "free" | "paid" | "freemium";
    checkCmd?: string;
    preset?: string;
    command?: string;
    argsTemplate?: string[];
  }

  const ENGINES: EngineMeta[] = [
    {
      id: "kitten",
      name: "Kitten TTS",
      type: "local",
      tier: "free",
      preset: "kitten-tts",
      command: "py",
      argsTemplate: [
        "-3.12",
        "{home_dir}/kittentts/kittentts-cli.py",
        "--text",
        "{raw_text}",
        "--voice",
        "{voice}",
        "--output",
        "{output}"
      ]
    },
    {
      id: "piper",
      name: "Piper",
      type: "local",
      tier: "free",
      preset: "piper",
      command: "python3",
      argsTemplate: [
        "-m",
        "piper",
        "--data-dir",
        "{data_dir}",
        "-m",
        "{voice}",
        "-f",
        "{output}",
        "--input-file",
        "{input}"
      ]
    },
    {
      id: "kokoro",
      name: "Kokoro",
      type: "local",
      tier: "free",
      preset: "kokoro-tts",
      command: "kokoro-tts",
      argsTemplate: ["{input}", "{output}", "--voice", "{voice}"]
    },
    {
      id: "pocket",
      name: "Pocket",
      type: "local",
      tier: "free",
      preset: "pocket-tts",
      command: "pocket-tts",
      argsTemplate: [
        "generate",
        "--voice",
        "{voice}",
        "--text",
        "{raw_text}",
        "--output-path",
        "{output}"
      ]
    },
    {
      id: "elevenlabs",
      name: "ElevenLabs",
      type: "cloud",
      tier: "freemium",
      checkCmd: "check_elevenlabs_credentials"
    },
    {
      id: "openai",
      name: "OpenAI",
      type: "cloud",
      tier: "paid",
      checkCmd: "check_openai_credentials"
    }
  ];

  const DEFAULT_VOICES: Record<string, string> = {
    kitten: "Rosie",
    piper: "en_US-joe-medium",
    kokoro: "af_heart",
    pocket: "alba",
    openai: "alloy",
    elevenlabs: "21m00Tcm4TlvDq8ikWAM"
  };

  // Derive current engine ID from config
  const currentEngineId = $derived(() => {
    if (!currentConfig) return null;
    const backend = currentConfig.tts.active_backend;
    if (backend === "local") {
      const preset = currentConfig.tts.preset;
      if (preset === "kitten-tts") return "kitten";
      if (preset === "kokoro-tts") return "kokoro";
      if (preset === "pocket-tts") return "pocket";
      return "piper";
    }
    return backend;
  });

  // Capitalize engine name for display
  function capitalizeEngine(engine: string): string {
    switch (engine) {
      case "elevenlabs":
        return "ElevenLabs";
      case "openai":
        return "OpenAI";
      case "local":
        return "Local";
      default:
        return engine.charAt(0).toUpperCase() + engine.slice(1);
    }
  }

  // Capitalize voice name for display
  function capitalizeVoice(voice: string): string {
    if (!voice) return "";
    // For voices like "Rachel", "Alloy", etc. - capitalize first letter
    return voice.charAt(0).toUpperCase() + voice.slice(1).toLowerCase();
  }

  function getEngineLabel(config: AppConfig): string {
    const backend = config.tts.active_backend as TtsEngine;
    if (backend === "local") {
      const preset = config.tts.preset;
      if (preset === "kitten-tts") return "Kitten TTS";
      if (preset === "piper") return "Piper TTS";
      if (preset === "kokoro-tts") return "Kokoro TTS";
      if (preset === "pocket-tts") return "Pocket TTS";
      return capitalizeEngine(preset ?? "local");
    }
    return capitalizeEngine(backend);
  }

  // Get voice label for a specific engine (for dropdown items)
  function getVoiceLabelForEngine(config: AppConfig, engineId: string): string | null {
    switch (engineId) {
      case "elevenlabs": {
        // Use cached voice_name if available
        const voiceName = config.tts.elevenlabs.voice_name;
        if (voiceName) {
          // Extract just the name before " -" (e.g., "Rachel - Professional" -> "Rachel")
          const name = voiceName.split(" -")[0].trim();
          return capitalizeVoice(name);
        }
        return null;
      }
      case "openai":
        return capitalizeVoice(config.tts.openai.voice);
      case "piper":
      case "kitten":
      case "kokoro":
      case "pocket": {
        const backend = config.tts.active_backend as TtsEngine;
        if (backend === "local") {
          const preset = config.tts.preset;
          const isActiveLocalEngine =
            (engineId === "kitten" && preset === "kitten-tts") ||
            (engineId === "piper" && preset === "piper") ||
            (engineId === "kokoro" && preset === "kokoro-tts") ||
            (engineId === "pocket" && preset === "pocket-tts");

          if (isActiveLocalEngine && config.tts.voice) {
            const voice = config.tts.voice;

            if (engineId === "kitten") {
              return capitalizeVoice(voice);
            }

            if (engineId === "kokoro") {
              const parts = voice.split("_");
              if (parts.length >= 2) {
                return capitalizeVoice(parts[1]);
              }
              return capitalizeVoice(voice);
            }

            const parts = voice.split("-");
            if (parts.length >= 2) {
              const namePart = parts[parts.length - 2] || parts[1];
              return capitalizeVoice(namePart);
            }
            return capitalizeVoice(voice);
          }
        }
        return null;
      }
    }
    return null;
  }

  // Get voice label for current engine (for footer display)
  function getVoiceLabel(config: AppConfig): string | null {
    const currentId = currentEngineId();
    if (!currentId) return null;
    return getVoiceLabelForEngine(config, currentId);
  }

  async function loadEngineInfo() {
    if (!isTauri || isHudRoute) return;

    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const config = await invoke<AppConfig>("get_config");

      currentConfig = config;
      engineLabel = getEngineLabel(config);
      voiceLabel = getVoiceLabel(config);
    } catch {
      // Silently fail - footer will show fallback
    }
  }

  async function checkEngineAvailability(engine: EngineMeta): Promise<AvailabilityStatus> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");

      if (engine.type === "cloud" && engine.checkCmd) {
        // Cloud engines: use credential check command
        const result = await invoke<{ success: boolean; error_type?: string }>(engine.checkCmd);
        return result.success ? "available" : "unavailable";
      } else if (engine.type === "local") {
        // For local engines, only test the currently active one via test_tts_engine
        // Non-active local engines remain "unknown" until user switches to them
        const currentId = currentEngineId();
        if (currentId === engine.id) {
          // Active local engine - use full health check
          const result = await invoke<{ success: boolean }>("test_tts_engine");
          return result.success ? "available" : "unavailable";
        } else {
          // Non-active local engine - don't check CLI (causes window flashing)
          // User will discover if it works when they switch to it
          return "unknown";
        }
      }
      return "unknown";
    } catch (e) {
      console.error(`Availability check failed for ${engine.id}:`, e);
      return "error";
    }
  }

  // Check a single engine (only called when user switches to it)
  async function checkSingleEngine(engineId: string) {
    const engine = ENGINES.find((e) => e.id === engineId);
    if (!engine) return;

    // Mark as checking
    availability.set(engineId, "checking");

    try {
      const status = await checkEngineAvailability(engine);
      availability.set(engineId, status);
    } catch (e) {
      console.error(`Failed to check engine ${engineId}:`, e);
      availability.set(engineId, "error");
    }
  }

  async function switchEngine(engine: EngineMeta) {
    if (!currentConfig) return;

    const { invoke } = await import("@tauri-apps/api/core");
    const newConfig = JSON.parse(JSON.stringify(currentConfig)) as AppConfig;

    if (engine.type === "cloud") {
      newConfig.tts.active_backend = engine.id as TtsEngine;
      // Ensure default voice is set
      if (engine.id === "elevenlabs" && !newConfig.tts.elevenlabs.voice_id) {
        newConfig.tts.elevenlabs.voice_id = DEFAULT_VOICES.elevenlabs;
      } else if (engine.id === "openai" && !newConfig.tts.openai.voice) {
        newConfig.tts.openai.voice = DEFAULT_VOICES.openai;
      }
    } else {
      newConfig.tts.active_backend = "local";
      newConfig.tts.preset = engine.preset!;
      newConfig.tts.command = engine.command!;
      newConfig.tts.args_template = engine.argsTemplate!;
      if (!newConfig.tts.voice) {
        newConfig.tts.voice = DEFAULT_VOICES[engine.id];
      }
    }

    try {
      await invoke("set_config", { newConfig });
      // Check availability of the new engine after switching
      await checkSingleEngine(engine.id);
    } catch (e) {
      console.error("Failed to switch engine:", e);
      toast.error(`Failed to switch engine: ${e}`);
    }
  }

  onMount(async () => {
    isHudRoute = window.location.pathname.startsWith("/hud");
    if (isHudRoute || !isTauri) return;

    // Initial load
    await loadEngineInfo();

    // Listen for config changes to update engine display
    try {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen("config-changed", async () => {
        await loadEngineInfo();
      });
    } catch (e) {
      console.error("Failed to listen for config-changed:", e);
    }
  });

  onDestroy(() => {
    if (unlisten) {
      unlisten();
    }
  });
</script>

{#if !isHudRoute}
  <footer
    class="border-border bg-card/95 fixed right-0 bottom-0 left-0 z-50 border-t px-4 py-2.5 shadow-[0_-2px_10px_rgba(0,0,0,0.08)] backdrop-blur-sm"
  >
    <div class="flex items-center justify-between gap-2">
      <!-- Engine status with listening indicator -->
      <div class="flex min-w-0 items-center gap-2">
        {#if engineLabel !== null}
          <!-- Glowing green listening indicator for app status -->
          <div
            class={cn(
              "h-2.5 w-2.5 shrink-0 rounded-full",
              isListening ? "bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]" : "bg-muted"
            )}
            title={isListening ? $_("footer.listening") : $_("footer.paused")}
          ></div>

          <!-- Engine dropdown -->
          <DropdownMenu bind:open={dropdownOpen}>
            <DropdownMenuTrigger
              class="hover:bg-muted/50 focus:ring-ring cursor-pointer rounded px-1 py-0.5 text-sm transition-colors focus:ring-2 focus:ring-offset-1 focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
              aria-label={$_("footer.switchEngine", {
                values: { engine: `${engineLabel}${voiceLabel ? ` (${voiceLabel})` : ""}` }
              })}
            >
              <span class="text-card-foreground truncate">
                {engineLabel}{voiceLabel ? ` (${voiceLabel})` : ""}
              </span>
            </DropdownMenuTrigger>

            <DropdownMenuContent align="start" class="min-w-50">
              {#each ENGINES as engine}
                {@const status = availability.get(engine.id) ?? "unknown"}
                {@const isSelected = currentEngineId() === engine.id}
                {@const isDisabled = status === "unavailable" || status === "error"}
                {@const engineVoice = currentConfig
                  ? getVoiceLabelForEngine(currentConfig, engine.id)
                  : null}

                <DropdownMenuItem
                  disabled={isDisabled}
                  class="flex items-center justify-between gap-2 data-disabled:opacity-50"
                  onclick={() => switchEngine(engine)}
                >
                  <div class="flex items-center gap-2">
                    <!-- Status indicator -->
                    {#if status === "checking"}
                      <Loader2 class="text-muted-foreground h-3 w-3 animate-spin" />
                    {:else if status === "available"}
                      <div
                        class="h-2.5 w-2.5 shrink-0 rounded-full bg-green-500"
                        title={$_("footer.available")}
                        style="box-shadow: 0 0 6px rgba(34, 197, 94, 0.5);"
                      ></div>
                    {:else if status === "unavailable"}
                      <div
                        class="h-2.5 w-2.5 shrink-0 rounded-full bg-red-500"
                        title={$_("footer.unavailable")}
                        style="box-shadow: 0 0 6px rgba(239, 68, 68, 0.5);"
                      ></div>
                    {:else if status === "error"}
                      <div
                        class="h-2.5 w-2.5 shrink-0 rounded-full bg-red-500"
                        title={$_("footer.error")}
                        style="box-shadow: 0 0 6px rgba(239, 68, 68, 0.5);"
                      ></div>
                    {:else}
                      <div
                        class="h-2.5 w-2.5 shrink-0 rounded-full bg-gray-400"
                        title={$_("footer.notChecked")}
                      ></div>
                    {/if}

                    <!-- Engine name with voice -->
                    <span class={cn(isDisabled && "opacity-50")}>
                      {engine.name}
                      {#if engineVoice}
                        <span class="text-muted-foreground">({engineVoice})</span>
                      {/if}
                    </span>
                  </div>

                  <!-- Checkmark for selected -->
                  {#if isSelected}
                    <Check class="text-primary h-4 w-4 shrink-0" />
                  {/if}
                </DropdownMenuItem>
              {/each}
            </DropdownMenuContent>
          </DropdownMenu>
        {:else}
          <!-- Fallback: show listening status only -->
          <div
            class={cn(
              "h-2.5 w-2.5 shrink-0 rounded-full",
              isListening ? "bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]" : "bg-muted"
            )}
          ></div>
          <span class="text-card-foreground truncate text-sm">
            {isListening ? $_("footer.listening") : $_("footer.paused")}
          </span>
        {/if}
      </div>

      <!-- Version and Update Checker -->
      <div class="flex items-center gap-2">
        <UpdateChecker />
        <span class="bg-border mx-2 h-3 w-px"></span>
        <span class="text-muted-foreground shrink-0 text-xs">v{VERSION}</span>
      </div>
    </div>

    {#if error}
      <p class="text-destructive mt-1 text-xs">
        {error}
      </p>
    {/if}
  </footer>
{/if}
