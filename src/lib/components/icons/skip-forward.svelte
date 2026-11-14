<script lang="ts">
  import type { SVGAttributes } from "svelte/elements";
  let { class: className = "", ...rest }: Props = $props();

  interface Props extends SVGAttributes<SVGSVGElement> {
    class?: string;
  }
</script>

<!-- Material "fast_forward" filled (google/material-design-icons, Apache-2.0) -->
<svg
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="currentColor"
  class={className}
  {...rest}
>
  <path d="M4 18l8.5-6L4 6v12zm9-12v12l8.5-6L13 6z" />
</svg>
