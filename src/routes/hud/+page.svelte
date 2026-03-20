<script lang="ts">
  import { onMount } from "svelte";
  import HudOverlay from "$lib/components/hud-overlay.svelte";

  onMount(() => {
    console.log("[HUD PAGE] onMount firing");
    // Mark body so the Tailwind base layer skips bg-background for this window
    document.body.setAttribute("data-hud", "");

    // Belt-and-suspenders: force transparent background on html+body
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";

    console.log("[HUD PAGE] data-hud attribute set, background transparent");
  });

  // Catch any errors
  window.onerror = (msg, _src, line, _col, err) => {
    console.error("[HUD PAGE] Global error:", msg, "at line", line, err);
  };

  window.onunhandledrejection = (e) => {
    console.error("[HUD PAGE] Unhandled rejection:", e.reason);
  };
</script>

<svelte:head>
  <title></title>
  <style>
    html,
    body {
      margin: 0;
      padding: 0;
      background: transparent !important;
      background-color: transparent !important;
      overflow: hidden;
      user-select: none;
    }
  </style>
</svelte:head>

<HudOverlay />
