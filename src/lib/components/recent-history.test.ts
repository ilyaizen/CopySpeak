import { expect, it, vi } from "vitest";
import { fireEvent, render } from "@testing-library/svelte";
import { waitForI18nReady } from "$lib/i18n";
import RecentHistory from "./recent-history.svelte";
import { historyStore } from "$lib/stores/history-store.svelte.js";

vi.mock("$app/navigation", () => ({ goto: vi.fn() }));
vi.mock("$lib/services/tauri", () => ({ isTauri: false }));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn() }));
vi.mock("$lib/stores/playback-store.svelte", () => ({ playbackStore: {} }));
vi.mock("$lib/stores/history-store.svelte.js", () => ({
  historyStore: {
    playEntry: vi.fn(),
    items: Array.from({ length: 6 }, (_, index) => ({
      id: String(index),
      timestamp: Date.now() - index * 60_000,
      text: `Reading ${index}`,
      tts_engine: "cartesia",
      voice: "saved",
      batch_id: index >= 4 ? "batch" : undefined,
      metadata: { fragment_index: index },
      speed: 1,
      success: true,
      output_path: `/audio/${index}.wav`
    }))
  }
}));

it("shows three text-first readings without intercepting scrolling, and separates restore from play", async () => {
  await waitForI18nReady();
  const onRestore = vi.fn().mockResolvedValue(undefined);
  const view = render(RecentHistory, { compact: true, onRestore });
  expect(view.getAllByRole("listitem")).toHaveLength(3);
  expect(view.queryByText("0.wav")).toBeNull();
  expect(view.getByRole("link", { name: "View all history" }).getAttribute("href")).toBe(
    "/history"
  );
  const wheel = new WheelEvent("wheel", { deltaY: 100, cancelable: true });
  view.getByRole("list").dispatchEvent(wheel);
  expect(wheel.defaultPrevented).toBe(false);
  await fireEvent.click(view.getByRole("button", { name: "Restore reading: Reading 0" }));
  expect(onRestore).toHaveBeenCalledWith(expect.objectContaining({ text: "Reading 0" }), false);
  expect(historyStore.playEntry).not.toHaveBeenCalled();
  await fireEvent.click(view.getAllByRole("button", { name: "Play" })[0]);
  expect(historyStore.playEntry).toHaveBeenCalledWith("0");
  expect(onRestore).toHaveBeenCalledTimes(1);
  view.unmount();
});

it("searches full history and bulk-selects only visible readings", async () => {
  await waitForI18nReady();
  const view = render(RecentHistory, { limit: Infinity });
  expect(view.getAllByRole("listitem")).toHaveLength(5);
  await fireEvent.input(view.getByRole("searchbox"), { target: { value: "Reading 4" } });
  expect(view.getAllByRole("listitem")).toHaveLength(1);
  expect(view.getByRole("button", { name: /Restore reading: Reading 4.*Reading 5/s })).toBeTruthy();
  await fireEvent.click(view.getByRole("checkbox", { name: "Select all visible readings" }));
  expect(view.getByRole("button", { name: "Delete selected (1)" }).hasAttribute("disabled")).toBe(
    false
  );
  await fireEvent.input(view.getByRole("searchbox"), { target: { value: "Reading 2" } });
  expect(view.getByRole("button", { name: "Delete selected (0)" }).hasAttribute("disabled")).toBe(
    true
  );
  view.unmount();
});
