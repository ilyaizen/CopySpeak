<script lang="ts">
  import { onMount } from "svelte";
  import HudOverlay from "$lib/components/hud-overlay.svelte";

  onMount(async () => {
    // Mark body so the Tailwind base layer skips bg-background for this window
    document.body.setAttribute("data-hud", "");
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";

    // Show the window now that transparent CSS is applied.
    // We only show once; from here on, HUD is hidden by moving off-screen.
    if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().show();
    }
  });
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
