<script lang="ts">
  import { _ } from "svelte-i18n";
  import { Button } from "#lib/components/ui/button/index.js";
  import { VERSION } from "#lib/version";
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
    { name: "Edge-TTS", url: "https://github.com/rany2/edge-tts", mark: "", w: 0, h: 0 }
  ];

  const localProviders = [
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
    }
  ];

  function marquee(viewport: HTMLDivElement, direction: number) {
    const track = viewport.querySelector(".track");
    const row = viewport.querySelector(".provider-list");
    if (!(track instanceof HTMLElement) || !(row instanceof HTMLElement)) {
      throw new Error("Marquee track missing");
    }
    const trackElement = track;
    const rowElement = row;
    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    let offset = 0;
    let speed = 0;
    let last = 0;
    let hover = false;
    let focus = false;
    let pointerId: number | null = null;
    let startX = 0;
    let previousX = 0;
    let moved = false;
    let frame = 0;

    function updatePosition() {
      const width = rowElement.getBoundingClientRect().width;
      if (width > 0) offset = ((offset % width) + width) % width;
      trackElement.style.transform = `translateX(${-offset}px)`;
    }
    function animate(time: number) {
      const elapsed = last ? Math.min(time - last, 100) : 0;
      const target = motion.matches || hover || focus || pointerId !== null ? 0 : direction * 0.03;
      speed += (target - speed) * (1 - Math.exp(-elapsed / 250));
      if (!motion.matches && elapsed) {
        offset += speed * elapsed;
        updatePosition();
      }
      last = time;
      frame = requestAnimationFrame(animate);
    }
    function syncMotion() {
      if (motion.matches) {
        offset = 0;
        speed = 0;
        updatePosition();
      }
    }
    function pointerDown(event: PointerEvent) {
      if (motion.matches || !event.isPrimary || event.button !== 0) return;
      pointerId = event.pointerId;
      startX = previousX = event.clientX;
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
      if (event.pointerId === pointerId) pointerId = null;
    }
    function focusIn(event: FocusEvent) {
      focus = true;
      if (motion.matches || !(event.target instanceof HTMLAnchorElement)) return;
      viewport.scrollLeft = 0;
      const bounds = viewport.getBoundingClientRect();
      const link = event.target.getBoundingClientRect();
      if (link.left < bounds.left + 12) offset += link.left - bounds.left - 12;
      if (link.right > bounds.right - 12) offset += link.right - bounds.right + 12;
      updatePosition();
    }
    function focusOut(event: FocusEvent) {
      if (!(event.relatedTarget instanceof Node) || !viewport.contains(event.relatedTarget))
        focus = false;
    }
    const enter = () => (hover = true);
    const leave = () => (hover = false);
    viewport.addEventListener("pointerenter", enter);
    viewport.addEventListener("pointerleave", leave);
    viewport.addEventListener("pointerdown", pointerDown);
    viewport.addEventListener("pointermove", pointerMove);
    viewport.addEventListener("pointerup", pointerEnd);
    viewport.addEventListener("pointercancel", pointerEnd);
    viewport.addEventListener("lostpointercapture", pointerEnd);
    viewport.addEventListener("focusin", focusIn);
    viewport.addEventListener("focusout", focusOut);
    motion.addEventListener("change", syncMotion);
    frame = requestAnimationFrame(animate);
    return {
      destroy() {
        cancelAnimationFrame(frame);
        motion.removeEventListener("change", syncMotion);
        viewport.removeEventListener("pointerenter", enter);
        viewport.removeEventListener("pointerleave", leave);
        viewport.removeEventListener("pointerdown", pointerDown);
        viewport.removeEventListener("pointermove", pointerMove);
        viewport.removeEventListener("pointerup", pointerEnd);
        viewport.removeEventListener("pointercancel", pointerEnd);
        viewport.removeEventListener("lostpointercapture", pointerEnd);
        viewport.removeEventListener("focusin", focusIn);
        viewport.removeEventListener("focusout", focusOut);
      }
    };
  }

  function openProvider(event: MouseEvent, url: string) {
    if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey)
      return;
    event.preventDefault();
    void openExternal(url);
  }
