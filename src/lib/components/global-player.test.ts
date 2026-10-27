import { afterEach, expect, it, vi } from "vite-plus/test";
import { cleanup, render, waitFor } from "@testing-library/svelte";
import { invoke } from "@tauri-apps/api/core";
import { playbackStore } from "#lib/stores/playback-store.svelte";
import GlobalPlayer from "./global-player.svelte";

vi.mock("#lib/services/tauri.js", () => ({ isTauri: true }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => undefined) }));

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

it("reports real webview playback, pause and stop to the tray", async () => {
  vi.spyOn(playbackStore, "setupListeners").mockResolvedValue();
  vi.spyOn(playbackStore, "teardownListeners").mockImplementation(() => {});
  playbackStore.isPlaying = false;
  playbackStore.isPaused = false;
  render(GlobalPlayer);
  await waitFor(() =>
    expect(invoke).toHaveBeenLastCalledWith("set_tray_playback_state", {
      playing: false,
      paused: false
    })
  );
  playbackStore.isPlaying = true;
  await waitFor(() =>
    expect(invoke).toHaveBeenLastCalledWith("set_tray_playback_state", {
      playing: true,
      paused: false
    })
  );
  playbackStore.isPaused = true;
  await waitFor(() =>
    expect(invoke).toHaveBeenLastCalledWith("set_tray_playback_state", {
      playing: true,
      paused: true
    })
  );
  playbackStore.isPlaying = false;
  playbackStore.isPaused = false;
  await waitFor(() =>
    expect(invoke).toHaveBeenLastCalledWith("set_tray_playback_state", {
      playing: false,
      paused: false
    })
  );
});
