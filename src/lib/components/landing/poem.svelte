<script lang="ts">
  const POEM = `He thought he kept the universe alone;
For all the voice in answer he could wake
Was but the mocking echo of his own
From some tree-hidden cliff across the lake.
Some morning from the boulder-broken beach
He would cry out on life, that what it wants
Is not its own love back in copy speech,
But counter-love, original response.
And nothing ever came of what he cried
Unless it was the embodiment that crashed
In the cliff's talus on the other side,
And then in the far distant water splashed,
But after a time allowed for it to swim,
Instead of proving human when it neared
And someone else additional to him,
As a great buck it powerfully appeared,
Pushing the crumpled water up ahead,
And landed pouring like a waterfall,
And stumbled through the rocks with horny tread,
And forced the underbrush—and that was all.`;
</script>

<!--
  Full-bleed colophon plate below the footer. Fixed dark palette on purpose
  (Nous-style) — it stays dark in both themes, so colors are hardcoded
  rather than theme tokens.
-->
<section class="relative isolate overflow-hidden bg-[#0a0f0c]">
  <img
    src="/deer-crossing-river.png"
    alt=""
    aria-hidden="true"
    class="absolute inset-0 h-full w-full object-cover"
  />
  <!-- soft veil behind the text so the verse stays crisp over the halftone water -->
  <div
    class="absolute inset-0"
    style="background: radial-gradient(ellipse 75% 65% at 50% 42%, rgb(10 15 12 / 0.78) 0%, rgb(10 15 12 / 0.4) 48%, rgb(10 15 12 / 0) 78%);"
  ></div>
  <div class="absolute inset-x-0 top-0 h-28 bg-gradient-to-b from-[#0a0f0c] to-transparent"></div>
  <div
    class="absolute inset-x-0 bottom-0 h-36 bg-gradient-to-t from-[#0a0f0c] to-transparent"
  ></div>

  <div class="relative mx-auto max-w-xl px-6 py-28 sm:py-36">
    <p class="font-mono text-[11px] font-semibold tracking-[0.4em] text-[#7e9187] uppercase">
      The Most of It
    </p>

    <p
      class="mt-10 text-xl leading-[1.8] whitespace-pre-line text-[#dce6df] italic sm:text-2xl lg:text-[1.7rem]"
      style="font-family: 'EB Garamond', Georgia, serif; text-shadow: 0 1px 14px rgb(5 8 6 / 0.6);"
    >
      {POEM}
    </p>

    <div class="mt-14">
      <div class="h-px w-10 bg-[#2a352e]"></div>
      <p class="mt-5 font-mono text-[11px] tracking-[0.3em] text-[#7e9187] uppercase">
        Robert Frost — Mountain Interval, 1916
      </p>
    </div>
  </div>
</section>
