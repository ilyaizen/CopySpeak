<script lang="ts">
  import { locale, setLocale } from "#lib/i18n";
  import { getSupportedLocales } from "#lib/i18n/utils";
  import type { SupportedLocale } from "#lib/types";

  const languages = getSupportedLocales();

  function handleLocaleChange(code: SupportedLocale) {
    setLocale(code);
  }
</script>

<div class="flex items-center gap-2">
  {#each languages as lang}
    <button
      onclick={() => handleLocaleChange(lang.value)}
      class="text-muted-foreground hover:text-foreground text-sm transition-colors {$locale ===
      lang.value
        ? 'text-foreground font-semibold'
        : ''}"
    >
      {lang.label}
    </button>
    {#if lang.value !== languages[languages.length - 1].value}
      <span class="text-muted-foreground">|</span>
    {/if}
  {/each}
</div>
