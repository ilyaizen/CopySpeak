import { beforeEach, describe, expect, it, vi } from "vite-plus/test";
import { fireEvent, render, waitFor } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import ProfileManager from "./profile-manager.svelte";
import { testConfig } from "#lib/test-fixtures/app-config";
import type { AppConfig, EngineCatalogEntry, VoiceCatalogEntry } from "#lib/types";

function voice(id: string, label: string): VoiceCatalogEntry {
  return {
    id,
    label,
    language: null,
    description: null,
    gender: null,
    preview_url: null,
    tags: []
  };
}

function entry(engine: string, voices: VoiceCatalogEntry[], refresh = false): EngineCatalogEntry {
  return {
    engine,
    label: engine,
    description: "",
    docs_url: "",
    supports_voice_refresh: refresh,
    supports_pitch: false,
    supports_captions: true,
    supports_bracket_emotes: false,
    options: [],
    voices
  };
}

const CATALOG = [
  entry("kokoro", [voice("af_heart", "Heart"), voice("am_adam", "Adam")]),
  entry("elevenlabs", [voice("premade-1", "Rachel"), voice("premade-2", "Adam")], true)
];

// Per-test knobs read at call time by the hoisted mock.
/** get_cached_voices' wire shape: engine id -> last provider list. */
interface CachedVoiceStore {
  [engine: string]: { fetched_at_ms: number; voices: VoiceCatalogEntry[] };
}
let cachedVoices: CachedVoiceStore = {};
let refreshError: string | null = null;
const calls: string[] = [];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, args?: { engine?: string }) => {
    calls.push(cmd);
    if (cmd === "list_tts_engines") return CATALOG;
    if (cmd === "get_cached_voices") return cachedVoices;
    if (cmd === "list_tts_voices") {
      if (refreshError) throw refreshError;
      return CATALOG.find((e) => e.engine === args?.engine)?.voices ?? [];
    }
    if (cmd === "has_engine_credentials") return true;
    return undefined;
  })
}));

vi.mock("svelte-sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() }
}));

function configWith(engine: "kokoro" | "elevenlabs", voiceId: string, label: string): AppConfig {
  const config = testConfig();
  config.tts.active_profile_id = "p1";
  config.tts.profiles = [
    {
      id: "p1",
      name: "Reader",
      description: null,
      engine,
      voice: voiceId,
      voice_label: label,
      speed: 1.0,
      pitch: 1.0,
      effects: { enabled: false, active_effect: "none" },
      engine_options: {}
    }
  ];
  return config;
}

describe("profile voice selection", () => {
  beforeEach(async () => {
    await waitForI18nReady();
    // jsdom has no scrolling; the open picker scrolls its active row into view.
    Element.prototype.scrollIntoView = () => {};
    cachedVoices = {};
    refreshError = null;
    calls.length = 0;
  });

  it("shows a saved voice that the current list does not contain", async () => {
    const config = configWith("elevenlabs", "my-clone-id", "My Clone");
    const view = render(ProfileManager, { localConfig: config });
    await waitFor(() => expect(view.getAllByRole("combobox").length).toBeGreaterThan(1));
    expect(view.getByRole("button", { name: /My Clone/ })).toBeTruthy();
    expect(config.tts.profiles[0].voice).toBe("my-clone-id");
    view.unmount();
  });

  it("does not carry one engine's voice id into another engine", async () => {
    const config = configWith("kokoro", "af_heart", "Heart");
    const view = render(ProfileManager, { localConfig: config });
    await waitFor(() => expect(view.getAllByRole("combobox").length).toBeGreaterThan(1));
    const engineSelect = view.getAllByRole("combobox")[1];
    await fireEvent.change(engineSelect, { target: { value: "elevenlabs" } });
    expect(config.tts.profiles[0].engine).toBe("elevenlabs");
    expect(config.tts.profiles[0].voice).toBe("premade-1");
    expect(config.tts.profiles[0].voice_label).toBe("Rachel");
    view.unmount();
  });

  it("never auto-refreshes a cached provider list, however old", async () => {
    const yearAgo = Date.now() - 365 * 24 * 60 * 60 * 1000;
    cachedVoices = {
      elevenlabs: { fetched_at_ms: yearAgo, voices: [voice("my-clone-id", "My Clone (cached)")] }
    };
    const config = configWith("elevenlabs", "my-clone-id", "My Clone");
    const view = render(ProfileManager, { localConfig: config });
    await waitFor(() => view.getByRole("button", { name: /My Clone \(cached\)/ }));
    expect(calls).not.toContain("list_tts_voices");
    view.unmount();
  });

  it("fetches the provider list once when nothing is cached", async () => {
    const config = configWith("elevenlabs", "premade-1", "Rachel");
    const view = render(ProfileManager, { localConfig: config });
    await waitFor(() => expect(calls).toContain("list_tts_voices"));
    expect(calls.filter((c) => c === "list_tts_voices")).toHaveLength(1);
    view.unmount();
  });

  it("keeps the previous list and shows the reason when a refresh fails", async () => {
    cachedVoices = {
      elevenlabs: { fetched_at_ms: Date.now(), voices: [voice("my-clone-id", "My Clone (cached)")] }
    };
    refreshError = "Failed to fetch voices: HTTP error: 401";
    const config = configWith("elevenlabs", "my-clone-id", "My Clone");
    const view = render(ProfileManager, { localConfig: config });
    const trigger = await waitFor(() => view.getByRole("button", { name: /My Clone \(cached\)/ }));
    await fireEvent.click(trigger);
    await fireEvent.click(await waitFor(() => view.getByRole("button", { name: /^Refresh$/ })));
    await waitFor(() => expect(document.body.textContent).toContain("HTTP error: 401"));
    // The failed refresh did not wipe the list.
    expect(view.getByRole("option", { name: /My Clone \(cached\)/ })).toBeTruthy();
    expect(config.tts.profiles[0].voice).toBe("my-clone-id");
    view.unmount();
  });

  it("shows a first-fetch failure in the sheet when it opens", async () => {
    refreshError = "Failed to fetch voices: HTTP error: 401";
    const config = configWith("elevenlabs", "premade-1", "Rachel");
    const view = render(ProfileManager, { localConfig: config });
    await waitFor(() => expect(calls).toContain("list_tts_voices"));
    await fireEvent.click(await waitFor(() => view.getByRole("button", { name: /Rachel/ })));
    await waitFor(() => expect(document.body.textContent).toContain("HTTP error: 401"));
    view.unmount();
  });

  it("moves focus to the manual id input once the sheet hands over", async () => {
    const config = configWith("elevenlabs", "premade-1", "Rachel");
    const view = render(ProfileManager, { localConfig: config });
    await fireEvent.click(await waitFor(() => view.getByRole("button", { name: /Rachel/ })));
    await fireEvent.click(view.getByRole("button", { name: /Custom \/ manual id/ }));
    // The input is disabled until the override renders; focus must wait for that.
    const manual = await waitFor(() => view.getByDisplayValue("premade-1"));
    await waitFor(() => expect(document.activeElement).toBe(manual));
    view.unmount();
  });
});
