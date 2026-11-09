<script lang="ts">
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";
  import { openExternal } from "#lib/utils/external-link";

  const providers = [
    {
      name: "OpenAI",
      url: "https://platform.openai.com/docs/guides/text-to-speech",
      mark: "openai",
      w: 80.35,
      h: 17.66
    },
    { name: "ElevenLabs", url: "https://elevenlabs.io/docs", mark: "elevenlabs", w: 123, h: 16 },
    { name: "Fish Audio", url: "https://docs.fish.audio/", mark: "fish", w: 128, h: 16 },
    { name: "Cartesia", url: "https://docs.cartesia.ai/", mark: "cartesia", w: 113, h: 16 },
    { name: "xAI", url: "https://docs.x.ai/", mark: "xai", w: 42, h: 16 },
    { name: "Mistral", url: "https://docs.mistral.ai/", mark: "mistral", w: 98, h: 22 },
    {
      name: "Google Gemini",
      url: "https://ai.google.dev/gemini-api/docs/speech-generation",
      mark: "gemini",
      w: 71,
      h: 16
    },
    {
      name: "Microsoft",
      url: "https://microsoft.ai/models/mai-voice-2-1/",
      mark: "mai",
      w: 37.1,
      h: 17.2
    },
    {
      name: "Kitten TTS",
      url: "https://github.com/KittenML/KittenTTS",
      mark: "kitten",
      w: 100.5,
      h: 18
    },
    { name: "Qwen3-TTS", url: "https://github.com/QwenLM/Qwen3-TTS", mark: "qwen", w: 98.2, h: 16 },
    {
      name: "Piper",
      url: "https://github.com/OHF-Voice/piper1-gpl",
      mark: "piper",
      w: 51.27,
      h: 16
    },
    { name: "Kokoro", url: "https://github.com/thewh1teagle/kokoro-onnx", mark: "", w: 0, h: 0 },
    {
      name: "Pocket TTS",
      url: "https://github.com/kyutai-labs/pocket-tts",
      mark: "pocket",
      w: 102.9,
      h: 16
    },
    { name: "Edge-TTS", url: "https://github.com/rany2/edge-tts", mark: "", w: 0, h: 0 }
  ];

  let viewport: HTMLDivElement;
  let track: HTMLDivElement;
  let row: HTMLUListElement;
  let hover = false;
  let focus = false;
  let pointerId: number | null = null;
  let startX = 0;
  let previousX = 0;
  let moved = false;
  let offset = 0;
  let reducedMotion = false;

  function updatePosition() {
    const width = row.getBoundingClientRect().width;
    if (width > 0) offset = ((offset % width) + width) % width;
    track.style.transform = `translateX(${-offset}px)`;
  }

  function pointerDown(event: PointerEvent) {
    if (reducedMotion || !event.isPrimary || event.button !== 0) return;
    pointerId = event.pointerId;
    startX = event.clientX;
    previousX = event.clientX;
    moved = false;
  }

  function pointerMove(event: PointerEvent) {
    if (event.pointerId !== pointerId) return;
    if (!moved && Math.abs(event.clientX - startX) >= 5) {
      moved = true;
      viewport.setPointerCapture(event.pointerId);
      previousX = startX;
    }
    if (moved) {
      offset -= event.clientX - previousX;
      updatePosition();
    }
    previousX = event.clientX;
  }

  function pointerEnd(event: PointerEvent) {
    if (event.pointerId !== pointerId) return;
    pointerId = null;
  }

  function openProvider(event: MouseEvent, url: string) {
    if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey)
      return;
    event.preventDefault();
    void openExternal(url);
  }

  onMount(() => {
    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    const syncMotion = () => {
      reducedMotion = motion.matches;
      if (reducedMotion) {
        offset = 0;
        updatePosition();
      }
    };
    syncMotion();
    motion.addEventListener("change", syncMotion);
    let last = 0;
    let frame = 0;
    const animate = (time: number) => {
      if (last && !reducedMotion && !hover && !focus && pointerId === null) {
        offset += Math.min(time - last, 100) * 0.03;
        updatePosition();
      }
      last = time;
      frame = requestAnimationFrame(animate);
    };
    frame = requestAnimationFrame(animate);
    return () => {
      cancelAnimationFrame(frame);
      motion.removeEventListener("change", syncMotion);
    };
  });
