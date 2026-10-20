<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import EffectsPage from "#lib/components/effects-page.svelte";
  import { invoke } from "#lib/services/tauri";
  import type { AppConfig } from "#lib/types";

  // Effects live behind Experimental. With it off this route has nothing to show,
  // so it returns to Home. Turning Experimental on keeps the page reachable.
  let experimental = $state<boolean | null>(null);

  onMount(async () => {
    try {
      const config = await invoke<AppConfig>("get_config");
      experimental = config.general.experimental_enabled === true;
    } catch {
      experimental = false;
    }
    if (experimental === false) goto("/", { replaceState: true });
  });
</script>

{#if experimental === true}
  <EffectsPage />
{/if}
