<script lang="ts">
  import type { SVGAttributes } from "svelte/elements";
  let { class: className = "", ...rest }: Props = $props();

  interface Props extends SVGAttributes<SVGSVGElement> {
    class?: string;
  }
</script>

<!-- Material "replay" filled (google/material-design-icons, Apache-2.0) -->
<svg
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="currentColor"
  class={className}
  {...rest}
>
  <path
    d="M12 5V1L7 6l5 5V7c3.31 0 6 2.69 6 6s-2.69 6-6 6-6-2.69-6-6H4c0 4.42 3.58 8 8 8s8-3.58 8-8-3.58-8-8-8z"
  />
</svg>
