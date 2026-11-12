import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { cleanup, fireEvent, render, within } from "@testing-library/svelte";
import { mockIPC, clearMocks } from "@tauri-apps/api/mocks";
import { tick } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import { waitForI18nReady } from "#lib/i18n";
import { testConfig } from "#lib/test-fixtures/app-config";
import { saveBar } from "#lib/stores/save-bar.svelte";
import { AUTO_SAVE_DEBOUNCE_MS } from "#lib/stores/auto-save.svelte";
import { VERSION } from "#lib/version";
import type { AppConfig } from "#lib/types";
import SettingsPage from "./settings-page.svelte";
import { tauriService } from "#lib/services/tauri";
import * as externalLink from "#lib/utils/external-link";

let config: AppConfig;
let saved: AppConfig[];
let intersect: IntersectionObserverCallback;
const scrollIntoView = vi.fn();

beforeEach(async () => {
  await waitForI18nReady();
  const motion = new EventTarget();
  Object.defineProperty(motion, "matches", { value: false });
  vi.stubGlobal("matchMedia", () => motion);
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

it("shows System first and saves it separately from an explicit language", async () => {
  config.general.locale = "";
  vi.spyOn(navigator, "language", "get").mockReturnValue("he-IL");
  const view = await mountSettings();
  const select = view.container.querySelector("#language");
  if (!(select instanceof HTMLSelectElement)) throw new Error("Language selector missing");
  expect(select.options[0].textContent).toBe("System");
  expect(select.options[0].value).toBe("");
  expect(select.value).toBe("");

  await fireEvent.change(select, { target: { value: "en" } });
  expect(select.value).toBe("en");
  await fireEvent.change(select, { target: { value: "" } });
  expect(select.value).toBe("");
  await fireEvent.change(select, { target: { value: "en" } });
  await saveBar.onSave?.();
  expect(saved.at(-1)?.general.locale).toBe("en");
  await fireEvent.change(select, { target: { value: "" } });
  await saveBar.onSave?.();
  expect(saved.at(-1)?.general.locale).toBe("");
});

it("shows an explicitly saved language even when the system language differs", async () => {
  config.general.locale = "en";
  vi.spyOn(navigator, "language", "get").mockReturnValue("he-IL");
  const view = await mountSettings();
  const select = view.container.querySelector("#language");
  if (!(select instanceof HTMLSelectElement)) throw new Error("Language selector missing");
  expect(select.value).toBe("en");
});

it("places appearance first and import/export after debug before acknowledgments", async () => {
  const view = await mountSettings();
  const general = view.container.querySelector("#general");
  expect(general?.querySelector("h3")?.textContent?.trim()).toBe("Appearance");
  const about = view.container.querySelector("#about");
  if (!(about instanceof HTMLElement)) throw new Error("About section missing");
  const debug = about.querySelector("#debug-mode");
  if (!debug) throw new Error("Debug switch missing");
  const importHeading = within(about).getByRole("heading", { name: "Import / Export" });
  const acknowledgments = view.getByRole("heading", { name: "Acknowledgments" });
  expect(
    debug.compareDocumentPosition(importHeading) & Node.DOCUMENT_POSITION_FOLLOWING
  ).toBeTruthy();
  expect(
    importHeading.compareDocumentPosition(acknowledgments) & Node.DOCUMENT_POSITION_FOLLOWING
  ).toBeTruthy();
});

it("renders centered acknowledgment marquees with accessible provider links", async () => {
  const view = await mountSettings();
  const section = view.container.querySelector('section[aria-labelledby="acknowledgments-title"]');
  if (!(section instanceof HTMLElement)) throw new Error("Acknowledgments section missing");
  expect(
    within(section).getByText("Big thanks to the teams behind these speech engines.")
  ).toBeTruthy();
  const grid = view.getByRole("navigation").closest("aside")?.parentElement;
  const footerAbout = grid?.nextElementSibling;
  expect(footerAbout?.getAttribute("aria-labelledby")).toBe("footer-version");
  expect(footerAbout?.nextElementSibling).toBe(section);
  expect(section.nextElementSibling?.tagName).toBe("FOOTER");
  const openai = within(section).getByRole("link", { name: "OpenAI" });
  expect(openai.getAttribute("href")).toBe(
    "https://platform.openai.com/docs/guides/text-to-speech"
  );
  expect(openai.querySelector(".provider-mark")).toBeTruthy();
  expect(openai.textContent?.trim()).toBe("");
  const rows = section.querySelectorAll("ul");
  expect(rows).toHaveLength(4);
  expect(rows[1].getAttribute("aria-hidden")).toBe("true");
  expect(rows[3].getAttribute("aria-hidden")).toBe("true");
  expect(rows[1].querySelectorAll('a[tabindex="-1"]')).toHaveLength(9);
  expect(rows[3].querySelectorAll('a[tabindex="-1"]')).toHaveLength(5);
  expect(rows[0].querySelector('[aria-label="Edge-TTS"]')).not.toBeNull();
  const clone = rows[3].querySelector("a");
  if (!(clone instanceof HTMLAnchorElement)) throw new Error("Cloned provider link missing");
  const openExternal = vi.spyOn(externalLink, "openExternal").mockResolvedValue();
  await fireEvent.click(clone);
  expect(openExternal).toHaveBeenCalledWith(clone.href);
  expect(rows[0].querySelectorAll(".provider-mark")).toHaveLength(8);
  expect(rows[2].querySelectorAll(".provider-mark")).toHaveLength(4);
  expect(section.querySelectorAll(".marquee")).toHaveLength(2);
  const tagline = within(section).getByText("Big thanks to the teams behind these speech engines.");
  expect(rows[3].compareDocumentPosition(tagline) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
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

it("uses the shared app scroller for sidebar navigation", async () => {
  const scrollTo = vi.fn();
  const view = render(SettingsPage, { context: new Map([["app-lenis", () => ({ scrollTo })]]) });
  const nav = await view.findByRole("navigation");
  const about = within(nav).getByRole("button", { name: "About" });
  await fireEvent.click(about);
  expect(about.getAttribute("aria-current")).toBe("page");
  expect(scrollTo).toHaveBeenCalledWith(view.container.querySelector("#footer-version"));
  expect(scrollIntoView).not.toHaveBeenCalled();
});

it("scrolls instantly from the sidebar when reduced motion is enabled", async () => {
  const motion = new EventTarget();
  Object.defineProperty(motion, "matches", { value: true });
  vi.stubGlobal("matchMedia", () => motion);
  const view = await mountSettings();
  await fireEvent.click(
    within(view.getByRole("navigation")).getByRole("button", { name: "About" })
  );
  expect(scrollIntoView).toHaveBeenCalledWith({ behavior: "instant", block: "start" });
});

it("places About actions before acknowledgments and keeps the credit outside the sidebar grid", async () => {
  const view = await mountSettings();
  const about = view.container.querySelector("#about");
  const footerAbout = view.container.querySelector('section[aria-labelledby="footer-version"]');
  if (!(about instanceof HTMLElement) || !(footerAbout instanceof HTMLElement))
    throw new Error("About section missing");
  expect(within(footerAbout).queryByRole("heading", { name: "About" })).toBeNull();
  expect(within(footerAbout).getByText(`CopySpeak v${VERSION}`)).toBeTruthy();
  expect(within(about).queryByText(`v${VERSION}`)).toBeNull();
  const donate = within(footerAbout).getByRole("link", { name: "Buy Me a Coffee" });
  expect(donate.getAttribute("href")).toBe("https://www.buymeacoffee.com/ilyaizen");
  const source = within(footerAbout).getByRole("button", { name: "GitHub" });
  const website = within(footerAbout).getByRole("link", { name: "Website" });
  expect(website.getAttribute("href")).toBe("https://copyspeak.vercel.app/");
  expect(website.nextElementSibling).toBe(source);
  expect(source.parentElement).toBe(donate.parentElement);
  expect(source.classList.contains("cursor-pointer")).toBe(true);
  expect(donate.classList.contains("cursor-pointer")).toBe(true);
  expect(within(about).queryByRole("button", { name: "GitHub" })).toBeNull();
  expect(within(about).queryByRole("link", { name: "Buy Me a Coffee" })).toBeNull();
  const openExternal = vi.spyOn(externalLink, "openExternal").mockResolvedValue();
  await fireEvent.click(website);
  expect(openExternal).toHaveBeenCalledWith("https://copyspeak.vercel.app/");
  await fireEvent.click(website, { ctrlKey: true });
  expect(openExternal).toHaveBeenCalledTimes(1);
  openExternal.mockClear();
  await fireEvent.click(source);
  expect(openExternal).toHaveBeenCalledWith("https://github.com/ilyaizen/CopySpeak");
  await fireEvent.click(donate);
  expect(openExternal).toHaveBeenCalledWith("https://www.buymeacoffee.com/ilyaizen");
  await fireEvent.click(donate, { ctrlKey: true });
  expect(openExternal).toHaveBeenCalledTimes(2);
  const footer = view.container.querySelector("footer");
  if (!(footer instanceof HTMLElement)) throw new Error("Settings footer missing");
  expect(footer.previousElementSibling?.getAttribute("aria-labelledby")).toBe(
    "acknowledgments-title"
  );
  expect(footer.contains(donate)).toBe(false);
  expect(footer.classList.contains("flex-nowrap")).toBe(true);
  expect(footer.classList.contains("font-mono")).toBe(true);
  expect(footer.classList.contains("text-[11px]")).toBe(true);
  expect(footer.classList.contains("tracking-[0.1em]")).toBe(true);
  expect(footer.classList.contains("uppercase")).toBe(true);
  expect(footer.classList.contains("opacity-60")).toBe(false);
  const profile = within(footer).getByRole("link", { name: "@ilyaizen" });
  const attribution = profile.parentElement;
  if (!attribution) throw new Error("Settings attribution missing");
  expect(attribution.textContent?.trim()).toBe(`by @ilyaizen (${new Date().getFullYear()})`);
  expect(attribution.classList.contains("whitespace-nowrap")).toBe(true);
  expect(profile.getAttribute("href")).toBe("https://x.com/ilyaizen");
  expect(profile.textContent).toBe("@ilyaizen");
  expect(profile.querySelector("[class*='tracking-']")).toBeNull();
  openExternal.mockClear();
  await fireEvent.click(profile);
  expect(openExternal).toHaveBeenCalledWith("https://x.com/ilyaizen");
  await fireEvent.click(profile, { ctrlKey: true });
  expect(openExternal).toHaveBeenCalledTimes(1);
  const credit = within(footer).getByLabelText("made with ♥ rust + robots");
  expect(credit.classList.contains("whitespace-nowrap")).toBe(true);
  expect(footer.firstElementChild).toBe(credit);
  const heart = credit.querySelector("svg");
  expect(heart?.getAttribute("aria-hidden")).toBe("true");
  expect(heart?.querySelector("path")?.getAttribute("fill")).toBe("currentColor");
  expect(heart?.classList.contains("mx-[0.1em]")).toBe(true);
  expect(credit.textContent).not.toContain("❤️");
  expect(credit.textContent?.trim()).toBe("made with  rust + robots");
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
  const experimental = view.getByRole("switch", { name: "Experimental Features (Alpha)" });
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
