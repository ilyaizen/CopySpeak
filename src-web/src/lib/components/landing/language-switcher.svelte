<script lang="ts">
  import { locale, setLocale } from "$lib/i18n";
  import type { SupportedLocale } from "$lib/types";

  const languages: { code: SupportedLocale; label: string }[] = [
    { code: "en", label: "English" },
    { code: "es", label: "Español" }
  ];

  function handleLocaleChange(code: SupportedLocale) {
    setLocale(code);
  }
</script>

<div class="flex items-center gap-2">
  {#each languages as lang}
    <button
      onclick={() => handleLocaleChange(lang.code)}
      class="text-muted-foreground hover:text-foreground text-sm transition-colors {$locale ===
      lang.code
        ? 'text-foreground font-semibold'
        : ''}"
    >
      {lang.label}
    </button>
    {#if lang.code !== languages[languages.length - 1].code}
      <span class="text-muted-foreground">|</span>
    {/if}
  {/each}
</div>
