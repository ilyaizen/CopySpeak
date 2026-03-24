<script lang="ts">
  import { ModeWatcher } from "mode-watcher";
  import ThemeToggle from "$lib/components/theme-toggle.svelte";
  // Import i18n to ensure init() runs (English-only for now)
  // TODO: Re-enable full i18n when ready for multi-language support
  import "$lib/i18n";
  import "./+layout.css";

  let { children } = $props();
</script>

<svelte:head>
  <title>CopySpeak - AI Text-to-Speech for Windows</title>
</svelte:head>

<ModeWatcher />

<!-- TODO: Re-add dir={$isRtl ? "rtl" : "ltr"} when i18n is re-enabled -->
<div class="bg-background min-h-screen">
  <header class="border-border bg-background sticky top-0 z-50 border-b">
    <div class="mx-auto flex max-w-5xl items-center justify-between px-6 py-3">
      <a href="/" class="flex items-center gap-3">
        <img src="/app-logo.png" alt="CopySpeak Logo" class="h-8 w-8" />
        <span class="text-foreground font-mono text-lg font-semibold">CopySpeak</span>
      </a>

      <div class="flex items-center gap-4">
        <ThemeToggle />
      </div>
    </div>
  </header>

  <main>
    {@render children()}
  </main>
</div>
