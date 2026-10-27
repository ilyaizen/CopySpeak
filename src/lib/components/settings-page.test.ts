import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { cleanup, fireEvent, render, within } from "@testing-library/svelte";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { tick } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import { waitForI18nReady } from "#lib/i18n";
import { testConfig } from "#lib/test-fixtures/app-config";
import { saveBar } from "#lib/stores/save-bar.svelte";
import { AUTO_SAVE_DEBOUNCE_MS } from "#lib/stores/auto-save.svelte";
import type { AppConfig } from "#lib/types";
import SettingsPage from "./settings-page.svelte";
import { tauriService } from "#lib/services/tauri";

let config: AppConfig;
let saved: AppConfig[];
let intersect: IntersectionObserverCallback;
const scrollIntoView = vi.fn();

beforeEach(async () => {
  await waitForI18nReady();
  config = testConfig();
  saved = [];
  mockIPC(
    (cmd, payload) => {
      if (cmd === "get_config") return structuredClone(config);
      if (cmd === "set_config") {
        // SAFETY: this test backend only receives Settings' typed set_config payload.
        saved.push(JSON.parse(JSON.stringify((payload as { newConfig: AppConfig }).newConfig)));
      }
      if (cmd === "reset_config") config = testConfig();
      return null;
    },
    { shouldMockEvents: true }
  );
  vi.spyOn(tauriService, "invoke").mockImplementation(invoke);
  vi.stubGlobal(
    "IntersectionObserver",
    class {
      constructor(callback: IntersectionObserverCallback) {
        intersect = callback;
      }
      observe() {}
      unobserve() {}
      disconnect() {}
      takeRecords() {
        return [];
      }
    }
  );
  Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
    configurable: true,
    value: scrollIntoView
  });
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue(
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64)"
  );
});

afterEach(() => {
  cleanup();
  clearMocks();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
  scrollIntoView.mockClear();
});

async function mountSettings() {
  const view = render(SettingsPage);
  await view.findByRole("navigation");
  return view;
}

it("keeps streaming in Advanced while legacy experimental flags are inert and hidden", async () => {
  config.history.enabled = true;
  config.post_process.enabled = true;
  config.playback.duck.enabled = true;
  config.hud.enabled = true;
  const view = await mountSettings();
  const nav = within(view.getByRole("navigation"));
  expect(nav.getAllByRole("button").map((button) => button.textContent?.trim())).toEqual([
    "General",
    "Playback",
    "Advanced",
    "About"
  ]);
  const advanced = view.container.querySelector<HTMLElement>("#advanced")!;
  expect(advanced.querySelector("#playback-streaming")).not.toBeNull();
  expect(view.container.querySelector("#playback #playback-streaming")).toBeNull();
  expect(within(advanced).getAllByRole("heading", { name: "Advanced" })).toHaveLength(1);
  expect(view.container.querySelector("#history")).toBeNull();
  expect(view.container.querySelector("#postProcess")).toBeNull();
  expect(view.container.querySelector("#playback-duck")).toBeNull();
  expect(view.container.querySelector("#hud-enabled")).toBeNull();
});

it("mounts opted-in detail sections once and returns focus to Advanced when hidden", async () => {
  const view = await mountSettings();
  const experimental = view.getByRole("switch", { name: "Experimental features" });
  await fireEvent.click(experimental);
  expect(view.container.querySelector("#history")).toBeNull();
  expect(view.container.querySelector("#postProcess")).toBeNull();
  await fireEvent.click(view.container.querySelector("#history-enabled")!);
  await fireEvent.click(view.container.querySelector("#post-process-enabled")!);
  await fireEvent.click(view.container.querySelector("#hud-enabled")!);
  expect(view.container.querySelectorAll("#history-enabled")).toHaveLength(1);
  expect(view.container.querySelectorAll("#post-process-enabled")).toHaveLength(1);
  expect(view.container.querySelector("#hud-position")).not.toBeNull();
  expect(view.container.querySelector("#history")).not.toBeNull();
  expect(view.container.querySelector("#postProcess textarea")).not.toBeNull();
  const nav = within(view.getByRole("navigation"));
  await fireEvent.click(nav.getByRole("button", { name: "History" }));
  await fireEvent.click(view.container.querySelector("#history-enabled")!);
  await tick();
  expect(nav.queryByRole("button", { name: "History" })).toBeNull();
  expect(nav.getByRole("button", { name: "Advanced" }).getAttribute("aria-current")).toBe("page");
  expect(document.activeElement?.id).toBe("advanced");
  await fireEvent.click(experimental);
  expect(view.container.querySelector("#postProcess")).toBeNull();
  await fireEvent.click(experimental);
  expect(view.container.querySelector("#postProcess")).not.toBeNull();
});

