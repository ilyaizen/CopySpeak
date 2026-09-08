import { afterEach, expect, it } from "vitest";
import { cleanup, render } from "@testing-library/svelte";
import { tick } from "svelte";
import { hudStore } from "$lib/stores/hud-store.svelte";
import HudPlaybackContent from "./hud-playback-content.svelte";

afterEach(() => {
  cleanup();
  hudStore.handleStop();
});

it("highlights a word, advances the phrase, and retains text without highlighting during buffering", async () => {
  const caption = {
    text: "One two. Three four.",
    captions: {
      text: "One two. Three four.",
      words: [
        { text_start: 0, text_end: 3, start_ms: 0, end_ms: 100 },
        { text_start: 4, text_end: 7, start_ms: 200, end_ms: 300 },
        { text_start: 9, text_end: 14, start_ms: 600, end_ms: 700 },
        { text_start: 15, text_end: 19, start_ms: 900, end_ms: 1000 }
      ]
    },
    position_ms: 0,
    duration_ms: 1000,
    paused: false,
    active: true
  };
  hudStore.handleCaption(caption);
  const view = render(HudPlaybackContent, { spokenText: caption.text, barValues: [] });
  expect(view.container.querySelector(".current")?.textContent?.trim()).toBe("One");
  expect(view.container.querySelector(".caption")?.textContent?.trim()).toBe("One two.");
  hudStore.handleCaption({ ...caption, position_ms: 950, paused: true });
  await tick();
  expect(view.container.querySelector(".current")?.textContent?.trim()).toBe("four.");
  expect(view.container.querySelector(".caption")?.textContent?.trim()).toBe("Three four.");
  hudStore.handleCaption({ ...caption, position_ms: 950, active: false });
  await tick();
  expect(view.container.querySelector(".current")).toBeNull();
  expect(view.container.querySelector(".caption")?.textContent?.trim()).toBe("Three four.");
});
