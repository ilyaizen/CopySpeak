<script lang="ts">
  let { durationMs }: { durationMs: number } = $props();
</script>

<div class="clipboard-notification">
  <div class="clipboard-pill" style:--dur="{durationMs}ms">
    <span class="clipboard-title">Clipboard Copied</span>
  </div>
</div>

<style>
  .clipboard-notification {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: notification-in 0.4s cubic-bezier(0, 0.7, 0.1, 1) forwards;
  }

  @property --progress {
    syntax: "<percentage>";
    initial-value: 0%;
    inherits: true;
  }

  .clipboard-pill {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 12px 24px;
    background: oklch(0.18 0.01 264.8/ 0.85);
    border-radius: 100px;
    --progress: 0%;
    animation: progress-fill var(--dur, 1500ms) cubic-bezier(0, 0.7, 0.1, 1) forwards;
  }

  .clipboard-pill::before {
    content: "";
    position: absolute;
    inset: -4px;
    border-radius: 100px;
    padding: 4px;
    background: conic-gradient(
      from 180deg,
      oklch(62.3% 0.214 259.815) 0%,
      oklch(62.3% 0.214 259.815) var(--progress),
      oklch(62.3% 0.214 259.815/ 0.3) var(--progress),
      oklch(62.3% 0.214 259.815/ 0.3) 100%
    );
    mask:
      linear-gradient(#fff 0 0) content-box,
      linear-gradient(#fff 0 0);
    mask-composite: exclude;
  }

  @keyframes progress-fill {
    to {
      --progress: 100%;
    }
  }

  @keyframes notification-in {
    from {
      opacity: 0;
      transform: scale(0.4);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .clipboard-title {
    font-size: 15px;
    font-weight: 600;
    color: oklch(0.96 0.01 264.8);
    letter-spacing: 0.02em;
    white-space: nowrap;
    z-index: 1;
  }
</style>
