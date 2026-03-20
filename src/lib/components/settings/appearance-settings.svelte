<script lang="ts">
  import { Sun, Moon, Monitor } from "@lucide/svelte";
  import { setMode } from "mode-watcher";

  let {
    appearance = "system",
    onchange
  }: {
    appearance: "system" | "light" | "dark";
    onchange: (value: "system" | "light" | "dark") => void;
  } = $props();

  const appearanceOptions = [
    { value: "system", label: "System", icon: Monitor },
    { value: "light", label: "Light", icon: Sun },
    { value: "dark", label: "Dark", icon: Moon }
  ];

  // Watch for appearance changes and sync with mode-watcher
  // Guard: only call setMode when value actually changes to prevent WebKit reactivity loops
  let prevAppearance: string | undefined;
  $effect(() => {
    if (appearance === prevAppearance) return;
    prevAppearance = appearance;
    if (appearance === "system") {
      setMode("system");
    } else if (appearance === "light") {
      setMode("light");
    } else if (appearance === "dark") {
      setMode("dark");
    }
  });
</script>

<div class="border-border my-4 space-y-3 border-t pt-4">
  <p class="text-sm font-medium">Theme</p>
  <div class="grid grid-cols-3 gap-2">
    {#each appearanceOptions as option}
      <button
        type="button"
        class="flex flex-col items-center gap-2 rounded-lg border-2 p-3 transition-all {appearance ===
        option.value
          ? 'border-primary bg-primary/10'
          : 'border-border hover:border-primary/50 hover:bg-accent'}"
        onclick={() => onchange(option.value as "system" | "light" | "dark")}
      >
        {#if option.value === "system"}
          <Monitor class="h-6 w-6" />
        {:else if option.value === "light"}
          <Sun class="h-6 w-6" />
        {:else}
          <Moon class="h-6 w-6" />
        {/if}
        <span class="text-sm font-medium">{option.label}</span>
      </button>
    {/each}
  </div>
</div>
