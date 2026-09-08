#!/usr/bin/env python3
"""CLI wrapper for Piper (piper1-gpl) - used by CopySpeak.

Reads text (inline or from a file), loads the Piper voice model named by
--voice (resolved as ../voices/<voice>.onnx relative to this wrapper), synthesizes,
and writes a WAV file.

Invoked by CopySpeak via:
    uv run --project {engine_dir}/piper python scripts/copyspeak-piper.py \
        --text-file {input} --voice {voice} --output {output} [--device cuda]

With --serve the model is loaded once and stays in RAM, speaking protocol v2
(see src-tauri/src/tts/local_daemon.rs): the wrapper prints READY 2, then answers
one JSON request per stdin line with a format header, length-prefixed 16-bit LE
PCM chunks, and an end frame. Piper yields one chunk per sentence, so playback
starts after the first sentence rather than after the whole passage. Stdin EOF
ends the process, so the daemon dies with CopySpeak.

Place <voice>.onnx + <voice>.onnx.json in <engine_dir>/voices/. Get voices
from https://github.com/OHF-Voice/piper1-gpl#voices
"""

import argparse
import glob
import json
import os
import sys
import wave
import re
from difflib import SequenceMatcher
from pathlib import Path


def enable_cuda_dlls() -> None:
    """Windows: register the nvidia-* wheel DLL directories.

    onnxruntime-gpu and torch do not locate cuDNN/cuBLAS on their own, and since
    Python 3.8 the process PATH is ignored for extension-module dependencies —
    only os.add_dll_directory counts. Globs both the CUDA 12 wheel layout
    (nvidia/<pkg>/bin) and CUDA 13, which consolidates under nvidia/cu13/bin/<arch>.

    A no-op off Windows and when the nvidia-* wheels are not installed; the
    caller then fails loudly at session creation rather than silently on CPU.
    """
    if os.name != "nt":
        return
    try:
        import nvidia
    except ImportError:
        print(
            "WARNING: --device cuda but no nvidia-* runtime wheels in this project; "
            "re-run the installer with -Cuda",
            file=sys.stderr,
            flush=True,
        )
        return
    root = list(nvidia.__path__)[0]
    for pattern in ("*/bin", "*/bin/*"):
        for path in glob.glob(os.path.join(root, pattern)):
            if os.path.isdir(path):
                os.add_dll_directory(path)


def read_text(args) -> str:
    if args.text_file:
        return Path(args.text_file).read_text(encoding="utf-8")
    if args.text:
        return args.text
    print("ERROR: provide --text or --text-file", file=sys.stderr)
    sys.exit(2)


def resolve_model(name: str) -> Path:
    # Wrapper lives in <engine_dir>/scripts/; voices live in <engine_dir>/voices/.
    voices_dir = Path(__file__).resolve().parent.parent / "voices"
    # ponytail: resolve by exact basename, else first .onnx whose stem ends with the voice name.
    model = voices_dir / f"{name}.onnx"
    if model.exists():
        return model
    match = next((p for p in voices_dir.glob("*.onnx") if p.stem == name), None)
    if match is None:
        available = [p.stem for p in voices_dir.glob("*.onnx")]
        raise FileNotFoundError(f"voice model not found: {model}\n  Available: {available}")
    return match


def phone_groups(phonemes):
    """Lexical phone groups; stress marks don't change the word's identity."""
    groups = []
    current = ""
    for phone in phonemes:
        if phone.isspace() or phone in "^$.,;:!?—–\"()[]":
            if current:
                groups.append(current)
                current = ""
        elif phone not in "ˈˌ":
            current += phone
    if current:
        groups.append(current)
    return groups


