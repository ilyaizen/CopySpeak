<script lang="ts">
  interface Props {
    items: any[];
    itemHeight: number;
    overscan?: number;
    children: (item: any, index: number) => any;
  }

  let { items, itemHeight, overscan = 3, children }: Props = $props();

  let containerElement: HTMLDivElement;
  let scrollTop = $state(0);
  let viewportHeight = $state(0);

  const totalHeight = $derived(items.length * itemHeight);
  const startIndex = $derived(Math.max(0, Math.floor(scrollTop / itemHeight) - overscan));
  const endIndex = $derived(
    Math.min(items.length - 1, Math.floor((scrollTop + viewportHeight) / itemHeight) + overscan)
  );
  const offsetY = $derived(startIndex * itemHeight);

  function handleScroll() {
    scrollTop = containerElement.scrollTop;
  }

  function handleResize() {
    if (containerElement) {
      viewportHeight = containerElement.clientHeight;
    }
  }

  $effect(() => {
    if (containerElement) {
      handleResize();
    }
  });
</script>

<div
  bind:this={containerElement}
  class="overflow-y-auto"
  onscroll={handleScroll}
  onresize={handleResize}
  style="height: 100%;"
>
  <div style="height: {totalHeight}px; position: relative;">
    {#if items.length > 0}
      <div
        style="transform: translateY({offsetY}px); position: absolute; top: 0; left: 0; right: 0;"
      >
        {#each items.slice(startIndex, endIndex + 1) as item, i (startIndex + i)}
          <div style="height: {itemHeight}px;">
            {@render children(item, startIndex + i)}
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>
