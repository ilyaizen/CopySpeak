<script lang="ts">
  import { onMount } from "svelte";
  import RecentHistory from "$lib/components/recent-history.svelte";
  import { historyStore } from "$lib/stores/history-store.svelte";
  import { isTauri } from "$lib/services/tauri.js";
  import { _ } from "svelte-i18n";

  onMount(async () => {
    if (isTauri && historyStore.items.length === 0) {
      await historyStore.loadHistory();
    }
  });
</script>

<div class="w-full min-w-0">
  <RecentHistory limit={Infinity} />
</div>
