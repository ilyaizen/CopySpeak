<script lang="ts">
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";
  import { Clipboard, Download, Pause, Play } from "@lucide/svelte";
  import Github from "#lib/components/icons/github.svelte";
  import { VERSION } from "#lib/version";
  import { INTRO_AUDIO, INTRO_LINES } from "./intro-captions";
  import { groundParallax } from "#lib/utils/ground-parallax";

  const DOWNLOAD_URL = "https://github.com/ilyaizen/CopySpeak/releases";
  const GITHUB_URL = "https://github.com/ilyaizen/CopySpeak";

  type Phase = "idle" | "selected" | "copy1" | "copy2" | "speaking" | "done";

  // Each word carries its index in the whole intro, so highlighting and the caption window
  // run across line breaks.
  let count = 0;
  const LINES = INTRO_LINES.map((line) => line.map((w) => ({ ...w, index: count++ })));
  const WORDS = LINES.flat();
  // Static frame (no JS / reduced motion / before playback): "copy" lit.
  const STATIC_WORD = WORDS.findIndex((w) => w.text === "copy");

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

  const caption = $derived.by(() => {
    // Before the first word the HUD previews it; the static frame keeps its lit word.
    const i = live >= 0 ? live : phase === null ? STATIC_WORD : 0;
    const text = (from: number, to: number) =>
      WORDS.slice(Math.max(0, from), to)
        .map((w) => w.text)
        .join(" ");
    return { before: text(i - 2, i), now: WORDS[i].text, after: text(i + 1, i + 3) };
  });

  function wordClass(index: number): string {
    if (phase === null) return index === STATIC_WORD ? "w now" : "w";
    if (phase !== "speaking") return "w";
    if (index === live) return "w live";
    return index > live ? "w unsaid" : "w";
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
        <h1 class="promise" class:sel={selected} bind:this={promise}>
          {#each LINES as line, li (li)}
            <span class="line"
              >{#each line as word (word.index)}<span class={wordClass(word.index)}
                  >{word.text}</span
                >{word === line[line.length - 1] ? "" : " "}{/each}</span
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
      <div class="hud" aria-hidden="true">
        <span class="wave"><i></i><i></i><i></i><i></i><i></i><i></i><i></i></span>
        <p class="cap">
          {#if caption.before}<b>{caption.before}</b>{" "}{/if}<span class="now">{caption.now}</span
          >{#if caption.after}{" "}<b>{caption.after}</b>{/if}
        </p>
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
      <audio bind:this={audio} src={INTRO_AUDIO} preload="auto"></audio>
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
