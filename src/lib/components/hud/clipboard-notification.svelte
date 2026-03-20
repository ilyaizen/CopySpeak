<script lang="ts">
  let { durationMs }: { durationMs: number } = $props();
</script>

<div class="clipboard-notification">
  <div class="clipboard-pill" style="--dur: {durationMs}ms;">
    <div class="progress-fill"></div>
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

  .clipboard-pill {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 12px 24px;
    background: oklch(0.18 0.01 264.8/ 0.85);
    border-radius: 100px;
    overflow: hidden;
  }

  .progress-fill {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 100%;
    background: linear-gradient(
      90deg,
      oklch(62.3% 0.214 259.815) 0%,
      oklch(54.3% 0.2 259.815) 100%
    );
    transform-origin: left;
    animation: fill-progress var(--dur, 1500ms) cubic-bezier(0, 0.7, 0.1, 1) forwards;
  }

  @keyframes fill-progress {
    from {
      transform: scaleX(0);
    }
    to {
      transform: scaleX(1);
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
    position: relative;
    font-size: 15px;
    font-weight: 600;
    color: oklch(0.96 0.01 264.8);
    letter-spacing: 0.02em;
    white-space: nowrap;
    z-index: 1;
  }
</style>
