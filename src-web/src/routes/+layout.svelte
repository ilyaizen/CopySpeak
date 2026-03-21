<script lang="ts">
  import { ModeWatcher } from "mode-watcher";
  import { _ } from "svelte-i18n";
  import ThemeToggle from "$lib/components/theme-toggle.svelte";
  import { locale } from "$lib/i18n";
  import { isRtl } from "$lib/i18n/store";
  import "./+layout.css";

  let { children } = $props();
</script>

<svelte:head>
  <title>CopySpeak - AI Text-to-Speech for Windows</title>
</svelte:head>

<ModeWatcher />

<div class="bg-background min-h-screen" dir={$isRtl ? "rtl" : "ltr"}>
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
