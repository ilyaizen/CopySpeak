<script lang="ts">
  import { onMount } from "svelte";
  import { _ } from "svelte-i18n";
  import { Clipboard, Download, Pause, Play, Square, Volume2 } from "@lucide/svelte";
  import Github from "#lib/components/icons/github.svelte";
  import { VERSION } from "#lib/version";
  import { INTRO_AUDIO, INTRO_LINES } from "./intro-captions";

  const DOWNLOAD_URL = "https://github.com/ilyaizen/CopySpeak/releases";
  const GITHUB_URL = "https://github.com/ilyaizen/CopySpeak";

  type Phase = "idle" | "selected" | "copy1" | "copy2" | "speaking" | "done";

  // Each word carries its index in the whole poem, so highlighting and the caption window
  // run across line breaks.
  let count = 0;
  const LINES = INTRO_LINES.map((line) => line.map((w) => ({ ...w, index: count++ })));
  const WORDS = LINES.flat();
  const LAST_END = WORDS[WORDS.length - 1].end;
  // Static frame (no JS / reduced motion): "the" in the first line lit.
  const STATIC_WORD = WORDS.findIndex((w) => w.text === "the");

  let stage: HTMLDivElement;
  let promise: HTMLHeadingElement;
  let audio: HTMLAudioElement;

  let phase = $state<Phase | null>(null);
  let live = $state(-1);
  let chip = $state("Copied ×2");
  let selected = $state(false);
  let paused = $state(false);
  let listening = $state(false);
  let animated = $state(false);

  const caption = $derived.by(() => {
    const i = live >= 0 ? live : STATIC_WORD;
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

  // Copied highlight: one Range over the whole poem, so spaces and wrapped lines paint like a
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
  let running = false;

  function at(ms: number, fn: () => void) {
    timers.push(setTimeout(fn, ms));
  }

  function clear() {
    timers.forEach(clearTimeout);
    timers = [];
    cancelAnimationFrame(frame);
  }

  function reset() {
    live = -1;
    select(false);
  }

  // Speaking clock: the audio's own position when listening, wall time for the silent loop.
  // Both read the same caption timings, so the silent demo keeps the recording's rhythm.
  function speak(clock: () => number, onEnd: () => void) {
    select(false);
    phase = "speaking";
    const tick = () => {
      const ms = clock();
      live = indexAt(ms);
      if (ms >= LAST_END + 300) onEnd();
      else frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
  }

  function finish() {
    reset();
    phase = "done";
    running = true;
    at(2600, cycle);
  }

  function cycle() {
    clear();
    reset();
    phase = "idle";
    at(600, () => {
      phase = "selected";
      select(true);
    });
    at(1200, () => {
      chip = "Copied";
      phase = "copy1";
    });
    at(1950, () => {
      chip = "Copied ×2";
      phase = "copy2";
    });
    at(2500, () => {
      const t0 = performance.now();
      speak(() => performance.now() - t0, finish);
    });
  }

  function start() {
    if (!running && !paused && !listening) {
      running = true;
      cycle();
    }
  }

  function stop() {
    running = false;
    clear();
    reset();
    phase = null;
  }

  function stopListening() {
    audio.pause();
    listening = false;
    stop();
    start();
  }

  // play() must start inside the click: browsers block audio started from a later timer,
  // so Listen skips the copy phases and goes straight to speaking.
  async function listen() {
    if (listening) return stopListening();
    stop();
    listening = true;
    audio.currentTime = 0;
    try {
      await audio.play();
    } catch {
      listening = false;
      start();
      return;
    }
    speak(
      () => audio.currentTime * 1000,
      () => {
        listening = false;
        if (paused) stop();
        else finish();
      }
    );
  }

  function togglePause() {
    paused = !paused;
    if (paused) stop();
    else start();
  }

  onMount(() => {
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const cleanups: (() => void)[] = [];
    highlights = "highlights" in CSS ? CSS.highlights : null;

    // Ground parallax: the fixed texture layer drifts up at a fraction of scroll speed,
    // clamped inside its oversize headroom so no edge ever shows. The .landing root is the
    // scroll container, not the window. Reduced motion leaves --bg-shift unset.
    const scroller = stage.closest<HTMLElement>(".landing");
    if (scroller && !reduced) {
      const FACTOR = 0.12; // texture moves at 12% of scroll speed
      const MAX = 0.145; // share of viewport height; the layer carries 15% headroom per edge
      let queued = false;
      const update = () => {
        queued = false;
        const shift = Math.max(
          0,
          Math.min(scroller.scrollTop * FACTOR, scroller.clientHeight * MAX)
        );
        scroller.style.setProperty("--bg-shift", `${(-shift).toFixed(1)}px`);
      };
      const queue = () => {
        if (!queued) {
          queued = true;
          requestAnimationFrame(update);
        }
      };
      scroller.addEventListener("scroll", queue, { passive: true });
      addEventListener("resize", queue);
      update();
      cleanups.push(() => {
        scroller.removeEventListener("scroll", queue);
        removeEventListener("resize", queue);
        scroller.style.removeProperty("--bg-shift");
      });
    }

    if (!reduced) {
      animated = true;
      const io = new IntersectionObserver(
        (entries) => {
          if (listening) return;
          if (entries[0].isIntersecting && !document.hidden) start();
          else stop();
        },
        { threshold: 0.35 }
      );
      io.observe(stage);
      const onVisibility = () => {
        if (document.hidden && !listening) stop();
      };
      document.addEventListener("visibilitychange", onVisibility);
      cleanups.push(() => {
        io.disconnect();
        document.removeEventListener("visibilitychange", onVisibility);
      });
    }

    return () => {
      cleanups.forEach((fn) => fn());
      clear();
      audio.pause();
      highlights?.delete("copied");
    };
  });
</script>

<!-- Copy-twice-then-speak illustration. The silent loop runs on the real recording's caption
     timings; Listen plays that recording with the same word highlighting. The Shakespeare
     lines and the chip states are art, not UI copy, so they stay English in every locale. -->
<div class="wrap">
  <section class="hero">
    <div class="stage" class:paused data-phase={phase ?? undefined} bind:this={stage}>
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
          <span class="chip">
            <Clipboard class="icon" />
            {chip}
          </span>
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
        <button class="demo-toggle" type="button" aria-pressed={listening} onclick={listen}>
          {#if listening}
            <Square class="icon" />
            {$_("landing.hero.stopListening")}
          {:else}
            <Volume2 class="icon" />
            {$_("landing.hero.listen")}
          {/if}
        </button>
        {#if animated && !listening}
          <button class="demo-toggle" type="button" onclick={togglePause}>
            {#if paused}
              <Play class="icon" />
              {$_("landing.hero.playDemo")}
            {:else}
              <Pause class="icon" />
              {$_("landing.hero.pauseDemo")}
            {/if}
          </button>
        {/if}
      </div>
      <audio bind:this={audio} src={INTRO_AUDIO} preload="none"></audio>
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
