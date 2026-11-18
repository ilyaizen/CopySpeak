<script lang="ts">
  import { Download } from "@lucide/svelte";
  import Github from "#lib/components/icons/github.svelte";

  const DOWNLOAD_URL = "https://github.com/ilyaizen/CopySpeech/releases";
  const GITHUB_URL = "https://github.com/ilyaizen/CopySpeech";
  const COFFEE_URL = "https://www.buymeacoffee.com/ilyaizen";
  const AUTHOR_URL = "https://x.com/ilyaizen";
  const POEM_URL = "https://allpoetry.com/poem/14326574-The-Most-Of-It-by-Robert-Frost";
</script>

<div class="coda">
  <section class="close">
    <div class="actions">
      <a class="btn primary" href={DOWNLOAD_URL}>
        <Download class="icon" />
        <span>Download</span>
      </a>
      <a class="btn" href={GITHUB_URL}>
        <Github class="icon" />
        <span>GitHub</span>
      </a>
    </div>
  </section>
</div>

<!-- Same credit block as the app's About page: links, then the maker line. -->
<footer>
  <div class="credit-links">
    <a class="btn" href={COFFEE_URL} target="_blank" rel="noopener noreferrer"> Buy Me a Coffee </a>
  </div>
  <p class="credit-line">
    <span
      >made with <svg
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 24 24"
        class="heart"
        aria-label="♥"
        role="img"
        ><path
          d="M12 21 3.3 12.9C-1.4 8.5 4.9 1.3 10 5.3L12 7l2-1.7c5.1-4 11.4 3.2 6.7 7.6Z"
          fill="currentColor"
        /></svg
      > rust + robots</span
    >
    <span
      >by <a href={AUTHOR_URL} target="_blank" rel="noopener noreferrer">@ilyaizen</a>
      ({new Date().getFullYear()})</span
    >
  </p>
  <p class="legal">
    Name inspired by
    <a href={POEM_URL} target="_blank" rel="noopener noreferrer">“The Most of It” by Robert Frost</a
    > (1942)
  </p>
  <p class="legal">MIT License</p>
</footer>
