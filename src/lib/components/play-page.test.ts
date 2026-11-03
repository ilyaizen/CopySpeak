import { describe, expect, it, vi, beforeEach, afterEach } from "vite-plus/test";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/svelte";
import { tick } from "svelte";
import { waitForI18nReady } from "#lib/i18n";
import PlayPage from "./play-page.svelte";
import { invoke } from "@tauri-apps/api/core";
import { listen, type Event } from "@tauri-apps/api/event";
import { historyStore } from "#lib/stores/history-store.svelte";
import { testConfig } from "#lib/test-fixtures/app-config";
import type { AppConfig } from "#lib/types";

// QuickSettings' emote-tag probe invokes through this service; it only feeds
// the post-process row's disabled state, which these tests never open. A plain
// function survives clearAllMocks/restoreAllMocks between tests.
vi.mock("#lib/services/tauri.js", () => ({
  isTauri: true,
  invoke: async () => false
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {})
}));

vi.mock("#lib/stores/history-store.svelte", () => ({
  historyStore: {
    items: [],
    loadHistory: vi.fn(async () => {})
  }
}));

vi.mock("#lib/stores/playback-store.svelte", () => ({
  playbackStore: {
    isPlaying: false,
    isPaused: false,
    isSynthesizing: false,
    caption: null,
    hasCachedAudio: false,
    historyReadingId: null,
    startReading: vi.fn(),
    handleStop: vi.fn(),
    handleReplay: vi.fn(),
    handleTogglePause: vi.fn(),
    skipBackward: vi.fn(),
    skipForward: vi.fn(),
    syncPlaybackConfig: vi.fn()
  }
}));

describe("play-page history gating", () => {
  beforeEach(async () => {
    await waitForI18nReady();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renders RecentHistory only when experimental and history are both enabled", async () => {
    const config = testConfig();
    config.general.experimental_enabled = true;
    config.history.enabled = true;
    vi.mocked(invoke).mockImplementation(async (cmd) => {
      if (cmd === "get_config") return config;
      return undefined;
    });

    const view = render(PlayPage);

    await waitFor(() => {
      expect(view.queryByRole("region", { name: /Recent History/i })).not.toBeNull();
    });
    expect(historyStore.loadHistory).toHaveBeenCalled();
    view.unmount();
  });

  it("hides RecentHistory and skips loadHistory when experimental is off", async () => {
    const config = testConfig();
    config.general.experimental_enabled = false;
    config.history.enabled = true;
    vi.mocked(invoke).mockImplementation(async (cmd) => {
      if (cmd === "get_config") return config;
      return undefined;
    });

    const view = render(PlayPage);

    await waitFor(() => {
      expect(view.queryByRole("textbox")).not.toBeNull();
    });
    expect(view.queryByRole("region", { name: /Recent History/i })).toBeNull();
    expect(historyStore.loadHistory).not.toHaveBeenCalled();
    view.unmount();
  });

  it("hides RecentHistory when history is disabled", async () => {
    const config = testConfig();
    config.general.experimental_enabled = true;
    config.history.enabled = false;
    vi.mocked(invoke).mockImplementation(async (cmd) => {
      if (cmd === "get_config") return config;
      return undefined;
    });

    const view = render(PlayPage);

    await waitFor(() => {
      expect(view.queryByRole("textbox")).not.toBeNull();
    });
    expect(view.queryByRole("region", { name: /Recent History/i })).toBeNull();
    expect(historyStore.loadHistory).not.toHaveBeenCalled();
    view.unmount();
  });
});

