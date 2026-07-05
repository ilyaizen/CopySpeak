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

  const motionDisabled = $derived(disableMotion);
  const resolvedClassName = $derived(`motion-wrapper${className ? ` ${className}` : ""}`);
</script>

{#key key}
  <div class={resolvedClassName} class:motion-disabled={motionDisabled} {...rest}>
    {@render children()}
  </div>
{/key}
