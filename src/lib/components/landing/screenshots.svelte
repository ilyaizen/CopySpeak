<script lang="ts">
  import { _ } from "svelte-i18n";
</script>

<div class="wrap">
  <section class="section">
    <figure class="shot">
      <img
        class="shot-light"
        src="/screen-0.3.1-light.png"
        width="1000"
        height="794"
        alt={$_("landing.screenshots.alt")}
      />
      <img
        class="shot-dark"
        src="/screen-0.3.1-dark.png"
        width="1000"
        height="794"
        alt={$_("landing.screenshots.alt")}
      />
      <figcaption>{$_("landing.screenshots.caption")}</figcaption>
    </figure>
  </section>
</div>
