<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import PlaybackControls from "$lib/components/playback-controls.svelte";
  import RecentHistory from "$lib/components/recent-history.svelte";
  import QuickSettings from "$lib/components/quick-settings.svelte";
  import { historyStore } from "$lib/stores/history-store.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Textarea } from "$lib/components/ui/textarea/index.js";
  import { playbackStore } from "$lib/stores/playback-store.svelte";
  import { toast } from "svelte-sonner";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { isTauri } from "$lib/services/tauri.js";
  import type { AppConfig } from "$lib/types";
  import { _ } from "svelte-i18n";

  const mockConfig: AppConfig = {
    trigger: {
      listen_enabled: true,
      double_copy_window_ms: 800,
      max_text_length: 50000
    },
    tts: {
      active_backend: "local",
      preset: "kokoro-tts",
      command: "kokoro-tts",
      args_template: [],
      voice: "adam",
      openai: {
        api_key: "",
        model: "tts-1",
        voice: "alloy"
      },
      elevenlabs: {
        api_key: "",
        voice_id: "21m00Tcm4TlvDq8ikWAM",
        model_id: "eleven_turbo_v2_5",
        output_format: "mp3_44100_128",
        voice_stability: 0.5,
        voice_similarity_boost: 0.75,
        voice_style: undefined,
        use_speaker_boost: undefined
      }
    },
    playback: {
      on_retrigger: "interrupt",
      volume: 100,
      playback_speed: 1.0,
      pitch: 1.0
    },
    hud: {
      enabled: false,
      position: "bottom-right",
      width: 300,
      height: 100,
      opacity: 0.85,
      theme: {
        preset: "dark",
        waveform_color: "rgba(255, 255, 255, 0.3)",
        waveform_active_color: "rgba(96, 165, 250, 1)",
        background_color: "rgba(0, 0, 0, 0.75)",
        border_radius: 12,
        animation_speed: 1.0
      }
    },
    general: {
      start_with_windows: false,
      start_minimized: true,
      show_notifications: true,
      debug_mode: false,
      close_behavior: "minimize-to-tray",
      appearance: "system",
      locale: "en"
    },
    output: {
      enabled: false,
      directory: "",
      filename_pattern: "copyspeak-{compact_datetime}-{seq}.wav",
      format_config: {
        format: "wav",
        mp3_bitrate: 192,
        ogg_bitrate: 192,
        flac_compression: 5
      }
    },
    sanitization: {
      enabled: true,
      markdown: {
        enabled: true,
        strip_headers: true,
        strip_code_blocks: true,
        strip_inline_code: true,
        strip_links: true,
        strip_bold_italic: true,
        strip_lists: true,
        strip_blockquotes: true
      },
      tts_normalization: {
        enabled: true
      }
    },
    pagination: {
      enabled: true,
      fragment_size: 2000
    },
    history: {
      enabled: true,
      storage_mode: "persistent",
      persistent_dir: null,
      auto_delete: { keep_latest: 50 },
      cleanup_orphaned_files: true
    },
    hotkey: {
      enabled: false,
      shortcut: "Super+Shift+A"
    }
  };

  let config = $state<AppConfig | null>(null);
  let manualText = $state("");
  let lastPlayedContent = $state<string | null>(null);
  let abortRequested = $state(false);
  let error = $state<string | null>(null);
  let truncationWarning = $state<{
    originalLength: number;
    truncatedLength: number;
    maxLength: number;
  } | null>(null);

  let unlistenTruncated: (() => void) | null = null;
  let unlistenHistoryUpdate: (() => void) | null = null;

  // Proxy store state for template readability
  let isPlaying = $derived(playbackStore.isPlaying);
  let isPaused = $derived(playbackStore.isPaused);

  // Sync playback config to store and auto-save (debounced)
  $effect(() => {
    if (config) {
      const { volume, playback_speed, pitch } = config.playback;

      // Keep playback store in sync so audio plays at correct settings
      playbackStore.syncPlaybackConfig(volume, playback_speed, pitch);

      const timeout = setTimeout(async () => {
        if (isTauri) {
          try {
            await invoke("set_config", { newConfig: config });
          } catch {
            // Ignore save errors
          }
        }
      }, 500);
      return () => clearTimeout(timeout);
    }
  });

  async function loadConfig() {
    if (!isTauri) {
      config = mockConfig;
      return;
    }
    try {
      config = await invoke<AppConfig>("get_config");
    } catch (e) {
      error = `Failed to load config: ${e}`;
      config = mockConfig;
    }
  }

  // Determine current content from manual text input
  let currentContent = $derived(manualText.trim() || "");

  // Most-recent history item, pre-sorted to avoid re-sorting in handlers
  let latestHistoryItem = $derived(
    [...historyStore.items].sort((a, b) => b.timestamp - a.timestamp)[0] ?? null
  );

  // Smart play mode: what the Play button will do
  type PlayMode = "play" | "replay" | "history" | "disabled";
  let playMode: PlayMode = $derived(
    currentContent
      ? playbackStore.hasCachedAudio && currentContent === lastPlayedContent
        ? "replay"
        : "play"
      : historyStore.items.length > 0
        ? "history"
        : "disabled"
  );

  async function handleReplay() {
    await playbackStore.handleReplay();
  }

  // Unified Play handler — dispatches based on playMode
  async function handlePlay() {
    if (!isTauri) return;
    error = null;
    if (playMode === "play") {
      await handleGenerate();
    } else if (playMode === "replay") {
      await handleReplay();
    } else if (playMode === "history") {
      const latest = latestHistoryItem;
      if (latest) {
        // Check if output_path exists before trying to play
        if (latest.output_path) {
          try {
            await historyStore.playEntry(latest.id);
          } catch (e) {
            error = `Failed to play: ${e}`;
          }
        } else {
          // No saved file — re-synthesize
          try {
            await historyStore.reSpeakEntry(latest.id);
          } catch (e) {
            error = `${e}`;
          }
        }
      }
    }
  }

  async function handleStop() {
    // Stop playback immediately in the frontend
    playbackStore.handleStop();

    // Also notify backend to ensure complete stop
    if (isTauri) {
      try {
        await invoke("stop_speaking");
      } catch (error) {
        console.error("Failed to stop speaking:", error);
      }
    }
  }

  async function handleTogglePause() {
    // Toggle pause in frontend
    playbackStore.handleTogglePause();

    // Also notify backend for consistency
    if (isTauri) {
      try {
        await invoke("toggle_pause");
      } catch (error) {
        console.error("Failed to toggle pause:", error);
      }
    }
  }

  async function handleGenerate() {
    if (!isTauri) {
      error = "Not running in Tauri environment";
      return;
    }
    try {
      error = null;
      abortRequested = false;
      await invoke("speak_now", { text: currentContent });
      lastPlayedContent = currentContent;
    } catch (e) {
      // Don't show error if abort was requested (process was killed)
      if (!abortRequested) {
        error = `${e}`;
      }
    }
  }

  async function handleAbort() {
    abortRequested = true;

    // Stop playback immediately
    playbackStore.handleStop();

    // Abort synthesis in backend
    if (isTauri) {
      try {
        await invoke("abort_synthesis");
        toast.success($_("toast.success.synthesisAborted"));
      } catch (error) {
        console.error("Failed to abort synthesis:", error);
        toast.error("Failed to abort synthesis");
      }
    }
  }

  onMount(async () => {
    await loadConfig();

    if (isTauri) {
      await historyStore.loadHistory();
      try {
        unlistenTruncated = await listen<{
          original_length: number;
          truncated_length: number;
          max_length: number;
        }>("text-truncated", (event) => {
          truncationWarning = {
            originalLength: event.payload.original_length,
            truncatedLength: event.payload.truncated_length,
            maxLength: event.payload.max_length
          };
          setTimeout(() => (truncationWarning = null), 5000);
        });
        unlistenHistoryUpdate = await listen<any>("history-updated", async () => {
          await historyStore.refresh();
        });
      } catch {}
    }
  });

  onDestroy(() => {
    if (unlistenTruncated) unlistenTruncated();
    if (unlistenHistoryUpdate) unlistenHistoryUpdate();
  });
