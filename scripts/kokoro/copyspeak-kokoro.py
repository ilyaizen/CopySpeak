#!/usr/bin/env python3
"""CLI wrapper for Kokoro (kokoro-onnx) - used by CopySpeak.

Replaces the third-party `kokoro-tts` binary: that CLI reloads its ~310 MB ONNX
model on every invocation and exposes no way to choose an execution provider, so
neither the resident daemon nor GPU inference was reachable through it.

Reads text (inline or from a file), synthesizes with kokoro-onnx, and writes a
24kHz WAV file.

Invoked by CopySpeak via:
    uv run --project {engine_dir}/kokoro python {engine_dir}/kokoro/scripts/copyspeak-kokoro.py \
        --text-file {input} --voice {voice} --output {output} [--device cuda]

With --serve the model is loaded once and stays in RAM, speaking protocol v2
(see src-tauri/src/tts/local_daemon.rs): READY 2, then one JSON request per
stdin line answered with a format header, length-prefixed 16-bit LE PCM chunks,
and an end frame. Stdin EOF ends the process.

Model files live in <engine_dir>/kokoro/models/ and are downloaded by
install-kokoro.ps1.
"""

import argparse
import contextlib
import glob
import json
import os
import sys
import wave
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


def resolve_models(model_arg, voices_arg):
    """Locate kokoro-v1.0-duration.onnx and voices-v1.0.bin.

    Wrapper lives in <engine_dir>/kokoro/scripts/; models in ../models/.
    """
    models_dir = Path(__file__).resolve().parent.parent / "models"
    model = Path(model_arg) if model_arg else models_dir / "kokoro-v1.0-duration.onnx"
    voices = Path(voices_arg) if voices_arg else models_dir / "voices-v1.0.bin"
    missing = [str(p) for p in (model, voices) if not p.exists()]
    if missing:
        raise FileNotFoundError(
            "model files not found: " + ", ".join(missing) +
            "\n  Re-run: ./scripts/install-kokoro.ps1 -Force"
        )
    return str(model), str(voices)


def pcm16(samples) -> bytes:
    """float32 in [-1, 1] -> signed 16-bit little-endian PCM bytes."""
    import numpy as np

    clipped = np.clip(np.asarray(samples, dtype=np.float32), -1.0, 1.0)
    return (clipped * 32767.0).astype("<i2").tobytes()


def caption_batches(text, tokens, vocab):
    """Keep source ownership while packing whole native tokens into ONNX windows."""
    if "".join(t.text + t.whitespace for t in tokens) != text:
        raise ValueError("Kokoro frontend did not preserve the original text")
    phones, spans, offset = "", [], 0
    for token in tokens:
        if len(token.text.split()) > 1:
            raise ValueError("Kokoro cannot caption merged words; replace tabs/unusual spaces with ordinary spaces")
        ps = token.phonemes
        if ps is None or any(p.isalpha() and p not in vocab for p in ps):
            raise ValueError(f"Kokoro cannot map pronunciation to model tokens: {token.text!r}")
        # misaki passes punctuation it cannot pronounce straight through as a
        # phoneme (an unbalanced "[", a guillemet). Those carry no sound and are
        # absent from Kokoro's vocab, so drop them rather than abort the reading.
        # A missing *letter* phoneme is a real pronunciation gap - stay loud.
        ps = "".join(p for p in ps if p in vocab)
        if len(ps) > 510:
            raise ValueError("Kokoro source token exceeds the model window; shorten the word/expression")
        if len(phones) + len(ps) > 510:
            yield phones.rstrip(), spans
            phones, spans = "", []
        first = len(phones)
        phones += ps
        if any(p.isalpha() for p in ps):
            # Keep numeric/emoji expansions as their native source token, never
            # subdivide them by the number of words in their pronunciation.
            left = offset + len(token.text) - len(token.text.lstrip())
            right = offset + len(token.text.rstrip())
            spans.append((len(text[:left].encode("utf-16-le")) // 2,
                          len(text[:right].encode("utf-16-le")) // 2,
                          first, len(phones)))
        if token.whitespace and phones:
            phones += " "
        offset += len(token.text + token.whitespace)
    if phones.strip():
        yield phones.rstrip(), spans


def native_words(text_spans, durations, sample_count, sample_offset):
    """Native Kokoro frames are 600 samples at 24 kHz, including BOS/EOS."""
    edges = [0]
    for duration in durations:
        if int(duration) != duration or duration <= 0:
            raise ValueError("Kokoro returned invalid native durations")
        edges.append(edges[-1] + int(duration) * 600)
    if edges[-1] != sample_count:
        raise ValueError("Kokoro duration geometry differs from its PCM; refusing to rescale")
    return [{"text_start": start, "text_end": end,
             "start_ms": (sample_offset + edges[first + 1]) / 24,
             "end_ms": (sample_offset + edges[last + 1]) / 24}
            for start, end, first, last in text_spans]


