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

it("places appearance first and import/export after debug before acknowledgments", async () => {
  const view = await mountSettings();
  const general = view.container.querySelector("#general");
  expect(general?.querySelector("h3")?.textContent?.trim()).toBe("Appearance");
  const about = view.container.querySelector("#about");
  if (!(about instanceof HTMLElement)) throw new Error("About section missing");
  const debug = about.querySelector("#debug-mode");
  if (!debug) throw new Error("Debug switch missing");
  const importHeading = within(about).getByRole("heading", { name: "Import / Export" });
  const acknowledgments = within(about).getByRole("heading", { name: "Acknowledgments" });
  expect(
    debug.compareDocumentPosition(importHeading) & Node.DOCUMENT_POSITION_FOLLOWING
  ).toBeTruthy();
  expect(
    importHeading.compareDocumentPosition(acknowledgments) & Node.DOCUMENT_POSITION_FOLLOWING
  ).toBeTruthy();
});

it("renders linked provider names without acknowledgment tooltips", async () => {
  const view = await mountSettings();
  const section = view.container.querySelector('[aria-labelledby="acknowledgments-title"]');
  if (!(section instanceof HTMLElement)) throw new Error("Acknowledgments section missing");
  // The section centers like the About footer below it.
  const heading = section.querySelector("h3");
  if (!heading) throw new Error("Acknowledgments heading missing");
  expect(heading.className).toContain("text-center");
  const grid = section.querySelector("ul");
  if (!grid) throw new Error("Acknowledgments grid missing");
  expect(grid.className).toContain("justify-center");
  // Mark and name are ONE link per provider, so they highlight together.
  const openaiLinks = within(section).getAllByRole("link", { name: "OpenAI" });
  expect(openaiLinks).toHaveLength(1);
  const openai = openaiLinks[0];
  expect(openai.getAttribute("href")).toBe(
    "https://platform.openai.com/docs/guides/text-to-speech"
  );
  expect(openai.className).toContain("group");
  expect(openai.querySelector(".provider-mark")).toBeTruthy();
  const nameSpan = [...openai.querySelectorAll("span")].find(
    (s) => s.textContent?.trim() === "OpenAI"
  );
  if (!nameSpan) throw new Error("Provider name span missing");
  expect(nameSpan.className).toContain("group-hover:underline");
  expect(section.querySelectorAll(".provider-mark")).toHaveLength(12);
  expect(section.querySelectorAll("button")).toHaveLength(0);
  for (const name of ["Kokoro", "Edge-TTS"]) {
    const link = within(section).getByRole("link", { name });
    const item = link.closest("li");
    expect(item?.querySelector(".provider-text-mark")?.textContent?.trim()).toBe(name);
  }
  expect(within(section).getByRole("link", { name: "Microsoft" }).getAttribute("href")).toBe(
    "https://microsoft.ai/models/mai-voice-2-1/"
  );
});

it("shows the creator credit and support link in About", async () => {
  const view = await mountSettings();
  const about = view.container.querySelector("#about");
  if (!(about instanceof HTMLElement)) throw new Error("About section missing");
  expect(within(about).getByRole("link", { name: "Buy Me a Coffee" }).getAttribute("href")).toBe(
    "https://www.buymeacoffee.com/ilyaizen"
  );
  expect(within(about).getByText(`🄯 ${new Date().getFullYear()} Ilya Aizenberg`)).toBeTruthy();
  const credit = within(about).getByLabelText("Made with ♥ Rust & Bots.");
  const heart = credit.querySelector("svg");
  expect(heart?.getAttribute("aria-hidden")).toBe("true");
  expect(credit.textContent).not.toContain("❤️");
  expect(credit.textContent?.trim()).toBe("Made with  Rust & Bots.");
});

