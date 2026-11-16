<script lang="ts">
  import { _ } from "svelte-i18n";
</script>

<div class="wrap">
  <section class="section">
    <figure class="shot">
      <img src="/screen-0.3.0.png" width="1000" height="795" alt={$_("landing.screenshots.alt")} />
      <figcaption>{$_("landing.screenshots.caption")}</figcaption>
    </figure>
  </section>
</div>
