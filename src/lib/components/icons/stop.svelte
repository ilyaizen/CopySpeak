<script lang="ts">
  import type { SVGAttributes } from "svelte/elements";
  let { class: className = "", ...rest }: Props = $props();

  interface Props extends SVGAttributes<SVGSVGElement> {
    class?: string;
  }
</script>

<!-- Material "stop" filled (google/material-design-icons, Apache-2.0) -->
<svg
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="currentColor"
  class={className}
  {...rest}
>
  <path d="M6 6h12v12H6z" />
</svg>
