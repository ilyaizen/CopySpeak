<script lang="ts">
  import SettingRow from "$lib/components/ui/setting-row/index.js";
  import { Select } from "$lib/components/ui/select/index.js";
  import { _ } from "svelte-i18n";
  import { getSupportedLocales } from "$lib/i18n/utils";
  import type { AppConfig, SupportedLocale } from "$lib/types";

  let {
    localConfig = $bindable()
  }: {
    localConfig: AppConfig;
  } = $props();

  const appearanceOptions = [
    { value: "system", label: "System" },
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" }
  ];

  const localeOptions = getSupportedLocales();

  function handleThemeChange(e: Event) {
    localConfig.general.appearance = (e.target as HTMLSelectElement).value as
      | "system"
      | "light"
      | "dark";
  }

  function handleLocaleChange(e: Event) {
    localConfig.general.locale = (e.target as HTMLSelectElement).value as SupportedLocale;
  }
</script>

<div class="space-y-4">
  <SettingRow
    label={$_("settings.general.language")}
    tooltip={$_("settings.general.languageDescription")}
  >
    <Select
      id="language"
      options={localeOptions}
      value={localConfig.general.locale}
      onchange={handleLocaleChange}
      class="w-32"
    />
  </SettingRow>

  <SettingRow label="Theme" tooltip="Choose your preferred color theme">
    <Select
      id="theme"
      options={appearanceOptions}
      value={localConfig.general.appearance}
      onchange={handleThemeChange}
      class="w-32"
    />
  </SettingRow>
</div>
