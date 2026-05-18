<script lang="ts">
  import PlayPage from "$lib/components/play-page.svelte";
  import Hero from "$lib/components/landing/hero.svelte";
  import Features from "$lib/components/landing/features.svelte";
  import Screenshots from "$lib/components/landing/screenshots.svelte";
  import Footer from "$lib/components/landing/footer.svelte";
</script>

{#if import.meta.env.VITE_IS_VERCEL}
  <Hero />
  <Features />
  <Screenshots />
  <Footer />
{:else}
  <div class="mx-auto flex min-h-0 max-w-6xl flex-1 flex-col">
    <PlayPage />
  </div>
{/if}
