<script lang="ts">
  import { _ } from "svelte-i18n";
  import { AudioLines, AudioWaveform, Copy, Eraser, Globe, History } from "@lucide/svelte";

  const features = $derived([
    {
      icon: Copy,
      title: $_("landing.features.doubleCopy.title"),
      description: $_("landing.features.doubleCopy.description")
    },
    {
      icon: Eraser,
      title: $_("landing.features.sanitization.title"),
      description: $_("landing.features.sanitization.description")
    },
    {
      icon: AudioLines,
      title: $_("landing.features.engines.title"),
      description: $_("landing.features.engines.description")
    },
    {
      icon: AudioWaveform,
      title: $_("landing.features.hud.title"),
      description: $_("landing.features.hud.description")
    },
    {
      icon: History,
      title: $_("landing.features.history.title"),
      description: $_("landing.features.history.description")
    },
    {
      icon: Globe,
      title: $_("landing.features.browser.title"),
      description: $_("landing.features.browser.description")
    }
  ]);
</script>

<div class="wrap">
  <section class="section" aria-labelledby="features">
    <div class="col">
      <h2 id="features">{$_("landing.features.title")}</h2>
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
