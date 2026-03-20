/**
 * Audio utility functions for playback store
 * Handles audio format detection, WAV conversion, and bar value generation
 */

/**
 * Convert frequency data to bar values for HUD waveform.
 * Excludes first 2 and last 4 bars (they rarely activate during speech),
 * focusing on the 10 middle bars where speech energy is concentrated.
 */
export function buildBarValues(dataArray: Uint8Array, numBars: number): number[] {
    const binCount = dataArray.length;
    const bars: number[] = [];

    // Number of edge bars to exclude: 2 low freq + 4 high freq
    const edgeBars = 2 + 4;
    const activeBars = numBars - edgeBars;

    // First two bars: always zero (low frequencies rarely active)
    bars.push(0, 0);

    // Active middle bars: map to frequency bins with spread emphasis
    for (let i = 0; i < activeBars; i++) {
        // Use a curve that spreads middle frequencies more evenly
        // t goes from 0 to 1 across active bars
        const t = i / (activeBars - 1);
        // Apply power curve for better middle spread (0.6 emphasizes mids)
        const skewedT = Math.pow(t, 0.6);

        // Map to frequency bins (use 85% of range, avoiding extreme highs)
        const startBin = Math.floor(skewedT * binCount * 0.85);
        const nextT = Math.pow((i + 1) / (activeBars - 1), 0.6);
        const endBin = Math.floor(Math.min(nextT * binCount * 0.85, binCount));

        let sum = 0;
        for (let b = startBin; b < endBin; b++) {
            sum += dataArray[b];
        }
        const count = endBin - startBin || 1;
        bars.push(sum / count / 255); // normalize to 0.0–1.0
    }

    // Last four bars: always zero (high frequencies rarely active)
    bars.push(0, 0, 0, 0);

    return bars;
}

/**
 * Convert AudioBuffer to WAV Blob
 */
export function audioBufferToWavBlob(buffer: AudioBuffer): Blob {
    const numCh = buffer.numberOfChannels;
    const SR = buffer.sampleRate;
    const N = buffer.length;
    const dataSize = N * numCh * 2;
    const ab = new ArrayBuffer(44 + dataSize);
    const v = new DataView(ab);
    const ws = (o: number, s: string) => {
        for (let i = 0; i < s.length; i++) v.setUint8(o + i, s.charCodeAt(i));
    };
    ws(0, "RIFF");
    v.setUint32(4, 36 + dataSize, true);
    ws(8, "WAVE");
    ws(12, "fmt ");
    v.setUint32(16, 16, true);
    v.setUint16(20, 1, true);
    v.setUint16(22, numCh, true);
    v.setUint32(24, SR, true);
    v.setUint32(28, SR * numCh * 2, true);
    v.setUint16(32, numCh * 2, true);
    v.setUint16(34, 16, true);
    ws(36, "data");
    v.setUint32(40, dataSize, true);
    let off = 44;
    for (let i = 0; i < N; i++) {
        for (let c = 0; c < numCh; c++) {
            v.setInt16(
                off,
                Math.max(-1, Math.min(1, buffer.getChannelData(c)[i])) * 0x7fff,
                true,
            );
            off += 2;
        }
    }
    return new Blob([ab], { type: "audio/wav" });
}

/**
 * Detect audio format from bytes and return appropriate MIME type
 */
export function detectAudioMimeType(bytes: ArrayBuffer): string {
    const data = new Uint8Array(bytes.slice(0, 16));

    // WAV: starts with "RIFF" and has "WAVE" at position 8
    if (
        data[0] === 0x52 &&
        data[1] === 0x49 &&
        data[2] === 0x46 &&
        data[3] === 0x46 &&
        data[8] === 0x57 &&
        data[9] === 0x41 &&
        data[10] === 0x56 &&
        data[11] === 0x45
    ) {
        return "audio/wav";
    }

    // MP3: starts with MPEG sync word (0xFFE0-0xFFFF)
    // Common patterns: 0xFF 0xFB (MPEG1 Layer3), 0xFF 0xF3, 0xFF 0xF2
    if (data[0] === 0xff && (data[1] & 0xe0) === 0xe0) {
        return "audio/mpeg";
    }

    // OGG: starts with "OggS"
    if (
        data[0] === 0x4f &&
        data[1] === 0x67 &&
        data[2] === 0x67 &&
        data[3] === 0x53
    ) {
        return "audio/ogg";
    }

    // FLAC: starts with "fLaC"
    if (
        data[0] === 0x66 &&
        data[1] === 0x4c &&
        data[2] === 0x61 &&
        data[3] === 0x43
    ) {
        return "audio/flac";
    }

    // Default to MP3 (most common for TTS APIs like ElevenLabs)
    return "audio/mpeg";
}
