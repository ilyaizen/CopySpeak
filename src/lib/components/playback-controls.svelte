<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Spinner } from "$lib/components/ui/spinner/index.js";
  import { _ } from "svelte-i18n";

  type PlayMode = "play" | "replay" | "history" | "disabled";

  let { isPlaying, isPaused, isSynthesizing, playMode, onPlay, onStop, onTogglePause, onAbort } =
    $props<{
      isPlaying: boolean;
      isPaused: boolean;
      isSynthesizing: boolean;
      playMode: PlayMode;
      onPlay: () => void;
      onStop: () => void;
      onTogglePause: () => void;
      onAbort: () => void;
    }>();

  // Determine button states
  const canPlay = $derived(!isSynthesizing && !isPlaying && playMode !== "disabled");
</script>

<div class="flex flex-wrap items-center gap-2">
  <!-- Status indicator when synthesizing -->
  {#if isSynthesizing}
    <div class="bg-muted text-muted-foreground flex items-center gap-2 rounded-md px-3 py-2">
      <Spinner class="h-4 w-4" />
      <span class="text-sm">{$_("play.processing")}</span>
    </div>
  {/if}

  <!-- Play/Replay button — always visible -->
  <Button
    variant="default"
    onclick={onPlay}
    disabled={!canPlay}
    title={playMode === "replay"
      ? $_("play.replayTooltip")
      : playMode === "history"
        ? $_("play.playLatestTooltip")
        : $_("play.playTooltip")}
  >
    {playMode === "replay" ? $_("play.replay") : $_("play.play")}
  </Button>

  <!-- Pause/Resume button — only when playing -->
  {#if isPlaying}
    <Button
      variant="default"
      onclick={onTogglePause}
      title={isPaused ? $_("play.resumeTooltip") : $_("play.pauseTooltip")}
    >
      {isPaused ? $_("play.resume") : $_("play.pause")}
    </Button>
  {/if}

  <!-- Stop button — only when playing -->
  {#if isPlaying}
    <Button variant="destructive" onclick={onStop} title={$_("play.stopTooltip")}
      >{$_("play.stop")}</Button
    >
  {/if}

  <!-- Abort button — only when synthesizing or playing -->
  {#if isSynthesizing || isPlaying}
    <Button variant="destructive" onclick={onAbort} title={$_("play.abortTooltip")}>
      {$_("play.abort")}
    </Button>
  {/if}
</div>
