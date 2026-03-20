<script lang="ts">
  import { Info } from "@lucide/svelte";
  import * as Tooltip from "$lib/components/ui/tooltip/index.js";

  let { text }: { text: string } = $props();
</script>

<Tooltip.Root>
  <Tooltip.Trigger
    class="text-muted-foreground hover:text-foreground inline-flex items-center justify-center transition-colors"
    aria-label="More info"
  >
    <Info class="h-3.5 w-3.5" />
  </Tooltip.Trigger>
  <Tooltip.Content>
    <p class="max-w-md text-sm whitespace-normal">{text}</p>
  </Tooltip.Content>
</Tooltip.Root>
