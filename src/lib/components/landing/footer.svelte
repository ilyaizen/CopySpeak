<script lang="ts">
  import Github from "#lib/components/icons/github.svelte";
  import { VERSION } from "#lib/version";
  // import LanguageSwitcher from "./language-switcher.svelte";

  const GITHUB_URL = "https://github.com/ilyaizen/CopySpeak";

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

<footer class="border-border bg-muted/30 border-t px-6 py-8">
  <div class="mx-auto max-w-5xl">
    <div class="flex flex-col items-center justify-between gap-4 sm:flex-row">
      <div class="flex items-center gap-3">
        <img src="/app-logo.png" alt="CopySpeak" class="h-8 w-8" />
        <span class="text-foreground font-mono text-sm font-semibold">CopySpeak v{VERSION}</span>
      </div>

      <div class="flex items-center gap-6">
        <!-- <LanguageSwitcher /> -->

        <a
          href={GITHUB_URL}
          class="text-muted-foreground hover:text-foreground flex items-center gap-2 text-sm transition-colors"
        >
          <Github class="h-4 w-4" />
          GitHub
        </a>

        <a
          href="https://github.com/ilyaizen/CopySpeak/blob/main/LICENSE"
          class="text-muted-foreground hover:text-foreground text-sm transition-colors"
        >
          MIT License
        </a>
      </div>
    </div>

    <div class="text-muted-foreground mt-6 text-center text-xs sm:text-start">
      Made with Rust, Svelte, and AI.
    </div>

    <div class="border-border text-muted-foreground mt-8 border-t pt-8 text-center">
      <p class="font-mono text-xs tracking-wide uppercase">The Most of It</p>
      <p class="mt-1 text-xs">Robert Frost, 1916</p>
      <p class="mx-auto mt-6 max-w-md text-xs leading-relaxed whitespace-pre-line italic">
        {POEM}
      </p>
    </div>
  </div>
</footer>
