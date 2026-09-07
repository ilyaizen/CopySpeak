#!/usr/bin/env python3
"""CLI wrapper for KittenTTS - used by CopySpeak.

Reads text (inline or from a file), synthesizes with KittenTTS, and writes a
24kHz WAV file. Kept stable so CopySpeak's args_template never changes when
upstream shifts.

Invoked by CopySpeak via:
    uv run --project {engine_dir}/kitten python {engine_dir}/kitten/scripts/copyspeak-kitten.py \
        --text-file {input} --voice {voice} --output {output} [--device cuda]

With --serve the model is loaded once and stays in RAM, speaking protocol v2
(see src-tauri/src/tts/local_daemon.rs): READY 2, then one JSON request per
stdin line answered with a format header, length-prefixed 16-bit LE PCM chunks,
and an end frame. Stdin EOF ends the process.
"""

import argparse
import glob
import json
import os
import sys
import wave
from pathlib import Path

SAMPLE_RATE = 24000


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


def pcm16(samples) -> bytes:
    """float32 in [-1, 1] -> signed 16-bit little-endian PCM bytes.

    CopySpeak's player only accepts 16-bit samples, so the conversion belongs
    here rather than in Rust.
    """
    import numpy as np

    clipped = np.clip(np.asarray(samples, dtype=np.float32), -1.0, 1.0)
    return (clipped * 32767.0).astype("<i2").tobytes()


def load_engine(model_name: str, device: str):
    """Build a KittenTTS and return it with the ONNX session it ended up using.

    The pinned 0.8.1 wheel takes no provider or device argument — its
    `KittenTTS(model_name, cache_dir)` builds the session itself — so the GPU is
    selected by swapping the session afterwards. Both `model` and `session` are
    public attributes of the object, and CPU never touches this path.
    """
    from kittentts import KittenTTS

    tts = KittenTTS(model_name)
    inner = getattr(tts, "model", None)
    session = getattr(inner, "session", None)

    if device == "cuda":
        if session is None or not hasattr(inner, "model_path"):
            raise RuntimeError(
                "this KittenTTS build exposes no ONNX session to move to the GPU"
            )
        import onnxruntime as ort

        inner.session = ort.InferenceSession(
            inner.model_path,
            providers=[
                ("CUDAExecutionProvider", {"cudnn_conv_algo_search": "HEURISTIC"}),
                "CPUExecutionProvider",
            ],
        )
        session = inner.session

    return tts, session


def stream(tts, voice: str, text: str):
    """Yield (pcm16_le_bytes, sample_rate, channels).

    KittenTTS has no incremental API, so this is one chunk per request; the
    framing is the same either way.
    """
    yield pcm16(tts.generate(text=text, voice=voice, clean_text=True)), SAMPLE_RATE, 1


def write_wav(output: str, pcm: bytes, rate: int, channels: int) -> None:
    Path(output).parent.mkdir(parents=True, exist_ok=True)
    with wave.open(output, "wb") as wf:
        wf.setnchannels(channels)
        wf.setsampwidth(2)
        wf.setframerate(rate)
        wf.writeframes(pcm)


def serve(tts, voice: str) -> int:
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
        for _ in stream(tts, voice, "Ready."):
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
            chunks = stream(tts, voice, json.loads(line)["text"])
            first = next(chunks, None)
            if first is None:
                raise RuntimeError("no audio produced")
            pcm, rate, channels = first
            frame({
                "ok": True,
                "sample_rate": rate,
                "channels": channels,
                "bits_per_sample": 16,
            })
            while True:
                frame({"chunk": len(pcm)})
                out.write(pcm)
                nxt = next(chunks, None)
                if nxt is None:
                    break
                pcm = nxt[0]
            frame({"end": True})
        except Exception as exc:  # pragma: no cover - environment dependent
            frame({"ok": False, "error": str(exc)})
        out.flush()


def main() -> int:
    parser = argparse.ArgumentParser(description="KittenTTS CLI wrapper for CopySpeak")
    parser.add_argument("--text", help="Inline text to synthesize")
    parser.add_argument("--text-file", help="Path to a UTF-8 text file to synthesize")
    parser.add_argument(
        "--voice",
        default="Rosie",
        help="Voice name. Options: Bella, Jasper, Luna, Bruno, Rosie, Hugo, Kiki, Leo",
    )
    parser.add_argument(
        "--model",
        default="KittenML/kitten-tts-nano-0.8",
        help="HuggingFace model id (default: kitten-tts-nano-0.8, 25MB)",
    )
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

    if args.device == "cuda":
        enable_cuda_dlls()

    try:
        import kittentts  # noqa: F401  (imported for its side effect: the check)
    except ImportError as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: missing dependency: {exc}", file=sys.stderr)
        print("Reinstall with: ./scripts/install-kittentts.ps1 -Force", file=sys.stderr)
        return 1

    try:
        tts, session = load_engine(args.model, args.device)
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

    # The daemon log is where a user checks whether the GPU is actually in use.
    providers = session.get_providers() if session is not None else "unknown"
    print(f"providers: {providers}", file=sys.stderr, flush=True)

    if args.serve:
        return serve(tts, args.voice)

    try:
        pcm, rate, channels = next(stream(tts, args.voice, text))
        write_wav(args.output, pcm, rate, channels)
        print(f"OK -> {args.output}", file=sys.stderr)
        return 0
    except Exception as exc:  # pragma: no cover - environment dependent
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
