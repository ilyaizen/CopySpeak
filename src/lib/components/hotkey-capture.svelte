<script lang="ts">
  import Kbd from "$lib/components/ui/kbd/kbd.svelte";
  import KbdGroup from "$lib/components/ui/kbd/kbd-group.svelte";
  import { X } from "@lucide/svelte";

  interface Props {
    value: string;
    disabled?: boolean;
    onchange: (value: string) => void;
    onclear: () => void;
  }

  let { value, disabled = false, onchange, onclear }: Props = $props();

  let isCapturing = $state(false);
  let inputRef: HTMLButtonElement | undefined = $state();

  function formatKey(key: string): string {
    const keyMap: Record<string, string> = {
      Control: "Ctrl",
      Alt: "Alt",
      Shift: "Shift",
      Meta: "Win",
      ArrowUp: "↑",
      ArrowDown: "↓",
      ArrowLeft: "←",
      ArrowRight: "→",
      " ": "Space",
      Escape: "Esc",
      Delete: "Del"
    };
    return keyMap[key] || key;
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (!isCapturing) return;

    e.preventDefault();
    e.stopPropagation();

    if (e.key === "Escape") {
      isCapturing = false;
      return;
    }

    if (e.key === "Backspace" || e.key === "Delete") {
      onclear();
      isCapturing = false;
      return;
    }

    const modifiers: string[] = [];
    if (e.ctrlKey) modifiers.push("Ctrl");
    if (e.altKey) modifiers.push("Alt");
    if (e.shiftKey) modifiers.push("Shift");
    if (e.metaKey) modifiers.push("Win");

    let key = formatKey(e.key);

    if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) {
      return;
    }

    if (modifiers.length === 0) {
      return;
    }

    if (key.length === 1) {
      key = key.toUpperCase();
    }

    const hotkey = [...modifiers, key].join("+");
    onchange(hotkey);
    isCapturing = false;
  }

  function startCapture() {
    if (disabled) return;
    isCapturing = true;
    inputRef?.focus();
  }

  function handleBlur() {
    isCapturing = false;
  }
</script>

<div class="flex items-center gap-1">
  <button
    type="button"
    id="hotkey-input"
    bind:this={inputRef}
    class="inline-flex min-w-20 items-center gap-1 rounded border px-2 py-1 text-sm transition-colors
      {isCapturing
      ? 'border-primary bg-primary/10'
      : 'border-input bg-background hover:bg-accent/50'}
      {disabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'}
      focus:ring-ring focus:ring-2 focus:ring-offset-1 focus:outline-none"
    onclick={startCapture}
    onkeydown={handleKeyDown}
    onblur={handleBlur}
    {disabled}
    aria-label="Set global hotkey"
  >
    {#if isCapturing}
      <span class="text-primary animate-pulse">...</span>
    {:else if value}
      <KbdGroup>
        {#each value.split("+") as key}
          <Kbd>{key}</Kbd>
        {/each}
      </KbdGroup>
    {:else}
      <span class="text-muted-foreground">Set</span>
    {/if}
  </button>

  {#if value && !disabled}
    <button
      type="button"
      class="hover:bg-muted/50 rounded p-1 transition-colors"
      onclick={onclear}
      aria-label="Clear hotkey"
    >
      <X class="text-muted-foreground size-3.5" />
    </button>
  {/if}
</div>