it("keeps hidden-start and close-to-tray preferences but disables them without a tray", async () => {
  config.general.show_tray_icon = true;
  config.general.start_minimized = true;
  config.general.close_behavior = "minimize-to-tray";
  const view = await mountSettings();
  const showTray = view.getByRole("switch", { name: "Show Tray Icon" });
  const startInTray = view.getByRole("switch", { name: "Start in Tray" });
  const closeToTray = view.getByRole("switch", { name: "Close to Tray" });
  await fireEvent.click(showTray);
  expect(startInTray.hasAttribute("disabled")).toBe(true);
  expect(closeToTray.hasAttribute("disabled")).toBe(true);
  expect(startInTray.getAttribute("aria-checked")).toBe("true");
  expect(closeToTray.getAttribute("aria-checked")).toBe("true");
  await fireEvent.click(showTray);
  expect(startInTray.hasAttribute("disabled")).toBe(false);
  expect(closeToTray.hasAttribute("disabled")).toBe(false);
});

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
  expect(view.container.querySelector("#hud-enabled")).toBeNull();
});

it("shows Duck in Playback on Windows without Experimental", async () => {
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Mozilla/5.0 (Windows NT 10.0)");
  config.general.experimental_enabled = false;
  config.playback.duck.enabled = true;
  const view = await mountSettings();
  expect(view.container.querySelector("#playback #playback-duck")).not.toBeNull();
  expect(view.container.querySelector("#playback #duck-level")).not.toBeNull();
  expect(view.container.querySelector("#advanced #playback-duck")).toBeNull();
});

it("mounts opted-in detail sections once and returns focus to Advanced when hidden", async () => {
  const view = await mountSettings();
  const experimental = view.getByRole("switch", { name: "Experimental Features" });
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

it("shows auto-save only under Experimental and keeps it off by default", async () => {
  const view = await mountSettings();
  expect(view.container.querySelector("#auto-save-profiles")).toBeNull();
  await fireEvent.click(view.container.querySelector("#experimental-enabled")!);
  const autoSave = view.getByRole("switch", { name: "Auto-Save Changes" });
  expect(view.container.querySelector("#advanced")?.contains(autoSave)).toBe(true);
  expect(autoSave.getAttribute("aria-checked")).toBe("false");
});

it("places the hotkey first under Experimental and preserves its saved preference", async () => {
  config.hotkey.enabled = true;
  const view = await mountSettings();
  expect(view.container.querySelector("#hotkey-enabled")).toBeNull();
  await fireEvent.click(view.container.querySelector("#experimental-enabled")!);
  const hotkey = view.container.querySelector("#hotkey-enabled")!;
  const autoSave = view.container.querySelector("#auto-save-profiles")!;
  expect(view.container.querySelector("#advanced")?.contains(hotkey)).toBe(true);
  expect(hotkey.compareDocumentPosition(autoSave) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(hotkey.getAttribute("aria-checked")).toBe("true");
  expect(view.container.querySelector("#hotkey-input")).not.toBeNull();
  await fireEvent.click(hotkey);
  expect(view.container.querySelector("#hotkey-input")).toBeNull();
  await fireEvent.click(view.container.querySelector("#experimental-enabled")!);
  expect(view.container.querySelector("#hotkey-enabled")).toBeNull();
});

it("does not auto-save when Experimental is off even with a saved opt-in", async () => {
  config.general.auto_save_profiles = true;
  const view = await mountSettings();
  vi.useFakeTimers();
  await fireEvent.click(view.container.querySelector("#playback-streaming")!);
  await vi.advanceTimersByTimeAsync(AUTO_SAVE_DEBOUNCE_MS);
  expect(saved).toHaveLength(0);
  expect(saveBar.visible).toBe(true);
});

it("auto-saves advanced mutations without clearing legacy opt-in preferences", async () => {
  config.general.auto_save_profiles = true;
  config.general.experimental_enabled = true;
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
  expect(saved[0].general.experimental_enabled).toBe(true);
  expect(saved[0].history.enabled).toBe(true);
  expect(saved[0].post_process.enabled).toBe(true);
  expect(saved[0].playback.duck.enabled).toBe(true);
});

it("keeps Duck Windows-only without resetting its saved preference", async () => {
  vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Mozilla/5.0 (Macintosh)");
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
