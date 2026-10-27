import { afterEach, beforeEach, expect, it, vi } from "vite-plus/test";
import { playbackStore } from "./playback-store.svelte";
import { emit, emitTo, listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "svelte-sonner";
import { PcmStreamScheduler } from "./playback/pcm-stream";

const { listeners, live, ipc } = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => Promise<void>>(),
  // Every registration not yet unlistened, including ones the Map overwrote.
  live: new Set<unknown>(),
  // Every `listen` waits on this, so a test can hold setup mid-registration.
  ipc: { ready: Promise.resolve() }
}));
vi.mock("#lib/services/tauri.js", () => ({ isTauri: true }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => undefined) }));
vi.mock("svelte-sonner", () => ({ toast: { error: vi.fn(), warning: vi.fn() } }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name, callback) => {
    await ipc.ready;
    listeners.set(name, callback);
    live.add(callback);
    return () => {
      listeners.delete(name);
      live.delete(callback);
    };
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

async function sendAudio(index?: number, sessionId?: string, total = 2) {
  const name = index === undefined ? "audio-ready" : "audio-fragment-ready";
  await listeners.get(name)!({
    payload:
      index === undefined
        ? "YQ=="
        : {
            audio_base64: "YQ==",
            fragment_index: index,
            fragment_total: total,
            is_final: index === total - 1,
            text: `Part ${index}`,
            session_id: sessionId ?? null
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

it("keeps one set of listeners when HMR tears GlobalPlayer down mid-setup", async () => {
  const perSetup = live.size;
  playbackStore.teardownListeners();

  // HMR destroys GlobalPlayer while its setup awaits `listen`, then mounts a new one.
  const ready = Promise.withResolvers<void>();
  ipc.ready = ready.promise;
  vi.mocked(listen).mockClear();
  const first = playbackStore.setupListeners();
  await vi.waitFor(() => expect(listen).toHaveBeenCalled());
  playbackStore.teardownListeners();
  const second = playbackStore.setupListeners();
  ready.resolve();
  ipc.ready = Promise.resolve();
  await Promise.all([first, second]);

  expect(live.size).toBe(perSetup);
});

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

it("emits playback-finished when a reading ends and when it is stopped", async () => {
  await sendAudio();
  audio.dispatchEvent(new Event("ended"));
  await vi.waitFor(() => expect(emit).toHaveBeenCalledWith("playback-finished", null));
  vi.mocked(emit).mockClear();
  playbackStore.handleStop();
  expect(emit).toHaveBeenCalledWith("playback-finished", null);
});

it("reports an incomplete reading and clears it when the next reading starts", async () => {
  await listeners.get("synthesis-state-change")!({ payload: true });
  await sendAudio(0);
  await sendAudio(1);
  const message =
    "Reading stopped: section 3 of 5 failed, so the rest of the passage was not read. HTTP 503";
  await listeners.get("reading-failed")!({ payload: message });
  expect(playbackStore.error).toBe(message);
  expect(toast.error).toHaveBeenCalledWith(message, { duration: 15000 });
  // A new reading owns the failure state from here on.
  await listeners.get("synthesis-state-change")!({ payload: true });
  expect(playbackStore.error).toBeNull();
});

it("warns about clipboard truncation independently of the active route", async () => {
  await listeners.get("text-truncated")!({
    payload: { original_length: 120000, truncated_length: 99999, max_length: 100000 }
  });
  expect(toast.warning).toHaveBeenCalledWith(expect.stringContaining("120000 to 99999"), {
    duration: 15000
  });
});

it("does not label a failed browser reading completed when its audio drains", async () => {
  await listeners.get("synthesis-state-change")!({ payload: true });
  await sendAudio(0, "b1");
  await vi.waitFor(() => expect(playSpy).toHaveBeenCalledTimes(1));
  await listeners.get("reading-failed")!({
    payload: "Reading stopped: section 2 of 5 failed, so the rest of the passage was not read."
  });
  audio.dispatchEvent(new Event("ended"));
  await vi.waitFor(() =>
    expect(vi.mocked(invoke)).toHaveBeenCalledWith("browser_reading_finished", {
      sessionId: "b1",
      status: "error"
    })
  );
});

it("releases a reading waiting for its next fragment when the section failure lands", async () => {
  vi.mocked(emit).mockClear();
  await sendAudio(0);
  await vi.waitFor(() => expect(playSpy).toHaveBeenCalledTimes(1));
  audio.dispatchEvent(new Event("ended"));
  // The next fragment is still synthesizing: the gap must not complete anything.
  expect(emit).not.toHaveBeenCalledWith("playback-finished", null);
  await listeners.get("reading-failed")!({
    payload: "Reading stopped: section 2 of 5 failed, so the rest of the passage was not read."
  });
  await vi.waitFor(() => expect(emit).toHaveBeenCalledWith("playback-finished", null));
});

it("replays the rendered audio, not a previous reading's fragments, once a new reading starts", async () => {
  await sendAudio(0);
  await sendAudio(1);
  audio.dispatchEvent(new Event("ended"));
  await vi.waitFor(() => expect(playSpy).toHaveBeenCalledTimes(2));
  audio.dispatchEvent(new Event("ended"));
  // A new reading that never queues fragments (streamed) starts.
  await listeners.get("synthesis-state-change")!({ payload: true });
  await playbackStore.handleReplay();
  // The queue replay would activate fragment 0; the rendered-audio fallback
  // plays without touching the fragment index.
  expect(playbackStore.currentFragmentIndex).toBeNull();
  expect(playSpy).toHaveBeenCalledTimes(3);
});

it("releases a queued reading when the bridge cancels its session", async () => {
  vi.mocked(emit).mockClear();
  await sendAudio(0, "trigger-a");
  await vi.waitFor(() => expect(playSpy).toHaveBeenCalledTimes(1));
  audio.dispatchEvent(new Event("ended"));
  expect(emit).not.toHaveBeenCalledWith("playback-finished", null);
  await listeners.get("browser-reading")!({
    payload: { active: false, session_id: "trigger-a" }
  });
  await vi.waitFor(() => expect(emit).toHaveBeenCalledWith("playback-finished", null));
});

it("replays every fragment from the beginning on the Play page", async () => {
  await sendAudio(0);
  await sendAudio(1);
  audio.dispatchEvent(new Event("ended"));
  await vi.waitFor(() => expect(playSpy).toHaveBeenCalledTimes(2));
  audio.dispatchEvent(new Event("ended"));
  await playbackStore.handleReplay();
  expect(playbackStore.currentFragmentIndex).toBe(0);
  expect(playSpy).toHaveBeenCalledTimes(3);
  audio.dispatchEvent(new Event("ended"));
  await vi.waitFor(() => expect(playSpy).toHaveBeenCalledTimes(4));
  expect(playbackStore.currentFragmentIndex).toBe(1);
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

it("delays element captions by the AudioContext output latency in media time", async () => {
  vi.useFakeTimers();
  vi.stubGlobal(
    "AudioContext",
    class {
      state = "running";
      outputLatency = 0.2;
      decodeAudioData = decode;
      close = vi.fn();
    }
  );
  vi.spyOn(audio, "readyState", "get").mockReturnValue(4);
  vi.spyOn(audio, "duration", "get").mockReturnValue(1);
  await sendAudio(0);
  audio.playbackRate = 1.5;
  audio.currentTime = 0.6;
  await vi.advanceTimersByTimeAsync(25);
  expect(emitTo).toHaveBeenLastCalledWith(
    "hud",
    "hud:caption",
    expect.objectContaining({ position_ms: expect.closeTo(300) })
  );
  audio.currentTime = 0.1;
  await vi.advanceTimersByTimeAsync(25);
  expect(emitTo).toHaveBeenLastCalledWith(
    "hud",
    "hud:caption",
    expect.objectContaining({ position_ms: 0 })
  );
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

it("does not resurrect idle audio when pause is toggled with nothing playing", () => {
  expect(playbackStore.isPlaying).toBe(false);
  playbackStore.handleTogglePause();
  expect(playSpy).not.toHaveBeenCalled();
  expect(playbackStore.isPaused).toBe(false);
});

it("delegates skips to the PCM scheduler while streaming", async () => {
  vi.stubGlobal(
    "AudioContext",
    class {
      state = "running";
      currentTime = 0;
      destination = {};
      createGain = () => ({ gain: { value: 1 }, connect() {}, disconnect() {} });
      resume = vi.fn();
      suspend = vi.fn();
      close = vi.fn();
    }
  );
  const backward = vi
    .spyOn(PcmStreamScheduler.prototype, "skipBackward")
    .mockImplementation(() => {});
  const forward = vi
    .spyOn(PcmStreamScheduler.prototype, "skipForward")
    .mockImplementation(() => {});
  await listeners.get("synthesis-state-change")!({ payload: true });
  playbackStore.handleStreamChunk({
    audio_base64: "",
    sample_rate: 24000,
    channels: 1,
    bits_per_sample: 16,
    fragment_index: 0,
    fragment_total: 1,
    is_final: false
  });
  expect(playbackStore.isPlaying).toBe(true);
  playbackStore.skipBackward();
  expect(backward).toHaveBeenCalledWith(5);
  playbackStore.skipForward();
  expect(forward).toHaveBeenCalledWith(5);
  playbackStore.handleStop();
});

it("skips the element clock by wall seconds on the element path", async () => {
  vi.spyOn(audio, "duration", "get").mockReturnValue(10);
  await sendAudio();
  expect(playbackStore.isPlaying).toBe(true);
  audio.currentTime = 4;
  playbackStore.skipForward();
  expect(audio.currentTime).toBe(9);
  playbackStore.skipForward();
  expect(audio.currentTime).toBe(10);
  audio.currentTime = 1;
  playbackStore.skipBackward();
  expect(audio.currentTime).toBe(0);
});

it("rewinds across a fragment boundary through the retained previous fragment", async () => {
  vi.spyOn(audio, "duration", "get").mockReturnValue(2);
  await sendAudio(0);
  await sendAudio(1);
  audio.dispatchEvent(new Event("ended"));
  await vi.waitFor(() => expect(playbackStore.currentFragmentIndex).toBe(1));
  audio.currentTime = 0.5;
  // One second back from 0.5s into the fragment: 0.5s inside it, 0.5s owed to
  // the retained previous fragment, played from its tail.
  playbackStore.skipBackward(1);
  await vi.waitFor(() => expect(audio.currentTime).toBeCloseTo(1.5));
  expect(playbackStore.currentFragmentIndex).toBe(0);
  // When the rewind target ends, the interrupted fragment follows from its start.
  audio.dispatchEvent(new Event("ended"));
  await vi.waitFor(() => expect(playbackStore.currentFragmentIndex).toBe(1));
});

it("drives pause, resume, clamped skips and stop from page control events", async () => {
  vi.spyOn(audio, "duration", "get").mockReturnValue(10);
  vi.spyOn(audio, "paused", "get").mockImplementation(() => playbackStore.isPaused);
  await sendAudio();
  expect(playbackStore.isPlaying).toBe(true);
  await listeners.get("playback-toggle-pause")!({ payload: null });
  expect(playbackStore.isPaused).toBe(true);
  await listeners.get("playback-toggle-pause")!({ payload: null });
  expect(playbackStore.isPaused).toBe(false);
  audio.currentTime = 8;
  await listeners.get("playback-skip-forward")!({ payload: null });
  expect(audio.currentTime).toBe(10);
  audio.currentTime = 3;
  await listeners.get("playback-skip-backward")!({ payload: null });
  expect(audio.currentTime).toBe(0);
  await listeners.get("playback-stop")!({ payload: null });
  expect(playbackStore.isPlaying).toBe(false);
});

function streamChunk(sessionId: string | null) {
  return {
    audio_base64: "",
    sample_rate: 24000,
    channels: 1,
    bits_per_sample: 16,
    fragment_index: 0,
    fragment_total: 1,
    is_final: false,
    text: "streamed",
    session_id: sessionId
  };
}

// A new synthesis clears the stop left by the previous reading, as in the app.
async function startStreamingReading() {
  vi.stubGlobal(
    "AudioContext",
    class {
      state = "running";
      currentTime = 0;
      destination = {};
      createGain = () => ({ gain: { value: 1 }, connect() {}, disconnect() {} });
      resume = vi.fn();
      suspend = vi.fn();
      close = vi.fn();
    }
  );
  await listeners.get("synthesis-state-change")!({ payload: true });
}

it("leaves the playing stream alone when another streamed reading fails", async () => {
  await startStreamingReading();
  vi.mocked(emit).mockClear();
  playbackStore.handleStreamChunk(streamChunk("trigger-a"));
  await listeners.get("playback-stream-failed")!({ payload: { session_id: "trigger-b" } });
  expect(playbackStore.isPlaying).toBe(true);
  expect(emit).not.toHaveBeenCalledWith("playback-finished", null);
  playbackStore.handleStop();
});

it("leaves a reading that is still synthesizing alone when an idle stream fails", async () => {
  await startStreamingReading();
  await listeners.get("playback-stream-failed")!({ payload: { session_id: "trigger-a" } });
  playbackStore.handleStreamChunk(streamChunk("trigger-b"));
  expect(playbackStore.isPlaying).toBe(true);
  playbackStore.handleStop();
});

it("stops the streamed reading that fails while it is playing", async () => {
  await startStreamingReading();
  vi.mocked(emit).mockClear();
  playbackStore.handleStreamChunk(streamChunk("trigger-a"));
  await listeners.get("playback-stream-failed")!({ payload: { session_id: "trigger-a" } });
  expect(playbackStore.isPlaying).toBe(false);
  expect(emit).toHaveBeenCalledWith("playback-finished", null);
});

it("finishes a failed streamed reading when nothing is playing, so /speak --wait returns", async () => {
  vi.mocked(emit).mockClear();
  await listeners.get("playback-stream-failed")!({ payload: { session_id: null } });
  expect(emit).toHaveBeenCalledWith("playback-finished", null);
});

it("reports a followed page session by id, and only the current one", async () => {
  const calls = () => vi.mocked(invoke).mock.calls;
  vi.mocked(invoke).mockClear();
  await sendAudio(0, "trigger-1", 1);
  expect(calls()).toContainEqual([
    "browser_reading_progress",
    expect.objectContaining({ sessionId: "trigger-1", fragmentIndex: 0 })
  ]);
  playbackStore.handleStop();
  expect(calls()).toContainEqual([
    "browser_reading_finished",
    { sessionId: "trigger-1", status: "cancelled" }
  ]);

  // A late end notice for a replaced session leaves its successor followed.
  vi.mocked(invoke).mockClear();
  await sendAudio(0, "b", 1);
  await listeners.get("browser-reading")!({ payload: { active: false, session_id: "a" } });
  audio.dispatchEvent(new Event("ended"));
  expect(calls()).toContainEqual([
    "browser_reading_finished",
    { sessionId: "b", status: "completed" }
  ]);
});

it("announces each fragment as it becomes audible, with its text", async () => {
  vi.mocked(emit).mockClear();
  await sendAudio(0);
  await vi.waitFor(() =>
    expect(emit).toHaveBeenCalledWith("playback-fragment-audible", { index: 0, text: "Part 0" })
  );
});

// Repro: a queued trigger B starts while A's clip still plays. A's queue drains
// before B's audio arrives, so that drain must not complete anything.
it("does not complete a queued trigger when the earlier clip drains before its audio arrives", async () => {
  const calls = () => vi.mocked(invoke).mock.calls;
  vi.mocked(invoke).mockClear();
  await sendAudio(0, "trigger-a", 1);
  // The bridge cancels A when B starts; B's clip has not arrived yet.
  await listeners.get("browser-reading")!({ payload: { active: false, session_id: "trigger-a" } });
  audio.dispatchEvent(new Event("ended"));
  expect(calls().filter(([name]) => name === "browser_reading_finished")).toEqual([]);
});

it("keeps following a queued trigger after the earlier clip drains, once its audio arrives", async () => {
  const calls = () => vi.mocked(invoke).mock.calls;
  await sendAudio(0, "trigger-a", 1);
  await listeners.get("browser-reading")!({ payload: { active: false, session_id: "trigger-a" } });
  audio.dispatchEvent(new Event("ended"));
  vi.mocked(invoke).mockClear();
  await sendAudio(0, "trigger-b", 1);
  expect(calls()).toContainEqual([
    "browser_reading_progress",
    expect.objectContaining({ sessionId: "trigger-b", fragmentIndex: 0 })
  ]);
  audio.dispatchEvent(new Event("ended"));
  expect(calls()).toContainEqual([
    "browser_reading_finished",
    { sessionId: "trigger-b", status: "completed" }
  ]);
});

it("binds a streamed clip to the session it belongs to once audible", async () => {
  vi.useFakeTimers();
  vi.stubGlobal(
    "AudioContext",
    class {
      state = "running";
      currentTime = 0;
      destination = {};
      createGain = () => ({ gain: { value: 1 }, connect() {}, disconnect() {} });
      resume = vi.fn();
      suspend = vi.fn();
      close = vi.fn();
    }
  );
  vi.spyOn(PcmStreamScheduler.prototype, "handleChunk").mockImplementation(() => {});
  vi.spyOn(PcmStreamScheduler.prototype, "getPlaybackPosition").mockReturnValue({
    fragmentIndex: 0,
    positionMs: 100
  });
  await listeners.get("synthesis-state-change")!({ payload: true });
  vi.mocked(invoke).mockClear();
  playbackStore.handleStreamChunk({
    audio_base64: "",
    sample_rate: 24000,
    channels: 1,
    bits_per_sample: 16,
    fragment_index: 0,
    fragment_total: 1,
    is_final: false,
    text: "streamed",
    session_id: "trigger-s"
  });
  expect(invoke).toHaveBeenCalledWith(
    "browser_reading_progress",
    expect.objectContaining({ sessionId: "trigger-s", fragmentIndex: 0 })
  );
  playbackStore.handleStop();
});
