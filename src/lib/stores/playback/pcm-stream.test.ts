import { afterEach, describe, expect, it, vi } from "vite-plus/test";
import { PcmStreamScheduler, type StreamChunkPayload } from "./pcm-stream";

interface RecordedBuffer {
  duration: number;
  samples: Float32Array[];
}

interface RecordedSource {
  buffer: RecordedBuffer | null;
  playbackRate: { value: number };
  onended: (() => void) | null;
  start: (at: number, offset?: number) => void;
  stop: () => void;
  connect: () => void;
  at: number;
  offset: number;
}

function createHarness() {
  const sources: RecordedSource[] = [];
  const ctx = {
    currentTime: 0,
    state: "running",
    destination: {},
    resume: () => {},
    suspend: () => {},
    createGain: () => ({
      gain: { value: 1 },
      connect: () => {},
      disconnect: () => {}
    }),
    createBuffer: (channels: number, length: number, sampleRate: number) => {
      const samples = Array.from({ length: channels }, () => new Float32Array(length));
      return {
        duration: length / sampleRate,
        samples,
        copyToChannel: (data: Float32Array, channel: number) => samples[channel].set(data)
      };
    },
    createBufferSource: () => {
      const source: RecordedSource = {
        buffer: null,
        playbackRate: { value: 1 },
        onended: null,
        at: 0,
        offset: 0,
        start: (at, offset = 0) => {
          source.at = at;
          source.offset = offset;
          sources.push(source);
        },
        stop: () => {},
        connect: () => {}
      };
      return source;
    }
  };
  vi.stubGlobal(
    "AudioContext",
    vi.fn(function () {
      return ctx;
    })
  );
  const audioContext = new AudioContext();
  const onComplete = vi.fn();
  const scheduler = new PcmStreamScheduler({
    ctx: audioContext,
    destination: audioContext.destination,
    onComplete
  });
  return { scheduler, sources, ctx, onComplete };
}

function chunk(bytes: Uint8Array, channels = 1, isFinal = false): StreamChunkPayload {
  return {
    audio_base64: btoa(Array.from(bytes, (byte) => String.fromCharCode(byte)).join("")),
    sample_rate: 24000,
    channels,
    bits_per_sample: 16,
    fragment_index: 0,
    fragment_total: 1,
    is_final: isFinal
  };
}

function pcmBytes(samples: number[]): Uint8Array {
  const bytes = new Uint8Array(samples.length * 2);
  const view = new DataView(bytes.buffer);
  samples.forEach((sample, index) => view.setInt16(index * 2, sample, true));
  return bytes;
}

