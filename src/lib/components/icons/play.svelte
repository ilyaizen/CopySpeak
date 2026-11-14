<script lang="ts">
  import type { SVGAttributes } from "svelte/elements";
  let { class: className = "", ...rest }: Props = $props();

  interface Props extends SVGAttributes<SVGSVGElement> {
    class?: string;
  }
</script>

<!-- Material "play_arrow" filled (google/material-design-icons, Apache-2.0) -->
<svg
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="currentColor"
  class={className}
  {...rest}
>
  <path d="M8 5v14l11-7z" />
</svg>