describe("play-page config save pipeline", () => {
  // Keep in sync with SAVE_DEBOUNCE_MS in play-page.svelte.
  const SAVE_DEBOUNCE_MS = 500;
  const scrollIntoView = vi.fn();
  let configChanged: ((event: Event<unknown>) => void) | null = null;

  beforeEach(async () => {
    await waitForI18nReady();
    vi.clearAllMocks();
    configChanged = null;
    vi.mocked(listen).mockImplementation(async (event, handler) => {
      if (event === "config-changed") configChanged = handler;
      return () => {};
    });
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: scrollIntoView
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    cleanup();
    vi.restoreAllMocks();
  });

  function deferredGate() {
    let resolve!: () => void;
    const promise = new Promise<void>((r) => (resolve = r));
    return { promise, resolve };
  }

  function experimentalPlayConfig(): AppConfig {
    const config = testConfig();
    config.general.experimental_enabled = true;
    config.general.effects_enabled = true;
    // The quick-settings edits below go through the effects switch: unlike the
    // hotkey switch it stays mounted across toggles, and it needs an active
    // profile to be clickable.
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
    return config;
  }

  interface SaveBackendOptions {
    getConfig: () => AppConfig;
    failSaves?: () => boolean;
    holdFirstSave?: boolean;
  }

  interface SaveBackend {
    saved: AppConfig[];
    cmdLog: string[];
    inFlight: () => number;
    maxConcurrent: () => number;
    release: () => void;
  }

  // Records every invoke in order and can park the first set_config in flight
  // on a gate only the test opens, so ordering is proven, not polled.
  function mockSaveBackend(options: SaveBackendOptions): SaveBackend {
    const saved: AppConfig[] = [];
    const cmdLog: string[] = [];
    let active = 0;
    let maxConcurrent = 0;
    let hold: { promise: Promise<void>; resolve: () => void } | null = null;
    vi.mocked(invoke).mockImplementation(async (cmd, payload) => {
      cmdLog.push(String(cmd));
      if (cmd === "get_config") return options.getConfig();
      if (cmd === "set_config") {
        active++;
        maxConcurrent = Math.max(maxConcurrent, active);
        try {
          if (options.holdFirstSave && saved.length === 0) {
            hold = deferredGate();
            await hold.promise;
          }
          if (options.failSaves?.()) throw new Error("disk full");
        } finally {
          active--;
        }
        // SAFETY: this test backend only receives Play's typed set_config payload.
        saved.push(JSON.parse(JSON.stringify((payload as { newConfig: AppConfig }).newConfig)));
      }
      return undefined;
    });
    return {
      saved,
      cmdLog,
      inFlight: () => active,
      maxConcurrent: () => maxConcurrent,
      release: () => hold?.resolve()
    };
  }

  // Fake timers are enabled before mount so every debounce the page arms is
  // fake and no real timer can fire mid-test; the async mount chain is drained
  // by advancing the fake clock.
  async function mountPlay() {
    vi.useFakeTimers();
    const view = render(PlayPage);
    for (let i = 0; i < 20 && view.container.querySelector("#qs-effects") === null; i++) {
      await vi.advanceTimersByTimeAsync(10);
    }
    return view;
  }

  function effectsSwitch(view: { container: HTMLElement }): Element {
    const toggle = view.container.querySelector("#qs-effects");
    if (!toggle) throw new Error("Missing quick effects switch");
    return toggle;
  }

  async function fireConfigChanged() {
    if (!configChanged) throw new Error("config-changed listener not registered");
    const event = { event: "config-changed", id: 1, payload: null } satisfies Event<unknown>;
    configChanged(event);
    await tick();
  }

  it("persists edits on Play even when the debounced auto-save has not fired", async () => {
    const config = experimentalPlayConfig();
    const backend = mockSaveBackend({ getConfig: () => config });
    const view = await mountPlay();

    await fireEvent.click(effectsSwitch(view)); // quick-settings edit
    await fireEvent.input(view.getByRole("textbox"), { target: { value: "hello" } });
    await fireEvent.click(view.getByRole("button", { name: "Play" }));
    await vi.advanceTimersByTimeAsync(1);

    expect(backend.saved).toHaveLength(1); // the debounce never fired; flush wrote exactly once
    expect(backend.saved[0].tts.profiles[0].effects.enabled).toBe(true);
    expect(backend.cmdLog.indexOf("set_config")).toBeLessThan(backend.cmdLog.indexOf("speak_now"));
    expect(invoke).toHaveBeenCalledWith("speak_now", { text: "hello" });
    expect(view.queryByText(/Failed to save settings/)).toBeNull();
    view.unmount();
  });

  it("serializes a debounced save behind an in-flight one and persists edits made meanwhile", async () => {
    const config = experimentalPlayConfig();
    const backend = mockSaveBackend({ getConfig: () => config, holdFirstSave: true });
    const view = await mountPlay();

    await fireEvent.click(effectsSwitch(view)); // edit A: effects on
    await vi.advanceTimersByTimeAsync(SAVE_DEBOUNCE_MS); // save A starts, held in flight
    expect(backend.inFlight()).toBe(1);
    expect(backend.saved).toHaveLength(0);

    await fireEvent.click(effectsSwitch(view)); // edit B while save A is in flight
    await vi.advanceTimersByTimeAsync(SAVE_DEBOUNCE_MS); // B's debounce fires behind A
    expect(backend.inFlight()).toBe(1); // still only A
    expect(backend.saved).toHaveLength(0);

    backend.release();
    await vi.advanceTimersByTimeAsync(1);

    expect(backend.saved).toHaveLength(2);
    expect(backend.maxConcurrent()).toBe(1);
    expect(backend.saved[0].tts.profiles[0].effects.enabled).toBe(true);
    expect(backend.saved[1].tts.profiles[0].effects.enabled).toBe(false); // the mid-flight edit, not a stale snapshot
    view.unmount();
  });

  it("flushes a later edit behind an in-flight auto-save before Play synthesizes", async () => {
    const config = experimentalPlayConfig();
    const backend = mockSaveBackend({ getConfig: () => config, holdFirstSave: true });
    const view = await mountPlay();

    await fireEvent.click(effectsSwitch(view)); // effects on
    await vi.advanceTimersByTimeAsync(SAVE_DEBOUNCE_MS); // save 1 in flight
    await fireEvent.click(effectsSwitch(view)); // effects off, debounce re-armed
    await fireEvent.input(view.getByRole("textbox"), { target: { value: "hello" } });
    await fireEvent.click(view.getByRole("button", { name: "Play" })); // beats the debounce
    expect(backend.saved).toHaveLength(0); // flush is parked behind the in-flight write

    backend.release();
    await vi.advanceTimersByTimeAsync(1);

    expect(backend.saved).toHaveLength(2); // save 1, then the edit — no duplicate write
    expect(backend.maxConcurrent()).toBe(1);
    expect(backend.saved[1].tts.profiles[0].effects.enabled).toBe(false);
    expect(backend.cmdLog.indexOf("set_config")).toBeLessThan(backend.cmdLog.indexOf("speak_now"));
    expect(invoke).toHaveBeenCalledWith("speak_now", { text: "hello" });
    expect(view.queryByText(/Failed to save settings/)).toBeNull();
    view.unmount();
  });

  it("surfaces a failed save, blocks synthesis, and recovers on the next Play", async () => {
    const config = experimentalPlayConfig();
    const failing = { current: true };
    const backend = mockSaveBackend({ getConfig: () => config, failSaves: () => failing.current });
    const view = await mountPlay();

    await fireEvent.click(effectsSwitch(view));
    await fireEvent.input(view.getByRole("textbox"), { target: { value: "hello" } });
    await fireEvent.click(view.getByRole("button", { name: "Play" }));
    await vi.advanceTimersByTimeAsync(1);

    expect(view.queryByText(/Failed to save settings/)).not.toBeNull();
    expect(view.getByText(/disk full/)).not.toBeNull();
    expect(backend.cmdLog).not.toContain("speak_now"); // the failure blocked synthesis
    expect(backend.cmdLog).toContain("set_config"); // and the write was attempted

    failing.current = false;
    await fireEvent.click(view.getByRole("button", { name: "Play" }));
    await vi.advanceTimersByTimeAsync(1);

    expect(backend.cmdLog).toContain("speak_now");
    expect(view.queryByText(/Failed to save settings/)).toBeNull();
    view.unmount();
  });

  it("does not clobber unsaved edits when a config-changed broadcast arrives", async () => {
    const serverState = experimentalPlayConfig();
    const backend = mockSaveBackend({ getConfig: () => structuredClone(serverState) });
    const view = await mountPlay();

    await fireEvent.click(effectsSwitch(view)); // unsaved local edit, debounce pending
    serverState.playback.volume = 7; // another window saved its own change
    await fireConfigChanged();
    await vi.advanceTimersByTimeAsync(SAVE_DEBOUNCE_MS); // the edit's debounce fires

    expect(backend.saved).toHaveLength(1);
    expect(backend.saved[0].tts.profiles[0].effects.enabled).toBe(true); // the edit survived the broadcast
    expect(backend.saved[0].playback.volume).toBe(100); // not the broadcast's value
    expect(effectsSwitch(view).getAttribute("aria-checked")).toBe("true");

    // With no unsaved edits, a broadcast must re-sync the page instead. The
    // server still has effects off — the edit was local-only — so the
    // broadcast flips the switch back.
    serverState.playback.volume = 55;
    await fireConfigChanged();
    await vi.advanceTimersByTimeAsync(SAVE_DEBOUNCE_MS);

    expect(effectsSwitch(view).getAttribute("aria-checked")).toBe("false");
    expect(backend.saved).toHaveLength(1); // the re-sync never wrote back
    view.unmount();
  });
});