function renderedChannel(sources: RecordedSource[], channel: number): number[] {
  return sources.flatMap((source) => Array.from(source.buffer?.samples[channel] ?? []));
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("PcmStreamScheduler", () => {
  it("reports the audible fragment and freezes its clock through pause and underruns", () => {
    const { scheduler, sources, ctx } = createHarness();
    const second = new Uint8Array(48000);
    scheduler.handleChunk(chunk(second));
    scheduler.handleChunk({ ...chunk(second), fragment_index: 1 });
    expect(scheduler.getPlaybackPosition()).toBeNull();
    ctx.currentTime = 0.53;
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(500);
    // Suspended AudioContexts keep the same currentTime, even as wall time passes.
    ctx.state = "suspended";
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(500);
    ctx.state = "running";
    ctx.currentTime = 1.28;
    expect(scheduler.getPlaybackPosition()?.fragmentIndex).toBe(1);
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(250);
    ctx.currentTime = 2.1;
    sources.forEach((source) => source.onended?.());
    expect(scheduler.getPlaybackPosition()).toBeNull();
    scheduler.stop();
    expect(scheduler.getPlaybackPosition()).toBeNull();
  });

  it("reports the position reaching the speakers, behind currentTime by outputLatency", () => {
    const { scheduler, sources, ctx } = createHarness();
    Object.assign(ctx, { outputLatency: 0.2 });
    const second = new Uint8Array(48000);
    scheduler.handleChunk(chunk(second));
    scheduler.handleChunk({ ...chunk(second), fragment_index: 1 });
    ctx.currentTime = 0.53;
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(300);
    // Fragment 0 has ended on the context clock but is still in the output pipeline.
    ctx.currentTime = 1.13;
    sources.filter((source) => source.at < 1).forEach((source) => source.onended?.());
    expect(scheduler.getPlaybackPosition()?.fragmentIndex).toBe(0);
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(900);
    ctx.currentTime = 1.33;
    expect(scheduler.getPlaybackPosition()?.fragmentIndex).toBe(1);
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(100);
    scheduler.stop();
  });

  it("keeps audible audio at the rate it was rendered at", () => {
    const { scheduler, ctx } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    ctx.currentTime = 0.28;
    scheduler.setRate(2, 1);
    // The audible source was time-stretched at speed 1 and cannot be retuned in
    // place, so its clock keeps advancing one native second per wall second.
    ctx.currentTime = 0.53;
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(500);
    scheduler.stop();
  });

  it.each([0.5, 2])("reschedules future fragments without gaps or overlap at rate %s", (rate) => {
    const { scheduler, sources, ctx } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    scheduler.handleChunk({ ...chunk(new Uint8Array(48000)), fragment_index: 1 });
    const oldFuture = sources[1];
    oldFuture.stop = vi.fn();
    ctx.currentTime = 0.53;
    scheduler.setRate(rate, 1);
    // The audible source still owes 0.5s of native audio at its rendered rate.
    const expectedStart = 1.03;
    expect(oldFuture.stop).toHaveBeenCalledOnce();
    expect(oldFuture.onended).toBeNull();
    expect(sources[2].at).toBeCloseTo(expectedStart);
    // The requeued chunk is re-stretched, so one native second now occupies
    // 1/rate wall seconds of buffer, minus the WSOLA window still held back
    // until the fragment is flushed.
    expect(sources[2].buffer!.duration).toBeGreaterThan(0.75 / rate);
    expect(sources[2].buffer!.duration).toBeLessThanOrEqual(1.02 / rate);
    ctx.currentTime = expectedStart + 0.1;
    expect(scheduler.getPlaybackPosition()?.fragmentIndex).toBe(1);
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(100 * rate);
    scheduler.stop();
  });

  it("flushes a short intermediate fragment without completing the queue", () => {
    const { scheduler, sources, onComplete } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(2400)));
    expect(sources).toHaveLength(0);
    scheduler.handleChunk({ ...chunk(new Uint8Array()), fragment_duration_ms: 50 });
    expect(sources).toHaveLength(1);
    sources[0].onended?.();
    expect(onComplete).not.toHaveBeenCalled();
    scheduler.stop();
  });
  it.each([1, 2])("preserves PCM samples across every byte split with %i channels", (channels) => {
    const samples = [0x1234, -0x1234, 32767, -32768, 1, -1, 8192, -8192];
    const bytes = pcmBytes(samples);
    for (let split = 1; split < bytes.length; split++) {
      const { scheduler, sources } = createHarness();
      scheduler.handleChunk(chunk(bytes.subarray(0, split), channels));
      scheduler.handleChunk(chunk(bytes.subarray(split), channels));
      scheduler.handleChunk(chunk(new Uint8Array(), channels, true));
      scheduler.markQueueComplete();
      for (let channel = 0; channel < channels; channel++) {
        expect(renderedChannel(sources, channel), `split=${split}, channel=${channel}`).toEqual(
          samples.filter((_, index) => index % channels === channel).map((sample) => sample / 32768)
        );
      }
      scheduler.stop();
    }
  });

  it("retains partial stereo frames through successive one-byte chunks", () => {
    const { scheduler, sources } = createHarness();
    for (const byte of pcmBytes([1234, -2345, 3456, -4567])) {
      scheduler.handleChunk(chunk(Uint8Array.of(byte), 2));
    }
    scheduler.handleChunk(chunk(new Uint8Array(), 2, true));
    scheduler.markQueueComplete();
    expect(renderedChannel(sources, 0)).toEqual([1234 / 32768, 3456 / 32768]);
    expect(renderedChannel(sources, 1)).toEqual([-2345 / 32768, -4567 / 32768]);
    scheduler.stop();
  });

  it("starts before the terminal marker and schedules jittered chunks contiguously", () => {
    const { scheduler, sources, ctx, onComplete } = createHarness();
    const samples = Array.from({ length: 9600 }, (_, index) => (index % 32768) - 16384);
    const bytes = pcmBytes(samples);
    const arrivals = [
      { end: 4001, time: 0 },
      { end: 9000, time: 0.08 },
      { end: 14001, time: 0.15 },
      { end: bytes.length, time: 0.27 }
    ];
    let offset = 0;
    for (const { end, time } of arrivals) {
      ctx.currentTime = time;
      scheduler.handleChunk(chunk(bytes.subarray(offset, end)));
      offset = end;
    }
    expect(renderedChannel(sources, 0)).toEqual(samples.map((sample) => sample / 32768));
    // The 150ms prebuffer fills with the third chunk (arriving at t=0.08).
    expect(sources[0].at).toBeCloseTo(0.11);
    for (let index = 1; index < sources.length; index++) {
      const previous = sources[index - 1];
      expect(sources[index].at).toBeCloseTo(previous.at + (previous.buffer?.duration ?? 0));
    }
    const audioSources = [...sources];
    scheduler.handleChunk(chunk(new Uint8Array(), 1, true));
    scheduler.markQueueComplete();
    expect(sources).toEqual(audioSources);
    expect(onComplete).not.toHaveBeenCalled();
    for (const source of sources) source.onended?.();
    expect(onComplete).toHaveBeenCalledOnce();
    scheduler.stop();
  });

  it("discards a truncated terminal frame rather than leaking it into the next fragment", () => {
    const { scheduler, sources } = createHarness();
    vi.spyOn(console, "warn").mockImplementation(() => {});
    scheduler.handleChunk(chunk(Uint8Array.of(0xff)));
    scheduler.handleChunk(chunk(new Uint8Array(), 1, true));
    scheduler.handleChunk({ ...chunk(pcmBytes([0x1234, -0x1234])), fragment_index: 1 });
    scheduler.handleChunk({ ...chunk(new Uint8Array(), 1, true), fragment_index: 1 });
    scheduler.markQueueComplete();
    expect(renderedChannel(sources, 0)).toEqual([0x1234 / 32768, -0x1234 / 32768]);
    scheduler.stop();
  });

  it("completes an empty terminal stream without scheduling audio", () => {
    const { scheduler, sources, onComplete } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(), 1, true));
    scheduler.markQueueComplete();
    expect(sources).toEqual([]);
    expect(onComplete).toHaveBeenCalledOnce();
    scheduler.stop();
  });

  it("completes on the terminal marker itself, without the 2s idle fallback", () => {
    const { scheduler, sources, onComplete } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    scheduler.handleChunk(chunk(new Uint8Array(), 1, true));
    // No markQueueComplete: only the authoritative is_final flag can finish the
    // queue, and it does so as the scheduled audio drains - not 2s later.
    for (const source of sources) source.onended?.();
    expect(onComplete).toHaveBeenCalledOnce();
    scheduler.stop();
  });

  it("rewinds through retained history and keeps fragment positions exact", () => {
    const { scheduler, sources, ctx } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    scheduler.handleChunk({ ...chunk(new Uint8Array(48000)), fragment_index: 1 });
    expect(sources).toHaveLength(2);
    ctx.currentTime = 1.5;
    sources[0].onended?.();
    scheduler.skipBackward(0.8);
    // Both retained buffers reschedule contiguously from now, the first one
    // entered 0.67s deep (0.8s back from media 1.47: the timeline starts at
    // the first source's scheduled start of 0.03, not at currentTime 0).
    expect(sources).toHaveLength(4);
    expect(sources[2].at).toBeCloseTo(1.53);
    expect(sources[2].offset).toBeCloseTo(0.67);
    expect(sources[3].at).toBeCloseTo(1.86);
    expect(sources[3].offset).toBeCloseTo(0);
    // The replaced source can no longer fire its end handler (the ended
    // fragment-0 source already fired and is not part of the rebuild).
    expect(sources[1].onended).toBeNull();
    ctx.currentTime = 1.53;
    expect(scheduler.getPlaybackPosition()?.fragmentIndex).toBe(0);
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(670);
    scheduler.stop();
  });

  it("fast-forwards through scheduled audio and remaps the captions", () => {
    const { scheduler, sources, ctx } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    scheduler.handleChunk({ ...chunk(new Uint8Array(48000)), fragment_index: 1 });
    ctx.currentTime = 0.5;
    scheduler.skipForward(0.8);
    // Target lands inside the retained fragment-1 buffer: it reschedules from
    // 0.27s in and the skipped fragment-0 sources are silenced.
    expect(sources).toHaveLength(3);
    expect(sources[2].at).toBeCloseTo(0.53);
    expect(sources[2].offset).toBeCloseTo(0.27);
    expect(sources[0].onended).toBeNull();
    expect(sources[1].onended).toBeNull();
    ctx.currentTime = 0.53;
    expect(scheduler.getPlaybackPosition()?.fragmentIndex).toBe(1);
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(270);
    scheduler.stop();
  });

  it("rewinding after a fast-forward returns to the pre-skip content", () => {
    const { scheduler, sources, ctx } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    scheduler.handleChunk({ ...chunk(new Uint8Array(48000)), fragment_index: 1 });
    ctx.currentTime = 0.5;
    scheduler.skipForward(0.8);
    ctx.currentTime = 0.55;
    scheduler.skipBackward(0.8);
    // Back into the skipped fragment-0 buffer: history entries survive the
    // first rebuild with their original spans intact.
    expect(sources[3].at).toBeCloseTo(0.58);
    expect(sources[3].offset).toBeCloseTo(0.49);
    // One tick past the rebuilt start (the exact boundary can underflow played
    // by float noise); fragment 0 resumes at native 0.49s.
    ctx.currentTime = 0.59;
    expect(scheduler.getPlaybackPosition()?.fragmentIndex).toBe(0);
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(500);
    scheduler.stop();
  });

  it("banks fast-forward overshoot as debt and drops arriving audio with it", () => {
    const { scheduler, sources, ctx, onComplete } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    ctx.currentTime = 0.5;
    scheduler.skipForward(2.5);
    // 1.97s past the buffered frontier: nothing to replay, nothing scheduled.
    expect(sources).toHaveLength(1);
    // The next whole second of audio is consumed by the debt...
    scheduler.handleChunk({ ...chunk(new Uint8Array(48000)), fragment_index: 1 });
    expect(sources).toHaveLength(1);
    // ...and the remainder trims the chunk after it to a 30ms tail.
    scheduler.handleChunk({ ...chunk(new Uint8Array(48000)), fragment_index: 1 });
    expect(sources).toHaveLength(2);
    expect(sources[1].buffer!.duration).toBeCloseTo(0.03);
    ctx.currentTime = 0.54;
    expect(scheduler.getPlaybackPosition()?.fragmentIndex).toBe(1);
    // The trim floor quantizes the skipped span to whole frames.
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(1980, 1);
    // Completion still waits for the tail to drain after the terminal marker.
    scheduler.handleChunk(chunk(new Uint8Array(), 1, true));
    expect(onComplete).not.toHaveBeenCalled();
    sources.at(-1)!.onended?.();
    expect(onComplete).toHaveBeenCalledOnce();
    scheduler.stop();
  });

  it("skips while the context is suspended and resumes into the rebuilt timeline", () => {
    const { scheduler, sources, ctx } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    ctx.state = "suspended";
    ctx.currentTime = 0.5;
    scheduler.skipBackward(0.2);
    // currentTime is frozen on pause; the rebuild anchors to it all the same.
    expect(sources).toHaveLength(2);
    expect(sources[1].at).toBeCloseTo(0.53);
    expect(sources[1].offset).toBeCloseTo(0.27);
    scheduler.stop();
  });

  it("ignores skips before anything has been scheduled and banks forward debt", () => {
    const { scheduler, sources } = createHarness();
    scheduler.skipBackward(5);
    expect(sources).toHaveLength(0);
    scheduler.skipForward(5);
    expect(sources).toHaveLength(0);
    // Five banked seconds swallow the first five arriving chunks wholesale...
    for (let i = 0; i < 5; i++) {
      scheduler.handleChunk(chunk(new Uint8Array(48000)));
    }
    expect(sources).toHaveLength(0);
    // ...and playback begins normally once the debt is paid off.
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    expect(sources).toHaveLength(1);
    expect(sources[0].buffer!.duration).toBeCloseTo(1);
    scheduler.stop();
  });

  it("keeps only ~30s of rewind history and clamps to its earliest span", () => {
    const { scheduler, sources, ctx } = createHarness();
    for (let i = 0; i < 31; i++) {
      scheduler.handleChunk(chunk(new Uint8Array(48000)));
    }
    ctx.currentTime = 2.2;
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    // Two fully-played seconds were evicted; rewind clamps to media 2. The
    // first rebuilt source enters its buffer at the start (offset 0).
    scheduler.skipBackward(1000);
    expect(sources[32].at).toBeCloseTo(2.23);
    expect(sources[32].offset).toBeCloseTo(0);
    ctx.currentTime = 2.23;
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(2000);
    scheduler.stop();
  });

  it("skips wall seconds at the rendered rate, not native seconds", () => {
    const { scheduler, sources, ctx } = createHarness();
    scheduler.setRate(2, 1);
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    // One native second rendered at speed 2 = ~half a wall second of buffer
    // (the WSOLA window holds back the rest until flush).
    ctx.currentTime = 0.3;
    scheduler.skipBackward(0.25);
    // 0.25 wall seconds back = 0.02 wall seconds into the rendered buffer =
    // 0.04 native seconds of content.
    expect(sources[1].at).toBeCloseTo(0.33);
    expect(sources[1].offset).toBeCloseTo(0.02);
    ctx.currentTime = 0.34;
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(60);
    scheduler.stop();
  });

  it("skips from the stalled playhead after the scheduled audio drains", () => {
    const { scheduler, sources, ctx } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    // Playback catches up with synthesis: the source drains and the caption
    // clock prunes it, leaving no source covering the playhead.
    ctx.currentTime = 1.5;
    sources[0].onended?.();
    scheduler.getPlaybackPosition();
    // A fast-forward from the stall banks the full 5s as debt instead of
    // computing its target from a collapsed position of 0.
    scheduler.skipForward(5);
    expect(sources).toHaveLength(1);
    // Five whole seconds of arriving audio are consumed by the debt...
    for (let i = 0; i < 5; i++) {
      scheduler.handleChunk({ ...chunk(new Uint8Array(48000)), fragment_index: 1 });
    }
    expect(sources).toHaveLength(1);
    // ...and playback resumes with the sixth chunk.
    scheduler.handleChunk({ ...chunk(new Uint8Array(48000)), fragment_index: 1 });
    expect(sources).toHaveLength(2);
    scheduler.stop();
  });

  it("rewinds from the stalled playhead instead of restarting the reading", () => {
    const { scheduler, sources, ctx } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    ctx.currentTime = 1.5;
    sources[0].onended?.();
    scheduler.getPlaybackPosition();
    scheduler.skipBackward(0.3);
    // Replays the drained source from 0.7s in instead of clamping to the
    // start of the timeline.
    expect(sources).toHaveLength(2);
    expect(sources[1].at).toBeCloseTo(1.53);
    expect(sources[1].offset).toBeCloseTo(0.7);
    ctx.currentTime = 1.55;
    expect(scheduler.getPlaybackPosition()?.positionMs).toBeCloseTo(720);
    scheduler.stop();
  });

  it("keeps a rapid second skip anchored during the rebuild's start delay", () => {
    const { scheduler, sources, ctx } = createHarness();
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    ctx.currentTime = 0.5;
    scheduler.skipBackward(0.2);
    // The first rebuild targets media 0.27; its source only becomes audible
    // at 0.53.
    expect(sources[1].at).toBeCloseTo(0.53);
    expect(sources[1].offset).toBeCloseTo(0.27);
    scheduler.skipBackward(0.2);
    // The second press lands before any rebuilt audio plays: it goes back
    // from the first target, not from a collapsed position of 0.
    expect(sources[2].at).toBeCloseTo(0.53);
    expect(sources[2].offset).toBeCloseTo(0.07);
    scheduler.stop();
  });

  it("consumes fast-forward debt in wall seconds at the rendered rate", () => {
    const { scheduler, sources, ctx } = createHarness();
    scheduler.setRate(2, 1);
    // One native second renders as half a wall second once the WSOLA window
    // is flushed.
    scheduler.handleChunk(chunk(new Uint8Array(48000)));
    scheduler.handleChunk({ ...chunk(new Uint8Array()), fragment_duration_ms: 500 });
    ctx.currentTime = 0.05;
    scheduler.skipForward(1);
    // 0.52 wall seconds of debt past the frontier swallow the next chunk
    // (0.5 wall) whole...
    scheduler.handleChunk({ ...chunk(new Uint8Array(48000)), fragment_index: 1 });
    expect(sources).toHaveLength(2);
    // ...and the timeline stays aligned with the audio that survives: the
    // trimmed next chunk reports the caption position at the skip target
    // (0.52 wall past the frontier = 1.04 native at speed 2) plus one tick.
    scheduler.handleChunk({ ...chunk(new Uint8Array(48000)), fragment_index: 1 });
    expect(sources).toHaveLength(3);
    ctx.currentTime = sources[2].at + 0.01;
    const position = scheduler.getPlaybackPosition();
    expect(position?.fragmentIndex).toBe(1);
    expect(position?.positionMs).toBeCloseTo(1060, 0);
    scheduler.stop();
  });
});
