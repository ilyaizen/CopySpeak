<script lang="ts">
  import { toggleMode } from "mode-watcher";
  import { Button } from "$lib/components/ui/button/index.js";
  import SunIcon from "@lucide/svelte/icons/sun";
  import MoonIcon from "@lucide/svelte/icons/moon";
</script>

<div class="flex items-center justify-center">
  <Button
    onclick={toggleMode}
    variant="outline"
    size="icon"
    class="hover:bg-primary/10 hover:border-primary/50 rounded-sm border-2 px-3 py-2 transition-colors duration-150"
    aria-label="Toggle theme"
  >
    <SunIcon class="h-5 w-5 scale-100 rotate-0 transition-all dark:scale-0 dark:-rotate-90" />
    <MoonIcon
      class="absolute h-5 w-5 scale-0 rotate-90 transition-all dark:scale-100 dark:rotate-0"
    />
    <span class="sr-only">Toggle theme</span>
  </Button>
</div>