def stream(kokoro, voice: str, text: str, frontend):
    """Yield one complete alignment before a fragment's buffered PCM.

    Bypass create_timed's rescaling/trimming/pause insertion. The raw inference
    durations and waveform must come from the same call.
    """
    import numpy as np

    # Third-party frontend diagnostics must never enter the binary protocol.
    with contextlib.redirect_stdout(sys.stderr):
        _, tokens = frontend(text, preprocess=False)
    vocab = kokoro.tokenizer.vocab
    pcm, words, sample_offset = [], [], 0
    # Validate the entire fragment before inference or releasing any PCM.
    batches = list(caption_batches(text, tokens, vocab))
    for phones, spans in batches:
        ids = [vocab[p] for p in phones]
        samples, durations = kokoro.sess.run(["waveform", "duration"], {
            "input_ids": np.array([[0, *ids, 0]], dtype=np.int64),
            "style": kokoro.get_voice_style(voice)[len(ids) - 1].reshape(1, 256).astype(np.float32),
            # Playback speed/pitch belong to CopySpeak's TimeStretcher.
            "speed": np.array([1.0], dtype=np.float32),
        })
        samples, durations = samples.reshape(-1), durations.reshape(-1)
        if len(durations) != len(ids) + 2 or not np.isfinite(samples).all():
            raise ValueError("Kokoro native alignment does not match the generated audio")
        words.extend(native_words(spans, durations, len(samples), sample_offset))
        pcm.append(pcm16(samples))
        sample_offset += len(samples)
    if not pcm:
        raise ValueError("Kokoro produced no pronunciation for this text")
    yield b"".join(pcm), 24000, 1, {"text": text, "words": words}


def write_wav(output: str, pcm: bytes, rate: int, channels: int) -> None:
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    with wave.open(output, "wb") as wf:
        wf.setnchannels(channels)
        wf.setsampwidth(2)
        wf.setframerate(rate)
        wf.writeframes(pcm)


def serve(kokoro, voice: str, frontend) -> int:
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
        for _ in stream(kokoro, voice, "Ready.", frontend):
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
            pcm, rate, channels, captions = next(stream(kokoro, voice, json.loads(line)["text"], frontend))
            frame({
                "ok": True,
                "sample_rate": rate,
                "channels": channels,
                "bits_per_sample": 16,
            })
            frame({"captions": captions})
            frame({"chunk": len(pcm)})
            out.write(pcm)
            frame({"end": True})
        except Exception as exc:  # pragma: no cover - environment dependent
            frame({"ok": False, "error": str(exc)})
        out.flush()


def main() -> int:
    parser = argparse.ArgumentParser(description="Kokoro CLI wrapper for CopySpeak")
    parser.add_argument("--text", help="Inline text to synthesize")
    parser.add_argument("--text-file", help="Path to a UTF-8 text file to synthesize")
    parser.add_argument("--voice", default="af_heart", help="Kokoro voice id (e.g. af_heart)")
    parser.add_argument("--model", help="Path to duration-capable Kokoro ONNX (default: ../models/kokoro-v1.0-duration.onnx)")
    parser.add_argument("--voices", help="Path to voices-v1.0.bin (default: ../models/)")
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
        model_path, voices_path = resolve_models(args.model, args.voices)
    except FileNotFoundError as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    # kokoro-onnx reads ONNX_PROVIDER when building its session — a supported
    # hook, so no private API and no hand-built InferenceSession. Must be set
    # before the import that creates the session.
    if args.device == "cuda":
        enable_cuda_dlls()
        os.environ["ONNX_PROVIDER"] = "CUDAExecutionProvider"

    try:
        from kokoro_onnx import Kokoro
    except ImportError as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: missing dependency: {exc}", file=sys.stderr)
        print("Reinstall with: ./scripts/install-kokoro.ps1 -Force", file=sys.stderr)
        return 1

    try:
        kokoro = Kokoro(model_path, voices_path)
        if not {"waveform", "duration"}.issubset(o.name for o in kokoro.sess.get_outputs()):
            raise ValueError("Kokoro needs the duration-capable model; re-run install-kokoro.ps1")
        if args.voice[:1] not in ("a", "b"):
            raise ValueError("Kokoro native source captions currently support English voices only")
        with contextlib.redirect_stdout(sys.stderr):
            from misaki import en, espeak
            british = args.voice.startswith("b")
            frontend = en.G2P(trf=False, british=british, fallback=espeak.EspeakFallback(british=british))
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    # The daemon log is where a user checks whether the GPU is actually in use.
    print(f"providers: {kokoro.sess.get_providers()}", file=sys.stderr, flush=True)

    if args.serve:
        return serve(kokoro, args.voice, frontend)

    try:
        pcm, rate, channels, captions = next(stream(kokoro, args.voice, text, frontend))
        write_wav(args.output, pcm, rate, channels)
        Path(args.output + ".captions.json").write_text(json.dumps(captions), encoding="utf-8")
        print(f"OK -> {args.output}", file=sys.stderr)
        return 0
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
