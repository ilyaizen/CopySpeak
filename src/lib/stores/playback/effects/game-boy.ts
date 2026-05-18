// 8-bit Game Boy bitcrush effect: quantize samples to 4-bit grid, then
// resample to 11_025 Hz via OfflineAudioContext for that aliased crunch.
// Duration is preserved (no pitch or speed change).

import type { Effect } from "./types";

const TARGET_SAMPLE_RATE = 11_025;
const BIT_DEPTH = 4;

function bitcrushInPlace(buffer: AudioBuffer, bitDepth: number) {
  const levels = Math.pow(2, bitDepth - 1);
  for (let ch = 0; ch < buffer.numberOfChannels; ch++) {
    const data = buffer.getChannelData(ch);
    for (let i = 0; i < data.length; i++) {
      data[i] = Math.round(data[i] * levels) / levels;
    }
  }
}

export const gameBoy: Effect = {
  id: "game_boy",
  async process(input: AudioBuffer, ctx: AudioContext): Promise<AudioBuffer> {
    const channels = input.numberOfChannels;
    const sourceRate = input.sampleRate;

    // Clone the input so we do not mutate the cached decoded buffer.
    const crushed = ctx.createBuffer(channels, input.length, sourceRate);
    for (let ch = 0; ch < channels; ch++) {
      crushed.getChannelData(ch).set(input.getChannelData(ch));
    }
    bitcrushInPlace(crushed, BIT_DEPTH);

    const outRate = Math.min(TARGET_SAMPLE_RATE, sourceRate);
    const outLen = Math.ceil(input.duration * outRate);
    const offline = new OfflineAudioContext(channels, outLen, outRate);

    const src = offline.createBufferSource();
    src.buffer = crushed;
    src.connect(offline.destination);
    src.start(0);

    return offline.startRendering();
  }
};
