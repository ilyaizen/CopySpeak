<script lang="ts">
  import { _ } from "svelte-i18n";
  import { Info } from "@lucide/svelte";
  import * as Tooltip from "#lib/components/ui/tooltip/index.js";

  let { text, detail }: { text: string; detail?: string } = $props();
</script>

<!-- Provider delay 100 ms: Handy-style snappy tooltips; only the delay is
     set here, so each instance does not need its own provider wrapper. -->
<Tooltip.Provider delayDuration={100}>
  <Tooltip.Root>
    <Tooltip.Trigger
      class="text-muted-foreground hover:text-foreground inline-flex items-center justify-center transition-colors"
      aria-label={$_("common.moreInfo")}
    >
      <Info class="h-3.5 w-3.5" />
    </Tooltip.Trigger>
    <Tooltip.Content>
      <p class="max-w-md text-sm whitespace-normal">{text}</p>
      {#if detail}
        <p class="mt-1 max-w-md text-sm whitespace-normal">{detail}</p>
      {/if}
    </Tooltip.Content>
  </Tooltip.Root>
</Tooltip.Provider>
