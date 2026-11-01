<script lang="ts">
  import { SettingRow } from "#lib/components/ui/setting-row/index.js";
  import { Select } from "#lib/components/ui/select/index.js";
  import type { AppConfig } from "#lib/types";
  import { _ } from "svelte-i18n";

  let { localConfig = $bindable() }: { localConfig: AppConfig } = $props();

  // Global daemon idle-shutdown timeout (seconds). One knob for every local
  // engine; 0 = never shut down. Falls back to the Rust default (600) for
  // configs persisted before this field existed.
  const IDLE_TIMEOUT_OPTIONS = [
    { value: "0", label: "Never" },
    { value: "60", label: "1 min" },
    { value: "300", label: "5 min" },
    { value: "600", label: "10 min" },
    { value: "1800", label: "30 min" }
  ];
  const DEFAULT_IDLE_TIMEOUT_SECS = 600;
  const idleTimeoutSecs = $derived(
    localConfig.tts.daemon_idle_timeout_secs ?? DEFAULT_IDLE_TIMEOUT_SECS
  );

  function onIdleTimeoutChange(e: Event) {
    // SAFETY: the handler is bound to the Select's onchange, so e.target is
    // the <select>; value is one of IDLE_TIMEOUT_OPTIONS' numeric strings.
    localConfig.tts.daemon_idle_timeout_secs = Number((e.target as HTMLSelectElement).value);
  }
</script>

<SettingRow label={$_("engines.daemonIdleTimeout")} tooltip={$_("engines.daemonIdleTimeoutHint")}>
  <Select
    id="daemon-idle-timeout"
    options={IDLE_TIMEOUT_OPTIONS}
    value={String(idleTimeoutSecs)}
    onchange={onIdleTimeoutChange}
    class="w-40"
  />
</SettingRow>
