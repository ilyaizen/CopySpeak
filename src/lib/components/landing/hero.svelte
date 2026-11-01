<script lang="ts">
  import { _ } from "svelte-i18n";
  import { Clipboard, Download } from "@lucide/svelte";
  import Github from "#lib/components/icons/github.svelte";
  import { VERSION } from "#lib/version";

  const DOWNLOAD_URL = "https://github.com/ilyaizen/CopySpeak/releases";
  const GITHUB_URL = "https://github.com/ilyaizen/CopySpeak";
</script>

<!-- Static frame of the copy-twice-then-speak illustration. The Frost lines are art, not UI copy, so they stay English in every locale. -->
<div class="wrap">
  <section class="hero">
    <div class="stage">
      <div class="win">
        <h1 class="promise">
          <span class="line">Pushing the crumpled water <span class="w now">up</span> ahead,</span>
          <span class="line">And landed pouring like a waterfall,</span>
          <span class="line">And stumbled through the rocks with horny tread,</span>
          <span class="line">And forced the underbrush&mdash;and that was all.</span>
        </h1>
        <div class="copied" aria-hidden="true">
          <span class="chip">
            <Clipboard class="icon" />
            {$_("landing.hero.chip")}
          </span>
          <span class="window-bar"><i></i></span>
        </div>
      </div>
      <div class="hud" aria-hidden="true">
        <span class="wave"><i></i><i></i><i></i><i></i><i></i><i></i><i></i></span>
        <p class="cap"><b>crumpled water</b> <span class="now">up</span> <b>ahead</b></p>
      </div>
      <p class="label stage-label">{$_("landing.hero.previewLabel")}</p>
    </div>

    <p class="lede">{$_("app.tagline")}</p>
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
    <p class="small meta">{$_("landing.hero.version", { values: { version: VERSION } })}</p>
  </section>
</div>
