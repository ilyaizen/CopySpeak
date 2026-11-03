<script lang="ts">
  import { _ } from "svelte-i18n";

  // Mirrors CLOUD_ENGINES / LOCAL_ENGINES order from #lib/components/engine/engine-meta.ts,
  // minus Edge (retired from every UI surface). Wordmarks exist where the owner's terms
  // permit the placement or the project owner chose to proceed (decisions + sources:
  // static/landing/providers/README.md); every other engine stays plain text on purpose.
  // Width/height are the intrinsic box size at a
  // ~16px ink height so the row reserves its layout before the SVGs load; boxes are
  // taller than the ink where the owner's file bakes clear space into its viewBox
  // (Mistral's 98x22, Microsoft AI's 37.1x17.2) or where the mark is deliberately
  // scaled up (Kitten's 18px: its typeset text is smaller than the cat, so the
  // whole lockup renders above the band height). OpenAI ships as ONE combined
  // lockup (Blossom + wordmark, including its descender) composed from the owner's kit
  // files. Marks render as CSS alpha masks painted with currentColor, so one file
  // per provider serves both themes.
  type ProviderMark = { mark: string; w: number; h: number };
  type Provider = { name: string; href: string; mark?: ProviderMark };

  const cloud: Provider[] = [
    {
      name: "OpenAI",
      href: "https://openai.com",
      mark: { mark: "openai", w: 80.35, h: 17.66 }
    },
    {
      name: "ElevenLabs",
      href: "https://elevenlabs.io",
      mark: { mark: "elevenlabs", w: 123, h: 16 }
    },
    {
      name: "Fish Audio",
      href: "https://fish.audio",
      mark: { mark: "fish", w: 128, h: 16 }
    },
    {
      name: "Cartesia",
      href: "https://cartesia.ai",
      mark: { mark: "cartesia", w: 113, h: 16 }
    },
    {
      name: "SpaceXAI",
      href: "https://x.ai",
      mark: { mark: "xai", w: 42, h: 16 }
    },
    {
      name: "Mistral",
      href: "https://mistral.ai",
      mark: { mark: "mistral", w: 98, h: 22 }
    },
    {
      name: "Google Gemini",
      href: "https://ai.google.dev",
      mark: { mark: "gemini", w: 71, h: 16 }
    },
    {
      name: "Microsoft AI",
      href: "https://microsoft.ai",
      // 56x26 viewBox with ~24.16 units of ink: the 17.2px box puts the ink
      // at the band's ~16px mark height (a 16px box would render it 14.9px).
      mark: { mark: "mai", w: 37.1, h: 17.2 }
    }
  ];

  const localEngines: Provider[] = [
    {
      name: "KittenTTS",
      href: "https://github.com/KittenML/KittenTTS",
      // Composed lockup: official cat mark (SVG mask conversion) + "Kitten TTS"
      // typeset in the owner's hosted Geist SemiBold, head-dominant (text ink =
      // 0.65x cat ink height). See README.
      mark: { mark: "kitten", w: 100.5, h: 18 }
    },
    {
      name: "Qwen",
      href: "https://github.com/QwenLM/Qwen3-TTS",
      // Wordmark line of the official Qwen3-TTS repo logo raster.
      mark: { mark: "qwen", w: 98.2, h: 16 }
    },
    {
      name: "Piper",
      href: "https://github.com/OHF-Voice/piper1-gpl",
      mark: { mark: "piper", w: 51.27, h: 16 }
    },
    { name: "Kokoro", href: "https://github.com/thewh1teagle/kokoro-onnx" },
    {
      name: "Pocket TTS",
      href: "https://github.com/kyutai-labs/pocket-tts",
      mark: { mark: "pocket", w: 102.9, h: 16 }
    }
  ];
</script>

{#snippet providerItems(items: Provider[], clone: boolean = false)}
  {#each items as item}
    <li>
      <a
        href={item.href}
        target="_blank"
        rel="noopener noreferrer"
        tabindex={clone ? -1 : undefined}
      >
        {#if item.mark}
          <span
            class="provider-mark"
            role="img"
            aria-label={item.name}
            style:width="{item.mark.w}px"
            style:height="{item.mark.h}px"
            style:--mark={`url("/landing/providers/${item.mark.mark}.svg")`}
          ></span>
        {:else}
          {item.name}
        {/if}
      </a>
    </li>
  {/each}
{/snippet}

{#snippet providerLine(clone: boolean = false)}
  <li class="label">{$_("landing.providers.cloud")}</li>
  {@render providerItems(cloud, clone)}
  <li class="label">{$_("landing.providers.offline")}</li>
  {@render providerItems(localEngines, clone)}
{/snippet}

<div class="wrap">
  <section class="providers" aria-labelledby="providers-title">
    <h2 class="sr-only" id="providers-title">{$_("landing.providers.title")}</h2>
    <div class="providers-carousel">
      <div class="providers-track">
        <ul class="providers-list">
          {@render providerLine()}
        </ul>
        <!-- Marquee clone: stays mouse-clickable (no inert — each copy is
             narrower than the carousel, so the clone is on screen for most
             of the loop and inert would make visible logos dead). It keeps
             aria-hidden, and tabindex=-1 keeps focus out of that subtree. -->
        <ul class="providers-list" aria-hidden="true">
          {@render providerLine(true)}
        </ul>
      </div>
    </div>
  </section>
</div>
