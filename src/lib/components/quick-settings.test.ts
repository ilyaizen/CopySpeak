import { afterEach, expect, it } from "vite-plus/test";
import { cleanup, fireEvent, render } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import { testConfig } from "#lib/test-fixtures/app-config";
import QuickSettings from "./quick-settings.svelte";

afterEach(() => cleanup());

it("shows the quick hotkey only when Experimental and the hotkey are enabled", async () => {
  await waitForI18nReady();
  for (const experimental of [false, true]) {
    for (const enabled of [false, true]) {
      const config = testConfig();
      config.general.experimental_enabled = experimental;
      config.hotkey.enabled = enabled;
      const view = render(QuickSettings, { config });
      expect(Boolean(view.container.querySelector("#qs-hotkey"))).toBe(experimental && enabled);
      view.unmount();
    }
  }
});

it("hides effects when the global effects switch is off", async () => {
  await waitForI18nReady();
  const config = testConfig();
  config.general.experimental_enabled = true;
  config.general.effects_enabled = false;
  const view = render(QuickSettings, { config });
  expect(view.container.querySelector("#qs-effects")).toBeNull();
});

it("keeps the selected effect when the quick switch is turned off", async () => {
  await waitForI18nReady();
  const config = testConfig();
  config.general.experimental_enabled = true;
  config.general.effects_enabled = true;
  config.tts.profiles = [
    {
      id: "default",
      name: "Default",
      description: null,
      engine: "kokoro",
      voice: "af_heart",
      voice_label: null,
      speed: 1,
      pitch: 1,
      effects: { enabled: true, active_effect: "cyberpunk" },
      post_process: { enabled: false, active_template_id: null },
      engine_options: {}
    }
  ];
  const view = render(QuickSettings, { config });
  const toggle = view.container.querySelector("#qs-effects");
  if (!toggle) throw new Error("Missing effects switch");
  await fireEvent.click(toggle);
  expect(config.tts.profiles[0].effects).toEqual({ enabled: false, active_effect: "cyberpunk" });
});

it("preserves effect selection when re-enabling the quick effects switch", async () => {
  await waitForI18nReady();
  const config = testConfig();
  config.general.experimental_enabled = true;
  config.general.effects_enabled = true;
  config.tts.profiles = [
    {
      id: "default",
      name: "Default",
      description: null,
      engine: "kokoro",
      voice: "af_heart",
      voice_label: null,
      speed: 1,
      pitch: 1,
      effects: { enabled: false, active_effect: "cyberpunk" },
      post_process: { enabled: false, active_template_id: null },
      engine_options: {}
    }
  ];
  const view = render(QuickSettings, { config });
  const toggle = view.container.querySelector("#qs-effects");
  if (!toggle) throw new Error("Missing effects switch");
  await fireEvent.click(toggle);
  expect(config.tts.profiles[0].effects).toEqual({ enabled: true, active_effect: "cyberpunk" });
});

it("defaults to walkie_talkie when re-enabling an effect with none active", async () => {
  await waitForI18nReady();
  const config = testConfig();
  config.general.experimental_enabled = true;
  config.general.effects_enabled = true;
  config.tts.profiles = [
    {
      id: "default",
      name: "Default",
      description: null,
      engine: "kokoro",
      voice: "af_heart",
      voice_label: null,
      speed: 1,
      pitch: 1,
      effects: { enabled: false, active_effect: "none" },
      post_process: { enabled: false, active_template_id: null },
      engine_options: {}
    }
  ];
  const view = render(QuickSettings, { config });
  const toggle = view.container.querySelector("#qs-effects");
  if (!toggle) throw new Error("Missing effects switch");
  await fireEvent.click(toggle);
  expect(config.tts.profiles[0].effects).toEqual({ enabled: true, active_effect: "walkie_talkie" });
});

it("updates active profile post_process.enabled when toggling the post-process switch", async () => {
  await waitForI18nReady();
  const config = testConfig();
  config.general.experimental_enabled = true;
  config.post_process.enabled = true;
  config.tts.profiles = [
    {
      id: "default",
      name: "Default",
      description: null,
      engine: "kokoro",
      voice: "af_heart",
      voice_label: null,
      speed: 1,
      pitch: 1,
      effects: { enabled: false, active_effect: "none" },
      post_process: { enabled: false, active_template_id: null },
      engine_options: {}
    }
  ];
  const view = render(QuickSettings, { config });
  const toggle = view.container.querySelector("#qs-post-process");
  if (!toggle) throw new Error("Missing post-process switch");
  await fireEvent.click(toggle);
  expect(config.tts.profiles[0].post_process.enabled).toBe(true);
});
