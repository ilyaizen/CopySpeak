<script lang="ts">
  import { _ } from "svelte-i18n";
</script>

<div class="wrap">
  <section class="section">
    <figure class="shot">
      <img src="/screen-0.2.13.png" width="1000" height="562" alt={$_("landing.screenshots.alt")} />
      <figcaption>{$_("landing.screenshots.caption")}</figcaption>
    </figure>
  </section>
</div>
