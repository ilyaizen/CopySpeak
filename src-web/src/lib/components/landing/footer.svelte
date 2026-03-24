<script lang="ts">
  // TODO: Re-enable i18n when ready for multi-language support
  // import { _ } from "svelte-i18n";
  import { Github } from "@lucide/svelte";
  // TODO: Re-enable LanguageSwitcher when i18n is re-enabled
  // import LanguageSwitcher from "./language-switcher.svelte";

  const GITHUB_URL = "https://github.com/ilyaizen/CopySpeak";
</script>

<footer class="border-border bg-muted/30 border-t px-6 py-8">
  <div class="mx-auto max-w-5xl">
    <div class="flex flex-col items-center justify-between gap-4 sm:flex-row">
      <div class="flex items-center gap-3">
        <img src="/app-logo.png" alt="CopySpeak" class="h-8 w-8" />
        <span class="text-foreground font-mono text-sm font-semibold">CopySpeak</span>
      </div>

      <div class="flex items-center gap-6">
        <!-- TODO: Re-enable LanguageSwitcher when i18n is re-enabled -->
        <!-- <LanguageSwitcher /> -->
        <span class="text-muted-foreground text-sm">English</span>

        <a
          href={GITHUB_URL}
          class="text-muted-foreground hover:text-foreground flex items-center gap-2 text-sm transition-colors"
        >
          <Github class="h-4 w-4" />
          GitHub
        </a>

        <a
          href="https://github.com/ilyaizen/CopySpeak/blob/main/LICENCE.txt"
          class="text-muted-foreground hover:text-foreground text-sm transition-colors"
        >
          MIT License
        </a>
      </div>
    </div>

    <div class="text-muted-foreground mt-6 text-center text-xs sm:text-left">
      Made with Rust, Svelte, and AI.
    </div>
  </div>
</footer>
