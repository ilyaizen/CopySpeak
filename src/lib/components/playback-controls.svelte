<script lang="ts">
  import { Button } from "$lib/components/ui/button/index.js";
  import { Spinner } from "$lib/components/ui/spinner/index.js";
  import { Play, Square, RotateCcw } from "@lucide/svelte";
  import { _ } from "svelte-i18n";

  type PlayMode = "play" | "replay" | "history" | "disabled";

  let { isPlaying, isSynthesizing, playMode, onPlay, onStop, onAbort } = $props<{
    isPlaying: boolean;
    isSynthesizing: boolean;
    playMode: PlayMode;
    onPlay: () => void;
    onStop: () => void;
    onAbort: () => void;
  }>();

  const canStop = $derived(isPlaying || isSynthesizing);
</script>

<Button
  variant={canStop ? "secondary" : "default"}
  onclick={isSynthesizing ? onAbort : isPlaying ? onStop : onPlay}
  disabled={!canStop && playMode === "disabled"}
  title={canStop
    ? $_("play.stopTooltip")
    : playMode === "replay"
      ? $_("play.replayTooltip")
      : $_("play.playTooltip")}
>
  {#if isSynthesizing}<Spinner aria-hidden="true" />{:else if isPlaying}<Square
    />{:else if playMode === "replay"}<RotateCcw />{:else}<Play />{/if}
  {canStop ? $_("play.stop") : playMode === "replay" ? $_("play.replay") : $_("play.play")}
</Button>
