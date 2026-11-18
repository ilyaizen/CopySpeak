<script lang="ts">
  import Hero from "#lib/components/landing/hero.svelte";
  import Providers from "#lib/components/landing/providers.svelte";
  import Features from "#lib/components/landing/features.svelte";
  import Screenshots from "#lib/components/landing/screenshots.svelte";
  import Footer from "#lib/components/landing/footer.svelte";
  import { INTRO_AUDIO, INTRO_LINES } from "#lib/components/landing/intro-captions";

  const poemIntro = { audio: INTRO_AUDIO, lines: INTRO_LINES, staticWord: "buck" };
</script>

<!-- Legacy "back version" of the landing page with the original poem hero. -->
<Hero intro={poemIntro} />
<Screenshots />
<Features />
<Providers />
<Footer />
