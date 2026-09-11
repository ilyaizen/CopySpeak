import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { playbackStore } from "./playback-store.svelte";
import { emitTo } from "@tauri-apps/api/event";
import { PcmStreamScheduler } from "./playback/pcm-stream";

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
let playSpy: ReturnType<typeof vi.spyOn>;

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
  playSpy = vi.spyOn(audio, "play").mockImplementation(async () => {
    audio.dispatchEvent(new Event("play"));
  });
  vi.spyOn(playbackStore, "buildPlaybackUrl").mockResolvedValue("blob:history-audio");
  playbackStore.setAudioElement(audio);
  playbackStore.syncPlaybackConfig(100, 1, 1);
  playbackStore.historyReadingId = "batch:reading";
  await playbackStore.setupListeners();
});

afterEach(() => {
  playbackStore.teardownListeners();
  if (vi.isFakeTimers()) vi.runOnlyPendingTimers();
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
    else playSpy.mockRejectedValueOnce(new Error("play rejected"));
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
  expect(playSpy).not.toHaveBeenCalled();
  expect(playbackStore.isLoadingAudio).toBe(false);
  await sendAudio(0);
  expect(playSpy).toHaveBeenCalledTimes(1);
});

it("plays grouped fragments in order and retains the reading for Replay", async () => {
  await sendAudio(0);
  await sendAudio(1);
  expect(playSpy).toHaveBeenCalledTimes(1);
  expect(playbackStore.currentFragmentIndex).toBe(0);
  audio.dispatchEvent(new Event("ended"));
  await vi.waitFor(() => expect(playSpy).toHaveBeenCalledTimes(2));
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

// The rendered blob carries native duration - pitch shifting no longer resamples
// it - so the media clock is already caption time, with no correction factor.
it.each([
  [0.5, 500],
  [Number.NaN, 1000]
])(
  "replays native timings through pitch, speed, and pause (duration %s)",
  async (duration, expectedDurationMs) => {
    vi.useFakeTimers();
    vi.spyOn(audio, "readyState", "get").mockReturnValue(4);
    vi.spyOn(audio, "duration", "get").mockReturnValue(duration);
    const captions = {
      text: "Hello world",
      words: [
        { text_start: 0, text_end: 5, start_ms: 100, end_ms: 200 },
        { text_start: 6, text_end: 11, start_ms: 600, end_ms: 800 }
      ]
    };
    playbackStore.syncPlaybackConfig(100, 1.5, 2);
    await listeners.get("audio-fragment-ready")!({
      payload: {
        audio_base64: "YQ==",
        fragment_index: 0,
        fragment_total: 1,
        is_final: true,
        text: "Hello world",
        captions
      }
    });
    audio.currentTime = 0.3;
    // Changing the configured pitch cannot alter the already-rendered media clock.
    playbackStore.syncPlaybackConfig(100, 2, 0.5);
    await vi.advanceTimersByTimeAsync(25);
    expect(emitTo).toHaveBeenLastCalledWith(
      "hud",
      "hud:caption",
      expect.objectContaining({
        captions,
        position_ms: 300,
        duration_ms: expectedDurationMs
      })
    );
    audio.dispatchEvent(new Event("pause"));
    await vi.advanceTimersByTimeAsync(100);
    expect(emitTo).toHaveBeenLastCalledWith(
      "hud",
      "hud:caption",
      expect.objectContaining({ captions, position_ms: 300, paused: true })
    );
    playbackStore.syncPlaybackConfig(100, 1, 1);
  }
);

it("keeps streaming captions on the audible fragment and rejects chunks after Stop", async () => {
  vi.useFakeTimers();
  vi.stubGlobal(
    "AudioContext",
    class {
      state = "running";
      currentTime = 0;
      destination = {};
      createGain = () => ({ gain: { value: 1 }, connect() {}, disconnect() {} });
      resume = vi.fn();
      suspend = vi.fn(() => {
        this.state = "suspended";
      });
      close = vi.fn();
    }
  );
  const receive = vi
    .spyOn(PcmStreamScheduler.prototype, "handleChunk")
    .mockImplementation(() => {});
  vi.spyOn(PcmStreamScheduler.prototype, "getPlaybackPosition").mockReturnValue({
    fragmentIndex: 0,
    positionMs: 700
  });
  await listeners.get("synthesis-state-change")!({ payload: true });
  const captions = {
    text: "first",
    words: [{ text_start: 0, text_end: 5, start_ms: 650, end_ms: 800 }]
  };
  const payload = {
    audio_base64: "",
    sample_rate: 24000,
    channels: 1,
    bits_per_sample: 16,
    fragment_index: 0,
    fragment_total: 2,
    is_final: false,
    text: "first",
    captions
  };
  playbackStore.handleStreamChunk(payload);
  playbackStore.handleTogglePause();
  playbackStore.handleStreamChunk({
    ...payload,
    fragment_index: 1,
    text: "second",
    captions: { text: "second", words: [] }
  });
  expect(playbackStore.isPaused).toBe(true);
  expect(emitTo).toHaveBeenLastCalledWith(
    "hud",
    "hud:caption",
    expect.objectContaining({ text: "first", captions, position_ms: 700, paused: true })
  );
  playbackStore.handleStop();
  receive.mockClear();
  vi.mocked(emitTo).mockClear();
  playbackStore.handleStreamChunk(payload);
  await vi.advanceTimersByTimeAsync(500);
  expect(receive).not.toHaveBeenCalled();
  expect(emitTo).not.toHaveBeenCalled();
});
