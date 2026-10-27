import { describe, expect, it, vi, beforeEach, afterEach } from "vite-plus/test";
import { render, waitFor } from "@testing-library/svelte";
import { waitForI18nReady } from "#lib/i18n";
import PlayPage from "./play-page.svelte";
import { invoke } from "@tauri-apps/api/core";
import { historyStore } from "#lib/stores/history-store.svelte";
import { testConfig } from "#lib/test-fixtures/app-config";

vi.mock("#lib/services/tauri.js", () => ({ isTauri: true }));

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
    hasCachedAudio: false,
    historyReadingId: null,
    startReading: vi.fn(),
    handleStop: vi.fn(),
    handleReplay: vi.fn(),
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
