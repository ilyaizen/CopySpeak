<script lang="ts">
  import type { SVGAttributes } from "svelte/elements";
  let { class: className = "", ...rest }: Props = $props();

  interface Props extends SVGAttributes<SVGSVGElement> {
    class?: string;
  }
</script>

<!-- Material "fast_rewind" filled (google/material-design-icons, Apache-2.0) -->
<svg
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="currentColor"
  class={className}
  {...rest}
>
  <path d="M11 18V6l-8.5 6 8.5 6zm.5-6l8.5 6V6l-8.5 6z" />
</svg>
