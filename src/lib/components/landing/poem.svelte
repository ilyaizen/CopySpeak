<script lang="ts">
  const STANZAS: readonly string[] = [
    `He thought he kept the universe alone;
For all the voice in answer he could wake
Was but the mocking echo of his own
From some tree-hidden cliff across the lake.`,
    `Some morning from the boulder-broken beach
He would cry out on life, that what it wants
Is not its own love back in copy speech,
But counter-love, original response.`,
    `And nothing ever came of what he cried
Unless it was the embodiment that crashed
In the cliff's talus on the other side,
And then in the far distant water splashed,`,
    `But after a time allowed for it to swim,
Instead of proving human when it neared
And someone else additional to him,
As a great buck it powerfully appeared,`,
    `Pushing the crumpled water up ahead,
And landed pouring like a waterfall,
And stumbled through the rocks with horny tread,
And forced the underbrush—and that was all.`
  ];

  // 3 stanzas open the left column, 2 close the right one above the colophon,
  // so reading order stays linear and both columns end at similar depth.
  const OPENING_STANZAS = STANZAS.slice(0, 3);
  const CLOSING_STANZAS = STANZAS.slice(3);

  // Theme pairs: light keys off the deer plate's pale halftone (#c5e0db),
  // dark keeps the established #0a0f0c family.
  const VERSE_CLASS =
    "verse text-xl leading-[1.7] whitespace-pre-line italic sm:text-2xl xl:text-xl text-[#26332d] dark:text-[#dce6df]";
  const EYEBROW_CLASS =
    "font-mono text-[13px] font-semibold tracking-[0.4em] uppercase lg:text-sm text-[#26332d] dark:text-[#dce6df]";
  const COLOPHON_CLASS =
    "mt-5 font-mono text-[11px] tracking-[0.3em] uppercase text-[#5e7a70] dark:text-[#7e9187]";
</script>

<!--
  Full-bleed colophon plate below the footer. The deer image stretches behind
  the verse in BOTH themes (theme-swapped assets, stag on the left); the 18rem
  slot on lg+ stays empty on purpose — the stag occupies it visually and the
  verse column keeps its indent.
-->
<section class="relative isolate overflow-hidden bg-[#c5e0db] dark:bg-[#0a0f0c]">
  <img
    src="/deer-crossing-river-light.png"
    alt=""
    aria-hidden="true"
    class="absolute inset-0 h-full w-full object-cover dark:hidden"
    style="object-position: 30% 50%;"
  />
  <img
    src="/deer-crossing-river.png"
    alt=""
    aria-hidden="true"
    class="absolute inset-0 hidden h-full w-full object-cover dark:block"
    style="object-position: 30% 50%; filter: brightness(1.5) saturate(1.05);"
  />

  <div class="absolute inset-0 bg-[#c5e0db]/50 dark:bg-[#0a0f0c]/30"></div>
  <div
    class="absolute inset-x-0 top-0 h-28 bg-gradient-to-b from-[#c5e0db] to-transparent dark:from-[#0a0f0c]"
  ></div>
  <div
    class="absolute inset-x-0 bottom-0 h-36 bg-gradient-to-t from-[#c5e0db] to-transparent dark:from-[#0a0f0c]"
  ></div>

  <div class="relative mx-auto max-w-7xl px-6 py-24 sm:py-32">
    <div class="grid gap-10 lg:grid-cols-[18rem_minmax(0,1fr)] lg:gap-16">
      <!-- Deliberate whitespace slot on lg+: the stag in the image fills it -->
      <div class="hidden lg:block" aria-hidden="true"></div>

      <div>
        <p class={EYEBROW_CLASS}>The Most of It</p>

        <div class="mt-12 flex flex-col gap-10 xl:grid xl:grid-cols-2 xl:items-start xl:gap-x-14">
          <div class="space-y-6">
            {#each OPENING_STANZAS as stanza}
              <p class={VERSE_CLASS}>
                {stanza}
              </p>
            {/each}
          </div>

          <div>
            <div class="space-y-6">
              {#each CLOSING_STANZAS as stanza}
                <p class={VERSE_CLASS}>
                  {stanza}
                </p>
              {/each}
            </div>

            <div class="mt-14">
              <div class="h-px w-10 bg-[#a9c4bc] dark:bg-[#2a352e]"></div>
              <p class={COLOPHON_CLASS}>Robert Frost — Mountain Interval, 1916</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</section>

<style>
  /* Pale lift in light mode so ink stays crisp over darker water patches;
     dark mode keeps the v2 shadow. */
  .verse {
    font-family: "EB Garamond", Georgia, serif;
    text-shadow: 0 1px 14px rgb(250 253 252 / 0.65);
  }
  :global(.dark) .verse {
    text-shadow: 0 1px 14px rgb(5 8 6 / 0.6);
  }
</style>
