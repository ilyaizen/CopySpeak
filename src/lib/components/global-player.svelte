<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { playbackStore } from "$lib/stores/playback-store.svelte";

  let audioEl = $state<HTMLAudioElement | null>(null);

  onMount(async () => {
    playbackStore.setAudioElement(audioEl);
    await playbackStore.setupListeners();
  });

  onDestroy(() => {
    playbackStore.teardownListeners();
    playbackStore.setAudioElement(null);
  });
</script>

<!-- svelte-ignore a11y_media_has_caption -->
<audio bind:this={audioEl} style="display:none"></audio>
