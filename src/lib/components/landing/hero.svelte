<script lang="ts">
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";
  import { Clipboard, Download, Pause, Play } from "@lucide/svelte";
  import Github from "#lib/components/icons/github.svelte";
  import { VERSION } from "#lib/version";
  import { HERO_AUDIO, HERO_LINES, HERO_STATIC_WORD } from "./hero-captions";
  import type { IntroWord } from "./intro-captions";
  import { groundParallax } from "#lib/utils/ground-parallax";
  import { captionGlide } from "#lib/utils/caption-glide";
  import type { CaptionWord } from "#lib/models/captions.js";

  const DOWNLOAD_URL = "https://github.com/ilyaizen/CopySpeech/releases";
  const GITHUB_URL = "https://github.com/ilyaizen/CopySpeech";

  type Phase = "idle" | "selected" | "copy1" | "copy2" | "speaking" | "done";

  // Each word carries its index in the whole intro, so the glide and the speaking state
  // run across line breaks.
  type Intro = { audio: string; lines: IntroWord[][]; staticWord: string };
  let { intro = { audio: HERO_AUDIO, lines: HERO_LINES, staticWord: HERO_STATIC_WORD } }: {
    intro?: Intro;
  } = $props();

  let count = 0;
  // The intro is fixed per route, so reading it once at setup is intended.
  // svelte-ignore state_referenced_locally
  const LINES = intro.lines.map((line) => line.map((w) => ({ ...w, index: count++ })));
  const WORDS = LINES.flat();
  // captionGlide reads offset as the word's data-word-offset, which is the intro index.
  const GLIDE_WORDS: CaptionWord[] = WORDS.map((w) => ({
    text: w.text,
    offset: w.index,
    start: w.start,
    end: w.end,
    phrase: 0
  }));
  // Static frame (no JS / reduced motion / before playback): the static word lit.
  const STATIC_WORD = WORDS.findIndex((w) => w.text === intro.staticWord);

  let stage: HTMLDivElement;
  let promise: HTMLHeadingElement;
  let audio: HTMLAudioElement;

  let phase = $state<Phase | null>(null);
  let live = $state(-1);
  let chip = $state("Copied ×2");
  let selected = $state(false);
  let playing = $state(false);
  // Set once the audio has played: browsers then allow play() from a timer, so a replay can
  // run the copy illustration first. Before that, play() must start inside a click.
  let unlocked = false;

  // The glide lights the word being spoken. Its pill holds on pause and clears on each phase
  // change (the resetKey), so a replay starts clean.
  const glideIndex = $derived(phase === null ? STATIC_WORD : phase === "speaking" ? live : -1);

  function wordClass(index: number): string {
    return phase === "speaking" && index > live ? "w unsaid" : "w";
  }

  function indexAt(ms: number): number {
    let i = -1;
    for (let j = 0; j < WORDS.length && WORDS[j].start <= ms; j++) i = j;
    return i;
  }

  // Copied highlight: one Range over the whole intro, so spaces and wrapped lines paint like a
  // real selection. Per-word backgrounds leave gaps; .sel is only the fallback without the API.
  // Resolved in onMount: select() only runs client-side, after mount.
  let highlights: HighlightRegistry | null = null;
  function select(on: boolean) {
    if (highlights) {
      if (on) {
        const r = document.createRange();
        r.selectNodeContents(promise);
        highlights.set("copied", new Highlight(r));
      } else highlights.delete("copied");
    } else selected = on;
  }

  let timers: ReturnType<typeof setTimeout>[] = [];
  let frame = 0;

  function at(ms: number, fn: () => void) {
    timers.push(setTimeout(fn, ms));
  }

  function halt() {
    timers.forEach(clearTimeout);
    timers = [];
    cancelAnimationFrame(frame);
    audio.pause();
    playing = false;
  }

  // The illustration: select the poem, copy it twice in quick succession, then speak it.
  function run() {
    halt();
    live = -1;
    playing = true;
    phase = "idle";
    audio.currentTime = 0;
    at(400, () => {
      phase = "selected";
      select(true);
    });
    at(1100, () => {
      chip = "Copied";
      phase = "copy1";
    });
    at(1500, () => {
      chip = "Copied ×2";
      phase = "copy2";
    });
    at(2200, speak);
  }

  // Plays from the audio's current position; the highlight reads audio.currentTime, so it
  // cannot drift from the recording.
  async function speak() {
    select(false);
    phase = "speaking";
    playing = true;
    try {
      await audio.play();
    } catch (e) {
      playing = false;
      // Autoplay blocked: hold the frame and start on the visitor's first click anywhere.
      // A pause during the pending play() rejects too (AbortError) and must stay paused.
      if (e instanceof DOMException && e.name === "NotAllowedError")
        document.addEventListener("click", resume, { once: true });
      return;
    }
    unlocked = true;
    const tick = () => {
      live = indexAt(audio.currentTime * 1000);
      if (audio.ended) {
        playing = false;
        live = -1;
        phase = "done";
      } else if (playing) frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
  }

  function resume() {
    if (!playing && phase === "speaking") speak();
  }

  function toggle() {
    if (playing) halt();
    else if (phase === "speaking") speak();
    else if (unlocked) run();
    else {
      audio.currentTime = 0;
      speak();
    }
  }

  onMount(() => {
    highlights = "highlights" in CSS ? CSS.highlights : null;

    // The landing root is the scroll container, not the window.
    const scroller = stage.closest<HTMLElement>(".landing");
    const cleanup = scroller ? groundParallax(scroller, scroller) : null;

    if (!matchMedia("(prefers-reduced-motion: reduce)").matches) run();

    return () => {
      cleanup?.();
      halt();
      document.removeEventListener("click", resume);
      highlights?.delete("copied");
    };
  });
</script>

<!-- Copy-twice-then-speak illustration: the poem is copied twice, then the recording plays
     with its caption timings driving the highlight. The intro lines and the chip states are
     art, not UI copy, so they stay English in every locale. -->
<div class="wrap">
  <section class="hero">
    <div class="stage" class:paused={!playing} data-phase={phase ?? undefined} bind:this={stage}>
      <div class="win">
        <h1
          class="promise"
          class:sel={selected}
          bind:this={promise}
          use:captionGlide={{
            words: GLIDE_WORDS,
            activeIndex: glideIndex,
            animated: true,
            resetKey: phase
          }}
        >
          {#each LINES as line, li (li)}
            <span class="line"
              >{#each line as word (word.index)}<span
                  class={wordClass(word.index)}
                  data-word-offset={word.index}>{word.text}</span
                >{word === line[line.length - 1] || word.text.endsWith("—") ? "" : " "}{/each}</span
            >
          {/each}
        </h1>
        <div class="copied" aria-hidden="true">
          {#key chip}
            <span class="chip">
              <Clipboard class="icon" />
              {chip}
            </span>
          {/key}
          <span class="window-bar"><i></i></span>
        </div>
      </div>
      <p class="label stage-label">{$_("landing.hero.previewLabel")}</p>
      <div class="demo-controls">
        <button class="demo-toggle" type="button" onclick={toggle}>
          {#if playing}
            <Pause class="icon" />
            {$_("landing.hero.pauseDemo")}
          {:else}
            <Play class="icon" />
            {$_("landing.hero.playDemo")}
          {/if}
        </button>
      </div>
      <audio bind:this={audio} src={intro.audio} preload="auto"></audio>
    </div>

    <p class="lede">{$_("app.tagline")}</p>
    <div class="actions">
      <a class="btn primary" href={DOWNLOAD_URL}>
        <Download class="icon" />
        <span>{$_("landing.hero.download")}</span>
      </a>
      <a class="btn" href={GITHUB_URL}>
        <Github class="icon" />
        <span>{$_("landing.hero.github")}</span>
      </a>
    </div>
    <p class="small meta">{$_("landing.hero.version", { values: { version: VERSION } })}</p>
  </section>
</div>
