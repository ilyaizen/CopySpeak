<script lang="ts">
  import { openExternal } from "#lib/utils/external-link";
</script>

<footer
  class="text-muted-foreground flex flex-nowrap items-center justify-between gap-2 px-2 pb-4 font-mono text-[11px] tracking-[0.1em] opacity-60"
  dir="ltr"
>
  <span class="whitespace-nowrap" aria-label="made with ♥ rust + robots"
    >made with <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      class="mx-[0.1em] inline-block size-[1em] align-[-0.12em]"
      aria-hidden="true"
      ><path
        d="M12 21 3.3 12.9C-1.4 8.5 4.9 1.3 10 5.3L12 7l2-1.7c5.1-4 11.4 3.2 6.7 7.6Z"
        fill="#e5484d"
      /></svg
    > rust + robots</span
  >
  <span class="whitespace-nowrap"
    >by <a
      href="https://x.com/ilyaizen"
      target="_blank"
      rel="noopener noreferrer"
      class="hover:underline focus-visible:outline focus-visible:outline-2"
      onclick={(event) => {
        if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey)
          return;
        event.preventDefault();
        void openExternal("https://x.com/ilyaizen");
      }}><span class="tracking-[0.2em]">@</span>ilyaizen</a
    ></span
  >
</footer>
