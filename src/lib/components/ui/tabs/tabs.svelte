<script lang="ts" module>
  import { cn } from "$lib/utils.js";
  import type { Snippet } from "svelte";
  import type { HTMLAttributes } from "svelte/elements";

  export type TabsProps = HTMLAttributes<HTMLDivElement> & {
    class?: string;
    value?: string;
    onchange?: (value: string) => void;
    children?: Snippet;
  };

  export type TabsListProps = HTMLAttributes<HTMLDivElement> & {
    class?: string;
    children?: Snippet;
  };

  export type TabsTriggerProps = HTMLAttributes<HTMLButtonElement> & {
    class?: string;
    value: string;
    disabled?: boolean;
    children?: Snippet;
  };

  export type TabsContentProps = HTMLAttributes<HTMLDivElement> & {
    class?: string;
    value: string;
    children?: Snippet;
  };

  // Context type using getter/setter pattern with a reactive object
  export type TabsContext = {
    getValue: () => string;
    setValue: (v: string) => void;
    subscribe: (fn: (v: string) => void) => () => void;
  };
</script>

<script lang="ts">
  import { setContext } from "svelte";

  let {
    class: className,
    value = $bindable(""),
    onchange,
    children,
    ...restProps
  }: TabsProps = $props();

  // List of subscribers
  const subscribers: Set<(v: string) => void> = new Set();

  function getValue() {
    return value;
  }

  function setValue(newValue: string) {
    value = newValue;
    onchange?.(newValue);
    // Notify all subscribers
    subscribers.forEach((fn) => fn(newValue));
  }

  function subscribe(fn: (v: string) => void) {
    subscribers.add(fn);
    // Immediately call with current value
    fn(value);
    return () => subscribers.delete(fn);
  }

  // Set context
  setContext<TabsContext>("tabs-context", { getValue, setValue, subscribe });
</script>

<div data-slot="tabs" class={cn("w-full", className)} {...restProps}>
  {@render children?.()}
</div>
