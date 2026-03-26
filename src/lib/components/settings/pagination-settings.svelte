<script lang="ts">
  import SettingRow from "$lib/components/ui/setting-row/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import InfoTooltip from "$lib/components/ui/info-tooltip.svelte";
  import type { AppConfig } from "$lib/types";
  import { _ } from "svelte-i18n";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

  const PAGINATION_OPTIONS = [
    { value: "disabled", label: "Disabled" },
    { value: "200", label: "200 characters" },
    { value: "400", label: "400 characters" },
    { value: "600", label: "600 characters" },
    { value: "800", label: "800 characters" },
    { value: "1000", label: "1000 characters" },
    { value: "1200", label: "1200 characters" },
    { value: "1400", label: "1400 characters" },
    { value: "1600", label: "1600 characters" },
    { value: "1800", label: "1800 characters" },
    { value: "2000", label: "2000 characters" }
  ];

  let paginationValue = $derived(
    localConfig.pagination.enabled ? String(localConfig.pagination.fragment_size) : "disabled"
  );

  function handlePaginationChange(e: Event) {
    const target = e.target as HTMLSelectElement;
    const value = target.value;
    if (value === "disabled") {
      localConfig.pagination.enabled = false;
    } else {
      localConfig.pagination.enabled = true;
      localConfig.pagination.fragment_size = parseInt(value, 10);
    }
  }
</script>

<SettingRow
  label={$_("settings.pagination.enabled")}
  tooltip={$_("settings.pagination.description")}
>
  <Select
    options={PAGINATION_OPTIONS}
    value={paginationValue}
    onchange={handlePaginationChange}
    class="w-40"
  />
</SettingRow>
