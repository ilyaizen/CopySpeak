<script lang="ts">
  import { _ } from "svelte-i18n";
  import { Download } from "@lucide/svelte";
  import Github from "#lib/components/icons/github.svelte";

  const DOWNLOAD_URL = "https://github.com/ilyaizen/CopySpeak/releases";
  const GITHUB_URL = "https://github.com/ilyaizen/CopySpeak";
</script>

<div class="coda">
  <section class="close">
    <div class="actions">
      <a class="btn primary" href={DOWNLOAD_URL}>
        <Download class="icon" />
        <span>{$_("landing.hero.download")}</span>
      </a>
      <a class="btn" href={GITHUB_URL}>
        <Github class="icon" />
        <span>{$_("landing.hero.github")}</span>
      </a>
    </div>
  </section>
</div>

<footer class="small">
  <a href={GITHUB_URL}>{$_("landing.footer.github")}</a>
  <span aria-hidden="true">·</span>
  <span>{$_("landing.footer.license")}</span>
</footer>