</script>

{#snippet providerRow(clone: boolean)}
  {#each providers as provider (provider.name)}
    <li>
      <a
        href={provider.url}
        target="_blank"
        rel="noopener noreferrer"
        aria-label={provider.name}
        tabindex={clone ? -1 : undefined}
        onclick={(event) => openProvider(event, provider.url)}
      >
        {#if provider.mark}
          <span
            class="provider-mark"
            aria-hidden="true"
            style:width="{provider.w}px"
            style:height="{provider.h}px"
            style:--mark={`url('/landing/providers/${provider.mark}.svg')`}
          ></span>
        {:else}
          <span class="provider-text-mark" aria-hidden="true">{provider.name}</span>
        {/if}
      </a>
    </li>
  {/each}
{/snippet}

<section class="acknowledgments" aria-labelledby="acknowledgments-title">
  <h3 id="acknowledgments-title" class="text-center text-sm font-semibold">
    {$_("settings.about.acknowledgments")}
  </h3>
  <p class="text-muted-foreground mt-1 text-center text-sm">
    {$_("settings.about.acknowledgmentsTagline")}
  </p>
  <div
    bind:this={viewport}
    class="marquee"
    role="group"
    aria-labelledby="acknowledgments-title"
    dir="ltr"
    onpointerenter={() => (hover = true)}
    onpointerleave={() => (hover = false)}
    onpointerdown={pointerDown}
    onpointermove={pointerMove}
    onpointerup={pointerEnd}
    onpointercancel={pointerEnd}
    onlostpointercapture={pointerEnd}
    onfocusin={(event) => {
      focus = true;
      if (reducedMotion || !(event.target instanceof HTMLAnchorElement)) return;
      viewport.scrollLeft = 0;
      const bounds = viewport.getBoundingClientRect();
      const link = event.target.getBoundingClientRect();
      if (link.left < bounds.left + 12) offset += link.left - bounds.left - 12;
      if (link.right > bounds.right - 12) offset += link.right - bounds.right + 12;
      updatePosition();
    }}
    onfocusout={(event) => {
      if (!(event.relatedTarget instanceof Node) || !viewport.contains(event.relatedTarget))
        focus = false;
    }}
  >
    <div bind:this={track} class="track">
      <ul bind:this={row} class="provider-list">
        {@render providerRow(false)}
      </ul>
      <ul class="provider-list" aria-hidden="true">
        {@render providerRow(true)}
      </ul>
    </div>
  </div>
</section>

<style>
  .acknowledgments {
    padding: 24px 0;
  }
  .marquee {
    overflow: hidden;
    user-select: none;
    touch-action: pan-y;
    cursor: grab;
    -webkit-mask-image: linear-gradient(
      to right,
      transparent 0%,
      black 10%,
      black 90%,
      transparent 100%
    );
    mask-image: linear-gradient(to right, transparent 0%, black 10%, black 90%, transparent 100%);
  }
  .marquee:active {
    cursor: grabbing;
  }
  .track {
    display: flex;
    width: max-content;
    will-change: transform;
  }
  .provider-list {
    display: flex;
    flex: none;
    align-items: center;
    gap: 32px;
    margin: 0;
    padding: 16px 32px 16px 0;
    list-style: none;
  }
  .provider-list a {
    display: inline-flex;
    align-items: center;
    min-height: 28px;
    color: var(--muted-foreground);
    transition: color 150ms;
  }
  .provider-list a:hover,
  .provider-list a:focus-visible {
    color: var(--foreground);
  }
  .provider-list a:focus-visible {
    outline: 2px solid currentColor;
    outline-offset: 3px;
  }
  .provider-mark {
    display: inline-block;
    flex: none;
    background-color: currentColor;
    -webkit-mask: var(--mark) center / contain no-repeat;
    mask: var(--mark) center / contain no-repeat;
  }
  .provider-text-mark {
    font-size: 14px;
    font-weight: 600;
    line-height: 20px;
  }
  @media (prefers-reduced-motion: reduce) {
    .marquee {
      overflow-x: auto;
      cursor: auto;
    }
    .track {
      will-change: auto;
    }
    .provider-list[aria-hidden="true"] {
      display: none;
    }
  }
</style>
