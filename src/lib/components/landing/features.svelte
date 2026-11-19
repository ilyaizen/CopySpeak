<script lang="ts">
  import { AudioLines, AudioWaveform, Copy, Eraser, Globe, History } from "@lucide/svelte";

  const features = [
    {
      icon: Copy,
      title: "Double-Copy Trigger",
      description: "Copy the same text twice within 1.5 s to hear it. No hotkeys to learn."
    },
    {
      icon: Eraser,
      title: "Text sanitization",
      description: "Strips markdown, expands URLs and normalizes text before it is spoken."
    },
    {
      icon: AudioLines,
      title: "Offline & Cloud TTS Engines",
      description:
        "tiny-voca, Kokoro, Piper, Kitten, Qwen3 and Pocket run fully offline. OpenAI, ElevenLabs, Cartesia, Google, Microsoft, Edge or any HTTP endpoint for cloud voices."
    },
    {
      icon: AudioWaveform,
      title: "HUD and live captions",
      description: "A floating waveform and word-level captions, synced to the audio clock."
    },
    {
      icon: History,
      title: "History",
      description:
        "Every generation is kept unless you switch history off. Replay and manage saved readings."
    },
    {
      icon: Globe,
      title: "Browser companion",
      description:
        "Reads your selection or hovered paragraphs in Chrome and Edge, highlighting the passage as it plays."
    }
  ];
</script>

<div class="wrap">
  <section class="section" aria-labelledby="features">
    <div class="col">
      <h2 id="features">Features</h2>
      {#each features as feature}
        {@const Icon = feature.icon}
        <article class="feat">
          <h3><Icon class="icon" />{feature.title}</h3>
          <p>{feature.description}</p>
        </article>
      {/each}
    </div>
  </section>
</div>