it("serializes only valid retrigger values through Save and restores changes on Cancel", async () => {
  const view = await mountSettings();
  const select = view.container.querySelector<HTMLSelectElement>("#retrigger-mode")!;
  expect(Array.from(select.options, (option) => option.value)).toEqual(["interrupt", "queue"]);
  await fireEvent.change(select, { target: { value: "queue" } });
  expect(saveBar.visible).toBe(true);
  saveBar.onCancel?.();
  await tick();
  expect(select.value).toBe("interrupt");
  expect(saveBar.visible).toBe(false);
  await fireEvent.change(select, { target: { value: "queue" } });
  await saveBar.onSave?.();
  expect(saved.at(-1)?.playback.on_retrigger).toBe("queue");
});

it("auto-saves advanced mutations without clearing legacy opt-in preferences", async () => {
  config.general.auto_save_profiles = true;
  config.history.enabled = true;
  config.post_process.enabled = true;
  config.playback.duck.enabled = true;
  const view = await mountSettings();
  vi.useFakeTimers();
  await fireEvent.click(view.container.querySelector("#playback-streaming")!);
  expect(saveBar.visible).toBe(false);
  await vi.advanceTimersByTimeAsync(AUTO_SAVE_DEBOUNCE_MS);
  expect(saved).toHaveLength(1);
  expect(saved[0].playback.streaming_enabled).toBe(false);
  expect(saved[0].general.experimental_enabled).toBe(false);
  expect(saved[0].history.enabled).toBe(true);
  expect(saved[0].post_process.enabled).toBe(true);
  expect(saved[0].playback.duck.enabled).toBe(true);
});

it("keeps Duck Windows-only without resetting its saved preference", async () => {
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Mozilla/5.0 (Macintosh)");
  config.general.experimental_enabled = true;
  config.playback.duck.enabled = true;
  const view = await mountSettings();
  expect(view.container.querySelector("#playback-duck")).toBeNull();
  await fireEvent.click(view.container.querySelector("#playback-streaming")!);
  await saveBar.onSave?.();
  expect(saved[0].playback.duck.enabled).toBe(true);
});

it("activates the short final section at the bottom of the layout scroll container", async () => {
  const view = await mountSettings();
  const scroller = view.getByRole("main");
  scroller.style.overflowY = "auto";
  Object.defineProperties(scroller, {
    scrollHeight: { configurable: true, value: 1200 },
    clientHeight: { configurable: true, value: 400 },
    scrollTop: { configurable: true, writable: true, value: 800 }
  });
  await fireEvent.scroll(scroller);
  const about = within(view.getByRole("navigation")).getByRole("button", { name: "About" });
  expect(about.getAttribute("aria-current")).toBe("page");
  // The reading band must not overwrite the bottom-of-scroll correction.
  // SAFETY: only isIntersecting and target are read by this observer callback.
  intersect(
    [
      {
        isIntersecting: true,
        target: view.container.querySelector("#advanced")!
      } as IntersectionObserverEntry
    ],
    {} as IntersectionObserver
  );
  await tick();
  expect(about.getAttribute("aria-current")).toBe("page");
});
