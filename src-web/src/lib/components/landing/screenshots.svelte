<script lang="ts">
  import { _ } from "svelte-i18n";
</script>

<section class="border-border bg-background border-b px-6 py-16 sm:py-24">
  <div class="mx-auto max-w-5xl">
    <h2 class="text-foreground text-center font-mono text-2xl font-bold sm:text-3xl">
      {$_("landing.screenshots.title")}
    </h2>

    <p class="text-muted-foreground mt-2 text-center">
      {$_("landing.screenshots.subtitle")}
    </p>

    <div class="mt-12">
      <div class="border-border bg-muted/30 mx-auto max-w-3xl border p-2">
        <img src="/screen-v0.0.1.png" alt="CopySpeak Screenshot" class="w-full" />
      </div>
      <p class="text-muted-foreground mt-4 text-center text-sm">
        {$_("landing.screenshots.caption")}
      </p>
    </div>
  </div>
</section>