def word_phone_map(voice, text):
    """Match text normalization to the full-context phone sequence, not to time.

    Only complete exact phone-group matches earn timings. Context-dependent
    pronunciations that differ are left unaligned, never interpolated.
    """
    expected = []
    tokens = []
    for match in re.finditer(r"\S+", text):
        phones = [p for sentence in voice.phonemize(match.group()) for p in sentence]
        groups = phone_groups(phones)
        start = len(expected)
        expected.extend(groups)
        if groups:
            tokens.append((match.start(), match.end(), start, len(expected)))
    actual = [group for sentence in voice.phonemize(text) for group in phone_groups(sentence)]
    mapping = {}
    for block in SequenceMatcher(None, expected, actual, autojunk=False).get_matching_blocks():
        for offset in range(block.size):
            mapping[block.a + offset] = block.b + offset
    result = []
    for start, end, first, last in tokens:
        indices = [mapping.get(i) for i in range(first, last)]
        if any(i is None for i in indices) or indices != list(range(indices[0], indices[0] + len(indices))):
            continue
        result.append((len(text[:start].encode("utf-16-le")) // 2,
                       len(text[:end].encode("utf-16-le")) // 2, indices[0], indices[-1]))
    return actual, result


def timed_phone_groups(alignments, sample_offset, rate):
    """Preserve native BOS, punctuation, and whitespace durations as silence."""
    groups = []
    current = ""
    start = end = sample_offset
    for item in alignments:
        phone = item.phoneme
        next_sample = sample_offset + item.num_samples
        if phone.isspace() or phone in "^$.,;:!?—–\"()[]":
            if current:
                groups.append((current, start * 1000 / rate, end * 1000 / rate))
                current = ""
        elif phone not in "ˈˌ":
            if not current:
                start = sample_offset
            current += phone
            end = next_sample
        sample_offset = next_sample
    if current:
        groups.append((current, start * 1000 / rate, end * 1000 / rate))
    return groups


def stream(voice, text: str):
    """Yield PCM plus native caption snapshots, one sentence at a time."""
    actual, tokens = word_phone_map(voice, text)
    groups = []
    sample_offset = 0
    for chunk in voice.synthesize(text, include_alignments=True):
        if not chunk.phoneme_alignments:
            raise RuntimeError("Piper voice has no phoneme alignments; reinstall with piper-tts[alignment]>=1.8.0")
        groups.extend(timed_phone_groups(chunk.phoneme_alignments, sample_offset, chunk.sample_rate))
        if [g[0] for g in groups] != actual[:len(groups)]:
            raise RuntimeError("Piper alignment phonemes differ from the synthesized text")
        words = []
        for start, end, first, last in tokens:
            if last < len(groups) and groups[last][2] > groups[first][1]:
                words.append({"text_start": start, "text_end": end,
                              "start_ms": groups[first][1], "end_ms": groups[last][2]})
        yield chunk.audio_int16_bytes, chunk.sample_rate, chunk.sample_channels, {"text": text, "words": words}
        sample_offset += len(chunk.audio_int16_bytes) // (2 * chunk.sample_channels)


def synthesize(voice, text: str, output: str) -> None:
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    with wave.open(output, "wb") as wf:
        captions = None
        for pcm, rate, channels, captions in stream(voice, text):
            if wf.getnframes() == 0:
                wf.setframerate(rate)
                wf.setnchannels(channels)
                wf.setsampwidth(2)
            wf.writeframes(pcm)
    if captions:
        Path(output + ".captions.json").write_text(json.dumps(captions), encoding="utf-8")


def serve(voice) -> int:
    """Daemon mode, protocol v2. Model stays resident; PCM is framed on stdout.

    Everything goes through sys.stdout.buffer — mixing text and binary writes to
    the same fd would let their separate buffers reorder the framing.

    Synthesis is warmed before the handshake: the first call after a model load
    pays one-off costs (phonemizer init, ONNX graph warmup) that later calls do
    not, and folding that into startup keeps the first real utterance as fast as
    every one after it. CopySpeak prewarms in the background, so this is free.
    """
    out = sys.stdout.buffer

    def frame(obj) -> None:
        out.write((json.dumps(obj) + "\n").encode("utf-8"))

    # Best effort: a warmup failure is not a reason to refuse to serve, and the
    # real request will surface the same error properly framed.
    try:
        for _ in stream(voice, "Ready."):
            pass
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"WARNING: warmup failed: {exc}", file=sys.stderr, flush=True)

    out.write(b"READY 2\n")
    out.flush()

    while True:
        line = sys.stdin.readline()
        if not line:
            return 0
        line = line.strip()
        if not line:
            continue
        try:
            chunks = stream(voice, json.loads(line)["text"])
            first = next(chunks, None)
            if first is None:
                raise RuntimeError("no audio produced")
            pcm, rate, channels, captions = first
            frame({
                "ok": True,
                "sample_rate": rate,
                "channels": channels,
                "bits_per_sample": 16,
            })
            while True:
                frame({"captions": captions})
                frame({"chunk": len(pcm)})
                out.write(pcm)
                out.flush()
                nxt = next(chunks, None)
                if nxt is None:
                    break
                pcm, rate, channels, captions = nxt
            frame({"end": True})
        except Exception as exc:  # pragma: no cover - environment dependent
            # Before the header this is the reply; after it, an error frame.
            # Rust treats both the same way.
            frame({"ok": False, "error": str(exc)})
        out.flush()


def main() -> int:
    parser = argparse.ArgumentParser(description="Piper CLI wrapper for CopySpeak")
    parser.add_argument("--text", help="Inline text to synthesize")
    parser.add_argument("--text-file", help="Path to a UTF-8 text file to synthesize")
    parser.add_argument("--voice", default="en_US-joe-medium", help="Voice model basename in voices/")
    parser.add_argument("--output", help="Output WAV file path (required unless --serve)")
    parser.add_argument("--serve", action="store_true", help="Keep the model in RAM and read JSON requests on stdin")
    parser.add_argument("--device", default="cpu", choices=["cpu", "cuda"], help="Inference device")
    args = parser.parse_args()

    if args.serve:
        text = None
    else:
        if not args.output:
            print("ERROR: --output is required unless --serve is given", file=sys.stderr)
            return 2
        text = read_text(args)

    try:
        model = resolve_model(args.voice)
    except FileNotFoundError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    if args.device == "cuda":
        enable_cuda_dlls()

    try:
        from piper import PiperVoice
    except ImportError as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: missing dependency: {exc}", file=sys.stderr)
        print("Reinstall with: ./scripts/install-piper.ps1 -Force", file=sys.stderr)
        return 1

    try:
        # use_cuda picks the CUDAExecutionProvider; without onnxruntime-gpu and
        # the nvidia-* wheels this raises rather than silently running on CPU.
        voice = PiperVoice.load(str(model), use_cuda=args.device == "cuda", include_alignments=True)
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    # The daemon log is where a user checks whether the GPU is actually in use.
    print(f"providers: {voice.session.get_providers()}", file=sys.stderr, flush=True)

    if args.serve:
        return serve(voice)

    try:
        synthesize(voice, text, args.output)
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    print(f"OK -> {args.output}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
