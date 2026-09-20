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
    // TEMP (2026-09-20): page slide-up animation globally disabled. Flip back
    // to `false` to re-enable — the keyframes in +layout.css and this prop
    // stay in place on purpose, this is not dead code.
    disableMotion = true,
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
