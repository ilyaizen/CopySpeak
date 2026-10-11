<script lang="ts">
  import { Button } from "#lib/components/ui/button/index.js";
  import { Spinner } from "#lib/components/ui/spinner/index.js";
  import { FastForward, Pause, Play, RotateCcw, Rewind, Square } from "@lucide/svelte";
  import { _ } from "svelte-i18n";

  type PlayMode = "play" | "replay" | "history" | "disabled";

  let {
    isPlaying,
    isPaused,
    isSynthesizing,
    playMode,
    onPlay,
    onTogglePause,
    onStop,
    onSkipBack,
    onSkipForward
  } = $props<{
    isPlaying: boolean;
    isPaused: boolean;
    isSynthesizing: boolean;
    playMode: PlayMode;
    onPlay: () => void;
    onTogglePause: () => void;
    onStop: () => void;
    onSkipBack: () => void;
    onSkipForward: () => void;
  }>();

  const canStop = $derived(isPlaying || isSynthesizing);
  const canPause = $derived(isPlaying || isPaused);
  const canSkip = $derived(isPlaying);
  // The primary button only ever plays/pauses. Synthesis without audible audio
  // is a disabled spinner: stopping it is the Stop button's job, and routing a
  // click to Play here would fire a second speak_now mid-synthesis.
  const canPrimary = $derived(canPause || (!isSynthesizing && playMode !== "disabled"));
</script>

<div class="flex items-center gap-1">
  <Button
    variant="ghost"
    size="icon"
    onclick={onSkipBack}
    disabled={!canSkip}
    title={$_("play.skipBackTooltip")}
    aria-label={$_("play.skipBackTooltip")}
  >
    <Rewind />
  </Button>
  <Button
    variant="default"
    size="icon"
    onclick={canPause ? onTogglePause : onPlay}
    disabled={!canPrimary}
    title={canPause
      ? isPaused
        ? $_("play.resumeTooltip")
        : $_("play.pauseTooltip")
      : playMode === "replay"
        ? $_("play.replayTooltip")
        : $_("play.playTooltip")}
    aria-label={canPause
      ? isPaused
        ? $_("play.resumeTooltip")
        : $_("play.pauseTooltip")
      : playMode === "replay"
        ? $_("play.replayTooltip")
        : $_("play.playTooltip")}
  >
    {#if canPause}
      {#if isPaused}<Play />{:else}<Pause />{/if}
    {:else if isSynthesizing}<Spinner
        aria-hidden="true"
      />{:else if playMode === "replay"}<RotateCcw />{:else}<Play />{/if}
  </Button>
  <Button
    variant="ghost"
    size="icon"
    onclick={onSkipForward}
    disabled={!canSkip}
    title={$_("play.skipFwdTooltip")}
    aria-label={$_("play.skipFwdTooltip")}
  >
    <FastForward />
  </Button>
  <Button
    variant="ghost"
    size="icon"
    onclick={onStop}
    disabled={!canStop}
    title={$_("play.stopTooltip")}
    aria-label={$_("play.stopTooltip")}
  >
    <Square />
  </Button>
</div>
