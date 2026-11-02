import { describe, expect, it, vi, beforeEach } from "vite-plus/test";
import { render, fireEvent } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import ProfileManager from "./profile-manager.svelte";
import { testConfig } from "#lib/test-fixtures/app-config";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "list_tts_engines") return [];
    if (cmd === "has_engine_credentials") return true;
    return undefined;
  })
}));

vi.mock("svelte-sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() }
}));

describe("profile-manager new profile effects defaults", () => {
  beforeEach(async () => {
    await waitForI18nReady();
  });

  it("creates new profiles with effects off even when experimental is enabled", async () => {
    const config = testConfig();
    config.general.experimental_enabled = true;
    config.tts.active_profile_id = "p1";
    config.tts.profiles = [
      {
        id: "p1",
        name: "Existing Profile",
        description: null,
        engine: "kokoro",
        voice: "af_heart",
        voice_label: "Heart",
        speed: 1.0,
        pitch: 1.0,
        effects: { enabled: true, active_effect: "cyberpunk" },
        post_process: { enabled: false, active_template_id: null },
        engine_options: {}
      }
    ];

    const view = render(ProfileManager, { localConfig: config });

    // Open the "New Profile" dropdown
    const newBtn = view.getByRole("button", { name: /New Profile|New/i });
    await fireEvent.click(newBtn);

    // Find and click the first preset menuitem
    const presetItems = view.getAllByRole("menuitem");
    expect(presetItems.length).toBeGreaterThan(0);
    await fireEvent.click(presetItems[0]);

    // A new profile was appended
    expect(config.tts.profiles.length).toBe(2);
    const newProfile = config.tts.profiles[1];

    // Effects must be disabled with active_effect: "none" even with experimental_enabled === true
    expect(newProfile.effects).toEqual({
      enabled: false,
      active_effect: "none"
    });

    // Existing profile effects must be preserved untouched
    expect(config.tts.profiles[0].effects).toEqual({
      enabled: true,
      active_effect: "cyberpunk"
    });

    view.unmount();
  });

  it("shows none in effect select when profile effects is disabled with preserved effect", async () => {
    const config = testConfig();
    config.general.experimental_enabled = true;
    config.general.effects_enabled = true;
    config.tts.active_profile_id = "p1";
    config.tts.profiles = [
      {
        id: "p1",
        name: "Test Profile",
        description: null,
        engine: "kokoro",
        voice: "af_heart",
        voice_label: "Heart",
        speed: 1.0,
        pitch: 1.0,
        effects: { enabled: false, active_effect: "cyberpunk" },
        post_process: { enabled: false, active_template_id: null },
        engine_options: {}
      }
    ];

    const view = render(ProfileManager, { localConfig: config });
    const select = view.getByRole("combobox", { name: "Effect" });
    if (!(select instanceof HTMLSelectElement))
      throw new Error("Effect select is not a native select");
    expect(select.value).toBe("none");

    // Selecting a new effect enables it
    await fireEvent.change(select, { target: { value: "game_boy" } });
    expect(config.tts.profiles[0].effects).toEqual({
      enabled: true,
      active_effect: "game_boy"
    });

    view.unmount();
  });
});
