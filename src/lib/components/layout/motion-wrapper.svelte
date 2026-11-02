<script lang="ts">
  import { afterNavigate } from "$app/navigation";
  import type { Snippet } from "svelte";

  let key = $state(0);
  afterNavigate(() => {
    key++;
  });

  interface MotionWrapperProps {
    children: Snippet;
    disableMotion?: boolean;
    class?: string;
  }

  let {
    children,
    disableMotion = false,
    class: className = "",
    ...rest
  }: MotionWrapperProps = $props();

  const resolvedClassName = $derived(`motion-wrapper${className ? ` ${className}` : ""}`);
</script>

{#key key}
  <div class={resolvedClassName} class:motion-disabled={disableMotion} {...rest}>
    {@render children()}
  </div>
{/key}
