<script>
  import { Toaster as Sonner } from "svelte-sonner";
  import { mode } from "mode-watcher";

  /** @type {{ [key: string]: any }} */
  let { ...rest } = $props();

  // mode is a Svelte 5 rune with a .current property
  const currentMode = $derived(mode.current);
</script>

<Sonner
  theme={currentMode}
  class="toaster group"
  toastOptions={{
    classes: {
      toast:
        "group toast group-[.toaster]:bg-background group-[.toaster]:text-foreground group-[.toaster]:border-border group-[.toaster]:shadow-lg",
      description: "group-[.toast]:text-muted-foreground",
      actionButton: "group-[.toast]:bg-primary group-[.toast]:text-primary-foreground",
      cancelButton: "group-[.toast]:bg-muted group-[.toast]:text-muted-foreground"
    }
  }}
  {...rest}
/>
