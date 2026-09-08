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
