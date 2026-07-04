<script lang="ts">
  import { _ } from "svelte-i18n";
  import { Download } from "@lucide/svelte";
  import Github from "$lib/components/icons/github.svelte";
  import { Button } from "$lib/components/ui/button/index.js";
  import { VERSION } from "$lib/version";

  const DOWNLOAD_URL = "https://github.com/ilyaizen/CopySpeak/releases";
  const GITHUB_URL = "https://github.com/ilyaizen/CopySpeak";
</script>

<section class="border-border bg-background border-b px-6 py-16 sm:py-24">
  <div class="mx-auto max-w-4xl text-center">
    <div class="mb-8 flex justify-center">
      <img src="/app-logo.png" alt="CopySpeak Logo" class="h-24 w-24 sm:h-32 sm:w-32" />
    </div>

    <h1 class="text-foreground font-mono text-4xl font-bold tracking-tight sm:text-6xl">
      {$_("landing.hero.title")}
    </h1>

    {#if $_("landing.hero.subtitle")}
      <p class="text-primary mt-3 font-mono text-sm font-medium tracking-wide uppercase">
        {$_("landing.hero.subtitle")}
      </p>
    {/if}

    <p class="text-muted-foreground mt-4 text-base">
      {$_("landing.hero.description")}
    </p>

    <div class="mt-8 flex flex-col items-center justify-center gap-4 sm:flex-row">
      <Button href={DOWNLOAD_URL} size="lg" class="gap-2 px-8">
        <Download class="h-5 w-5" />
        {$_("landing.hero.download")}
      </Button>

      <Button href={GITHUB_URL} variant="outline" size="lg" class="gap-2 px-8">
        <Github class="h-5 w-5" />
        {$_("landing.hero.github")}
      </Button>
    </div>

    <p class="text-muted-foreground mt-6 text-sm">
      {$_("landing.hero.version", { values: { version: VERSION } })}
    </p>
  </div>
</section>
