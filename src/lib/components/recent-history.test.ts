import { expect, it, vi } from "vitest";
import { render } from "@testing-library/svelte";
import { waitForI18nReady } from "$lib/i18n";
import RecentHistory from "./recent-history.svelte";

vi.mock("$lib/services/tauri", () => ({ isTauri: false }));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir: vi.fn() }));
vi.mock("$lib/stores/history-store.svelte.js", () => ({
  historyStore: {
    items: Array.from({ length: 6 }, (_, index) => ({
      id: String(index), timestamp: Date.now() - index * 60_000,
      text: `Reading ${index}`, tts_engine: "cartesia", voice: "saved",
      success: true, output_path: `/audio/${index}.wav`
    }))
  }
}));

it("scrolls the five-reading preview sideways with the wheel and ends with History navigation", async () => {
  await waitForI18nReady();
  const view = render(RecentHistory, { compact: true, limit: 5 });
  const rail = view.getByRole("list");
  Object.defineProperties(rail, {
    scrollWidth: { value: 1600 }, clientWidth: { value: 600 }
  });
  const wheel = new WheelEvent("wheel", { deltaY: 100, cancelable: true });
  rail.dispatchEvent(wheel);
  expect(rail.scrollLeft).toBe(100);
  expect(wheel.defaultPrevented).toBe(true);
  const zoom = new WheelEvent("wheel", { deltaY: 100, ctrlKey: true, cancelable: true });
  rail.dispatchEvent(zoom);
  expect(rail.scrollLeft).toBe(100);
  expect(zoom.defaultPrevented).toBe(false);
  rail.style.direction = "rtl";
  rail.scrollLeft = 0;
  rail.dispatchEvent(new WheelEvent("wheel", { deltaY: 40, cancelable: true }));
  expect(rail.scrollLeft).toBe(-40);
  expect(view.getAllByRole("listitem")).toHaveLength(6);
  expect(view.queryByRole("button", { name: "Delete" })).toBeNull();
  const link = view.getByRole("link", { name: "View more" });
  expect(link.getAttribute("href")).toBe("/history");
  expect(view.getByRole("button", { name: "Open folder" })).toBeTruthy();
  expect(rail.lastElementChild?.contains(link)).toBe(true);
  view.unmount();

  const history = render(RecentHistory, { limit: Infinity });
  const verticalWheel = new WheelEvent("wheel", { deltaY: 100, cancelable: true });
  history.getByRole("list").dispatchEvent(verticalWheel);
  expect(verticalWheel.defaultPrevented).toBe(false);
  expect(history.queryByRole("link", { name: "View more →" })).toBeNull();
  history.unmount();
});
