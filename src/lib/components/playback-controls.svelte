<script lang="ts">
  import { Button } from "#lib/components/ui/button/index.js";
  import SkipBackFilled from "#lib/components/icons/skip-back.svelte";
  import SkipForwardFilled from "#lib/components/icons/skip-forward.svelte";
  import PlayFilled from "#lib/components/icons/play.svelte";
  import PauseFilled from "#lib/components/icons/pause.svelte";
  import ReplayFilled from "#lib/components/icons/replay.svelte";
  import StopFilled from "#lib/components/icons/stop.svelte";
  import { Spinner } from "#lib/components/ui/spinner/index.js";
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
  }: Props = $props();

  interface Props {
    isPlaying: boolean;
    isPaused: boolean;
    isSynthesizing: boolean;
    playMode: PlayMode;
    onPlay: () => void;
    onTogglePause: () => void;
    onStop: () => void;
    onSkipBack: () => void;
    onSkipForward: () => void;
  }

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
    class="rtl:rotate-180"
    onclick={onSkipBack}
    disabled={!canSkip}
    title={$_("play.skipBackTooltip")}
    aria-label={$_("play.skipBackTooltip")}
  >
    <SkipBackFilled />
  </Button>
  <Button
    variant="default"
    size="icon"
    class="rtl:rotate-180"
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
      {#if isPaused}<PlayFilled />{:else}<PauseFilled />{/if}
    {:else if isSynthesizing}<Spinner
        aria-hidden="true"
      />{:else if playMode === "replay"}<ReplayFilled />{:else}<PlayFilled />{/if}
  </Button>
  <Button
    variant="ghost"
    size="icon"
    class="rtl:rotate-180"
    onclick={onSkipForward}
    disabled={!canSkip}
    title={$_("play.skipFwdTooltip")}
    aria-label={$_("play.skipFwdTooltip")}
  >
    <SkipForwardFilled />
  </Button>
  <Button
    variant="ghost"
    size="icon"
    class="rtl:rotate-180"
    onclick={onStop}
    disabled={!canStop}
    title={$_("play.stopTooltip")}
    aria-label={$_("play.stopTooltip")}
  >
    <StopFilled />
  </Button>
</div>