</script>

{#snippet providerRow(items: typeof providers, clone: boolean)}
  {#each items as provider (provider.name)}
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
            style:width="{provider.w * 1.4}px"
            style:height="{provider.h * 1.4}px"
            style:--mark={`url('/landing/providers/${provider.mark}.svg')`}
          ></span>
        {:else}
          <span class="provider-text-mark" aria-hidden="true">{provider.name}</span>
        {/if}
      </a>
    </li>
  {/each}
{/snippet}

<section class="pt-2 pb-6 text-center" aria-labelledby="footer-version">
  <p id="footer-version" class="text-muted-foreground mb-4 scroll-mt-4 font-mono text-sm">
    CopySpeak v{VERSION}
  </p>
  <div class="flex flex-wrap justify-center gap-3">
    <Button
      class="cursor-pointer"
      href="https://copyspeak.vercel.app/"
      target="_blank"
      rel="noopener noreferrer"
      variant="outline"
      size="sm"
      onclick={(event) => openProvider(event, "https://copyspeak.vercel.app/")}
    >
      {$_("settings.about.website")}
    </Button>
    <Button
      class="cursor-pointer"
      variant="outline"
      size="sm"
      onclick={() => openExternal("https://github.com/ilyaizen/CopySpeak")}
    >
      {$_("settings.about.github")}
    </Button>
    <Button
      class="cursor-pointer"
      href="https://www.buymeacoffee.com/ilyaizen"
      target="_blank"
      rel="noopener noreferrer"
      variant="outline"
      size="sm"
      onclick={(event) => openProvider(event, "https://www.buymeacoffee.com/ilyaizen")}
    >
      {$_("settings.about.buyMeCoffee")}
    </Button>
  </div>
</section>
<section class="acknowledgments" aria-labelledby="acknowledgments-title">
  <h3 id="acknowledgments-title" class="text-muted-foreground text-center text-sm font-medium">
    {$_("settings.about.acknowledgments")}
  </h3>
  {#each [providers, localProviders] as items, index}
    <div
      class="marquee font-mono"
      role="group"
      aria-labelledby="acknowledgments-title"
      dir="ltr"
      use:marquee={index === 0 ? 1 : -1}
    >
      <div class="track">
        <ul class="provider-list">
          {@render providerRow(items, false)}
        </ul>
        <ul class="provider-list" aria-hidden="true">
          {@render providerRow(items, true)}
        </ul>
      </div>
    </div>
  {/each}
  <p
    class="text-muted-foreground mt-6 text-center font-mono text-[11px] tracking-[0.1em] uppercase"
  >
    {$_("settings.about.acknowledgmentsTagline")}
  </p>
</section>
<footer
  class="text-muted-foreground flex flex-nowrap items-center justify-between gap-2 px-2 pt-1 pb-6 font-mono text-[11px] tracking-[0.1em] uppercase"
  dir="ltr"
>
  <span class="whitespace-nowrap" aria-label="made with ♥ rust + robots"
    >made with <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      class="mx-[0.1em] inline-block size-[1em] align-[-0.12em]"
      aria-hidden="true"
      ><path
        d="M12 21 3.3 12.9C-1.4 8.5 4.9 1.3 10 5.3L12 7l2-1.7c5.1-4 11.4 3.2 6.7 7.6Z"
        fill="currentColor"
      /></svg
    > rust + robots</span
  >
  <span class="whitespace-nowrap"
    >by <a
      href="https://x.com/ilyaizen"
      target="_blank"
      rel="noopener noreferrer"
      class="hover:underline focus-visible:outline focus-visible:outline-2"
      onclick={(event) => openProvider(event, "https://x.com/ilyaizen")}>@ilyaizen</a
    >
    ({new Date().getFullYear()})</span
  >
</footer>

<style>
  .acknowledgments {
    padding: 24px 0;
  }
  .marquee {
    width: 100%;
    max-width: 480px;
    margin-inline: auto;
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
    padding: 8px 32px 8px 0;
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
    font-size: 20px;
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