</script>

<div class="flex flex-col gap-4">
  {#if config}
    <QuickSettings bind:config>
      <div class="border-border bg-card flex h-full flex-col rounded-lg border p-3 shadow-sm">
        <Textarea
          class="min-h-0 flex-1 resize-none"
          placeholder={$_("play.placeholder")}
          bind:value={manualText}
        />
        <div class="mt-2 flex items-center gap-2">
          <PlaybackControls
            {isPlaying}
            {isPaused}
            isSynthesizing={playbackStore.isSynthesizing}
            {playMode}
            onPlay={handlePlay}
            onStop={handleStop}
            onTogglePause={handleTogglePause}
            onAbort={handleAbort}
          />
          {#if manualText}
            <Button variant="ghost" size="sm" onclick={() => (manualText = "")}
              >{$_("play.clear")}</Button
            >
          {/if}
          <span class="text-muted-foreground ml-auto text-xs">
            {$_("play.characters", { values: { count: manualText.length.toLocaleString() } })}
          </span>
        </div>
      </div>
    </QuickSettings>
  {/if}

  <!-- Recent Generations History -->
  <div class="border-border bg-card rounded-lg border p-4 shadow-sm">
    <RecentHistory limit={10} />
  </div>
</div>

{#if error}
  <div class="border-destructive/50 bg-destructive/10 mt-4 rounded-lg border p-3">
    <p class="text-destructive text-sm">
      {error}
    </p>
  </div>
{/if}

{#if truncationWarning}
  <div class="mt-4 rounded-none border border-amber-500/50 bg-amber-500/10 p-3">
    <p class="text-sm text-amber-600 dark:text-amber-400">
      {$_("play.truncatedDescription", {
        values: {
          original: truncationWarning.originalLength.toLocaleString(),
          truncated: truncationWarning.truncatedLength.toLocaleString(),
          max: truncationWarning.maxLength.toLocaleString()
        }
      })}
    </p>
  </div>
{/if}
