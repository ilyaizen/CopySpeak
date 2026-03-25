<script lang="ts">
  import { Label } from "$lib/components/ui/label/index.js";

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
      ArrowUp: "Up",
      ArrowDown: "Down",
      ArrowLeft: "Left",
      ArrowRight: "Right",
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

  function handleFocus() {
    if (!disabled) {
      isCapturing = true;
    }
  }
</script>

<div class="flex items-center justify-between gap-4">
  <div class="min-w-0 flex-1">
    <Label for="hotkey-input" class="font-medium">Global Shortcut</Label>
    <p class="text-muted-foreground mt-0.5 text-xs">Press a key combination to set the shortcut</p>
  </div>

  <div class="flex items-center gap-2">
    <button
      type="button"
      id="hotkey-input"
      bind:this={inputRef}
      class="min-w-32 rounded-md border px-3 py-1.5 text-sm transition-colors
				{isCapturing
        ? 'border-primary bg-primary/10 text-primary'
        : 'border-input bg-background hover:bg-accent hover:text-accent-foreground'}
				{disabled ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'}
				focus:ring-ring focus:ring-2 focus:ring-offset-2 focus:outline-none"
      onclick={startCapture}
      onkeydown={handleKeyDown}
      onblur={handleBlur}
      onfocus={handleFocus}
      {disabled}
      aria-label="Set global hotkey"
    >
      {#if isCapturing}
        <span class="animate-pulse">Press keys...</span>
      {:else if value}
        <span class="font-mono">{value}</span>
      {:else}
        <span class="text-muted-foreground">Click to set</span>
      {/if}
    </button>

    {#if value && !disabled}
      <button
        type="button"
        class="hover:bg-muted text-muted-foreground hover:text-foreground rounded p-1"
        onclick={() => onclear()}
        title="Clear hotkey"
        aria-label="Clear hotkey"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <line x1="18" y1="6" x2="6" y2="18"></line>
          <line x1="6" y1="6" x2="18" y2="18"></line>
        </svg>
      </button>
    {/if}
  </div>
</div>
