import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { playbackStore } from "./playback-store.svelte";
import { emitTo } from "@tauri-apps/api/event";

const { listeners } = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => Promise<void>>()
}));
vi.mock("$lib/services/tauri.js", () => ({ isTauri: true }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name, callback) => {
    listeners.set(name, callback);
    return () => listeners.delete(name);
  }),
  emit: vi.fn(async () => {}),
  emitTo: vi.fn(async () => {})
}));
vi.mock("./playback/analyser.js", () => ({
  AudioAnalyser: class {
    getAnalyser() {
      return null;
    }
    setup() {}
    start() {}
    stop() {}
    destroy() {}
  }
}));

const decode = vi.fn(async () => ({ duration: 1 }));
let audio: HTMLAudioElement;

beforeEach(async () => {
  decode.mockReset().mockResolvedValue({ duration: 1 });
  vi.stubGlobal(
    "AudioContext",
    class {
      state = "running";
      decodeAudioData = decode;
      close = vi.fn();
    }
  );
  audio = document.createElement("audio");
  vi.spyOn(audio, "pause").mockImplementation(() => {});
  vi.spyOn(audio, "play").mockImplementation(async () => {
    audio.dispatchEvent(new Event("play"));
  });
  vi.spyOn(playbackStore, "buildPlaybackUrl").mockResolvedValue("blob:history-audio");
  playbackStore.setAudioElement(audio);
  playbackStore.historyReadingId = "batch:reading";
  await playbackStore.setupListeners();
});

afterEach(() => {
  playbackStore.teardownListeners();
  playbackStore.setAudioElement(null);
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

async function sendAudio(index?: number) {
  const name = index === undefined ? "audio-ready" : "audio-fragment-ready";
  await listeners.get(name)!({
    payload:
      index === undefined
        ? "YQ=="
        : {
            audio_base64: "YQ==",
            fragment_index: index,
            fragment_total: 2,
            is_final: index === 1,
            text: `Part ${index}`
          }
  });
}

it.each(["decode", "play"])(
  "releases the queue after a %s failure so history can retry",
  async (failure) => {
    if (failure === "decode") decode.mockRejectedValueOnce(new Error("broken audio"));
    else vi.mocked(audio.play).mockRejectedValueOnce(new Error("play rejected"));
    await sendAudio();
    expect(playbackStore.error).toContain("Audio playback failed");
    expect(playbackStore.isLoadingAudio).toBe(false);
    expect(playbackStore.isPlaying).toBe(false);
    await sendAudio();
    expect(decode).toHaveBeenCalledTimes(2);
    expect(playbackStore.isPlaying).toBe(true);
    expect(playbackStore.error).toBeNull();
  }
);

it("does not restart a stopped reading when its pending decode finishes", async () => {
  const pending = Promise.withResolvers<{ duration: number }>();
  decode.mockReturnValueOnce(pending.promise);
  const first = sendAudio(0);
  expect(playbackStore.isLoadingAudio).toBe(true);
  playbackStore.handleStop();
  pending.resolve({ duration: 1 });
  await first;
  expect(audio.play).not.toHaveBeenCalled();
  expect(playbackStore.isLoadingAudio).toBe(false);
  await sendAudio(0);
  expect(audio.play).toHaveBeenCalledTimes(1);
});

it("plays grouped fragments in order and retains the reading for Replay", async () => {
  await sendAudio(0);
  await sendAudio(1);
  expect(audio.play).toHaveBeenCalledTimes(1);
  expect(playbackStore.currentFragmentIndex).toBe(0);
  audio.dispatchEvent(new Event("ended"));
  await vi.waitFor(() => expect(audio.play).toHaveBeenCalledTimes(2));
  expect(playbackStore.currentFragmentIndex).toBe(1);
  audio.dispatchEvent(new Event("ended"));
  expect(playbackStore.isPlaying).toBe(false);
  expect(playbackStore.historyReadingId).toBe("batch:reading");
});

it("publishes captions from the audio clock and audible fragment, then stops publishing on Stop", async () => {
  vi.useFakeTimers();
  vi.spyOn(audio, "readyState", "get").mockReturnValue(4);
  vi.spyOn(audio, "duration", "get").mockReturnValue(1);
  await sendAudio(0);
  await sendAudio(1);
  audio.currentTime = 0.6;
  await vi.advanceTimersByTimeAsync(80);
  expect(emitTo).toHaveBeenLastCalledWith(
    "hud",
    "hud:caption",
    expect.objectContaining({ text: "Part 0", position_ms: 600, duration_ms: 1000, active: true })
  );
  audio.dispatchEvent(new Event("pause"));
  await vi.advanceTimersByTimeAsync(800);
  expect(emitTo).toHaveBeenLastCalledWith(
    "hud",
    "hud:caption",
    expect.objectContaining({ position_ms: 600, paused: true })
  );
  playbackStore.handleStop();
  vi.mocked(emitTo).mockClear();
  await vi.advanceTimersByTimeAsync(800);
  expect(emitTo).not.toHaveBeenCalled();
});
